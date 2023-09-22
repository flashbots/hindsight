use ethers::types::U256;
use hindsight::{
    commands::{self},
    config::Config,
    data::{
        arbs::{ArbFilterParams, WriteEngine},
        db::DbEngine,
        MongoConfig,
    },
    debug,
};
use revm::primitives::bitvec::macros::internal::funty::Fundamental;
mod cli;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::default();
    let cli = Cli::parse_args();

    match cli.command {
        Some(Commands::Scan {
            block_end,
            block_start,
            timestamp_end,
            timestamp_start,
            batch_size,
            db_engine,
        }) => {
            debug!("scan command");
            let scan_options = commands::scan::ScanOptions {
                block_start,
                block_end,
                timestamp_start,
                timestamp_end,
                batch_size,
                db_engine: db_engine.unwrap_or(DbEngine::Mongo(MongoConfig::default())),
            };
            commands::scan::run(scan_options.to_owned(), config.to_owned()).await?;
        }
        Some(Commands::Export {
            filename,
            block_end,
            block_start,
            timestamp_end,
            timestamp_start,
            min_profit,
            read_db,
            write_db,
        }) => {
            let min_profit = if let Some(min_profit) = min_profit {
                min_profit
            } else {
                0f64
            };

            let umin_profit = U256::from((min_profit * 1e9) as u64) * U256::from(1e9.as_u64());

            // if filename is specified, use that, otherwise use write_engine
            // if filename & write_engine are both not specified, use file exporter & default filename
            let write_dest = if filename.is_some() {
                WriteEngine::File(filename)
            } else {
                if let Some(write_db) = write_db {
                    WriteEngine::Db(write_db)
                } else {
                    WriteEngine::File(None)
                }
            };
            commands::export::run(
                ArbFilterParams {
                    block_end,
                    block_start,
                    timestamp_end,
                    timestamp_start,
                    min_profit: Some(umin_profit),
                },
                read_db.unwrap_or(DbEngine::Mongo(MongoConfig::default())),
                write_dest,
            )
            .await?;
        }
        None => {
            let program = std::env::args().next().unwrap_or("hindsight".to_owned());
            println!("for usage, run: {} --help", program);
        }
    }

    Ok(())
}
