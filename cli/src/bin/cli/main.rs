mod config;
mod core;

use crate::core::{commands, Cli, Commands};
use config::DotEnv;
use data::{
    arbs::{ArbFilterParams, WriteEngine},
    db::{Db, DbEngine},
    MongoConfig, PostgresConfig,
};
use ethers::{types::H160, utils::parse_ether};
use hindsight_core::{
    debug,
    eth_client::WsClient,
    info,
    interfaces::{Config, TokenPair},
    mev_share_sse::EventClient,
    util::WETH,
};
use std::thread::available_parallelism;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    debug!("parsing args...");
    let cli = Cli::parse_args();
    debug!("loading .env file...");
    let config = Config::from_env();

    ctrlc::set_handler(move || {
        info!("\nstopping hindsight!");
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let max_reconnects = cli.ws_max_reconnects.unwrap_or_default();
    debug!("initializing ws client...");
    let ws_client = WsClient::new(Some(config.rpc_url_ws), max_reconnects).await?;
    debug!("initializing mev-share client...");
    let mevshare = EventClient::default();

    debug!("running command...");
    match cli.command {
        Some(Commands::Scan {
            // cli args:
            block_end,
            block_start,
            timestamp_end,
            timestamp_start,
            batch_size,
            db_engine,
        }) => {
            /* If no start/end params are defined,
                refine params based on ranges present in DB.
                Overwriting old results may be accomplished by setting the start/end timestamp/block params.
                Replace the provided timestamp/block params with the latest respective
                value + 1 in the DB if it's higher than the param.
                We add 1 to prevent duplicates. If an arb is saved in the DB,
                then we know we've scanned & simulated up to that point.
                Timestamp is evaluated by default, falls back to block.
            */
            let db_engine = match db_engine.unwrap_or_default() {
                // ignore the configs the cli passes; they're populated w/ default vals
                // we want to load vars from .env
                // TODO: revisit this, seems wrong
                DbEngine::Postgres(_) => DbEngine::Postgres(PostgresConfig::from_env()),
                DbEngine::Mongo(_) => DbEngine::Mongo(MongoConfig::from_env()),
            };
            let db = Db::new(db_engine.to_owned()).await;
            let (block_start, timestamp_start) =
                if block_start.is_none() && timestamp_start.is_none() {
                    let db_ranges = db.connect.get_previously_saved_ranges().await?;
                    info!("previously saved event ranges: {:?}", db_ranges);
                    let block_start = db_ranges.latest_block;
                    let timestamp_start = db_ranges.latest_timestamp;
                    (block_start as u32, timestamp_start as u32)
                } else {
                    if block_start.is_some() && timestamp_start.is_some() {
                        panic!("cannot specify both block_start and timestamp_start");
                    }
                    // use whichever is specified; the other (being 1) will not alter the selection
                    (block_start.unwrap_or(1), timestamp_start.unwrap_or(1))
                };

            let batch_size = batch_size.unwrap_or(
                available_parallelism()
                    .map(|n| usize::from(n) / 2)
                    .unwrap_or(4)
                    .max(1),
            );
            info!("batch size: {}", batch_size);
            let scan_options = commands::scan::ScanOptions {
                block_start,
                block_end,
                timestamp_start,
                timestamp_end,
                batch_size,
                db_engine,
            };
            commands::scan::run(scan_options.to_owned(), &ws_client, &mevshare, &db.connect)
                .await?;
        }
        Some(Commands::Rescan {
            file_path,
            db_engine,
        }) => {
            let db_engine = db_engine.unwrap_or(DbEngine::Postgres(PostgresConfig::from_env()));
            let db = Db::new(db_engine).await;

            // parse the CSV file into TxEvents
            let tx_events = commands::rescan::parse_csv(&file_path).await?;

            // run the rescan command with chunked concurrency
            for chunk in tx_events.chunks(10) {
                commands::rescan::run(chunk, &ws_client, &mevshare, &db.connect).await?;
            }
        }
        Some(Commands::Export {
            // cli args:
            filename,
            block_end,
            block_start,
            timestamp_end,
            timestamp_start,
            min_profit,
            read_db,
            write_db,
        }) => {
            let min_profit = min_profit.unwrap_or(0f64);
            if min_profit < 0f64 {
                panic!("min_profit must be >= 0");
            } else if min_profit > 0.0 && min_profit * 1e9 < 1.0 {
                panic!("min_profit must be >= 1e9 wei");
            }
            let umin_profit = parse_ether(min_profit)?;

            let db_engine = read_db.unwrap_or_default();
            let read_db = Db::new(db_engine.to_owned()).await.connect;
            // if filename is specified, use that, otherwise try write_db
            // if filename & write_db are both None, use file exporter & default filename
            let write_dest = if filename.is_some() {
                WriteEngine::File(filename)
            } else if let Some(write_db) = write_db {
                WriteEngine::Db(write_db)
            } else {
                WriteEngine::File(None)
            };

            commands::export::run(
                ArbFilterParams {
                    block_end,
                    block_start,
                    timestamp_end,
                    timestamp_start,
                    min_profit: Some(umin_profit),
                    token_pair: None,
                },
                &read_db,
                write_dest,
            )
            .await?;
        }
        Some(Commands::Analyze {
            filename,
            timestamp_start,
            timestamp_end,
            block_start,
            block_end,
            min_profit,
            read_db,
            write_db,
            token,
        }) => {
            let min_profit = if let Some(min_profit) = min_profit {
                Some(parse_ether(min_profit)?)
            } else {
                None
            };
            let token = if let Some(token) = token {
                Some(token.parse::<H160>()?)
            } else {
                None
            };

            let db_engine = read_db.unwrap_or_default();
            let read_db = Db::new(db_engine.to_owned()).await.connect;
            let write_dest = if filename.is_some() {
                WriteEngine::File(filename)
            } else if let Some(write_db) = write_db {
                WriteEngine::Db(write_db)
            } else {
                WriteEngine::File(None)
            };

            let params = ArbFilterParams {
                block_end,
                block_start,
                timestamp_end,
                timestamp_start,
                min_profit,
                token_pair: if let Some(token) = token {
                    Some(TokenPair { token, weth: *WETH })
                } else {
                    None
                },
            };
            commands::analyze::run(params, &read_db, write_dest).await?;
        }
        None => {
            let program = std::env::args().next().unwrap_or("hindsight".to_owned());
            println!("for usage, run: {} --help", program);
        }
    }

    Ok(())
}
