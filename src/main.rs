use clap::{Parser, Subcommand};
use ethers::types::U256;
use hindsight::{
    commands::{self},
    config::Config,
    data::{
        arbs::{ArbFilterParams, WriteEngine},
        db::DbEngine,
        MongoConfig,
    },
    debug, info,
};
use revm::primitives::bitvec::macros::internal::funty::Fundamental;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Analyze historical events from MEV-Share to simulate past arbitrage opportunities and export the simulated profits.
#[derive(Subcommand)]
enum Commands {
    /// Scan previous MEV-Share events and simulate arbitrage opportunities. Automatically saves results to DB.
    Scan {
        /// Scan from this block.
        #[arg(short, long)]
        block_start: Option<u64>,
        /// Scan from this block.
        #[arg(short, long)]
        timestamp_start: Option<u64>,
        /// Scan until this block.
        #[arg(long)]
        block_end: Option<u64>,
        /// Scan until this timestamp.
        #[arg(long)]
        timestamp_end: Option<u64>,
        /// Number of transactions to simulate concurrently. Defaults to 1/2 the CPU cores on host.
        #[arg(short = 'n', long)]
        batch_size: Option<usize>,
        /// DB Engine to use to store arb data. Defaults to "mongo".
        #[arg(
            long = "db",
            help = &format!("<{}>: DB engine to store arb data, defaults to mongo", DbEngine::enum_flags())
        )]
        db_engine: Option<DbEngine>,
    },
    /// Export arbs from DB to a JSON file.
    Export {
        /// File to save arbs to.
        ///
        /// All files are saved in `./arbData/`. (Default="arbs_{unix-timestamp}.json")
        #[arg(short, long)]
        filename: Option<String>,
        /// Export arbs starting from this timestamp.
        #[arg(short, long)]
        timestamp_start: Option<u64>,
        /// Stop exporting arbs at this timestamp.
        #[arg(long)]
        timestamp_end: Option<u64>,
        /// Export arbs starting from this block.
        #[arg(short, long)]
        block_start: Option<u64>,
        /// Stop exporting arbs at this block.
        #[arg(long)]
        block_end: Option<u64>,
        /// Minimum profit of arb to export, in ETH decimal format (e.g. 0.01 => 1e16 wei)
        #[arg(short = 'p', long)]
        min_profit: Option<f64>,
        /// DB Engine to use to store arb data. Defaults to "mongo".
        /// TODO: DRY this up
        #[arg(
            long = "db",
            help = &format!("<{}>: DB engine to read arb data from, defaults to mongo", DbEngine::enum_flags())
        )]
        read_db: Option<DbEngine>,
        #[arg(
            short = 'o',
            long = "db-out",
            help = &format!("<{}>: DB engine to write arb data to, default None (save to file). Ignored if --filename is specified.", DbEngine::enum_flags())
        )]
        write_db: Option<DbEngine>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::default();
    let cli = Cli::parse();

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
                filename_events: None,
                filename_txs: None,
                batch_size,
                db_engine: db_engine.unwrap_or(DbEngine::Mongo(MongoConfig::default())),
            };
            loop {
                let res = commands::scan::run(scan_options.to_owned(), config.to_owned()).await;
                if res.is_err() {
                    info!("program crashed with error {:?}, restarting...", res);
                }
            }
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
