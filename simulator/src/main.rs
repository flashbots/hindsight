use clap::{Parser, Subcommand};
use ethers::types::U256;
use hindsight::{
    commands::{self},
    config::Config,
    data::arbs::ArbFilterParams,
    debug,
};
use revm::primitives::bitvec::macros::internal::funty::Fundamental;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Enum to parse CLI params.
#[derive(Subcommand)]
enum Commands {
    /// Run arb simulator on one example transaction.
    Test {
        /// Simulate more than one tx at a time.
        #[arg(short, long)]
        batch_size: Option<usize>,
        #[arg(short, long)]
        save_to_db: bool,
    },
    /// Scan previous MEV-Share events for arbitrage opportunities. Automatically saves results to DB.
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
    },
    /// Export arbs from DB to a JSON file.
    Export {
        /// File to save arbs to. (Default="arbs.json")
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
        /// Minimum profit of arb to export, in ETH decimal format (e.g. 0.01 => 1e17 wei)
        #[arg(short = 'p', long)]
        min_profit: Option<f64>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::default();
    let cli = Cli::parse();

    match cli.debug {
        0 => {
            println!("no debug");
        }
        1 => {
            println!("debug 1");
        }
        2 => {
            println!("debug 2");
        }
        _ => {
            println!("max debug");
        }
    }

    match cli.command {
        Some(Commands::Test {
            batch_size,
            save_to_db,
        }) => {
            commands::test::run(batch_size, config, save_to_db).await?;
        }
        Some(Commands::Scan {
            block_end,
            block_start,
            timestamp_end,
            timestamp_start,
            batch_size,
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
            };
            commands::scan::run(scan_options, config).await?;
        }
        Some(Commands::Export {
            filename,
            block_end,
            block_start,
            timestamp_end,
            timestamp_start,
            min_profit,
        }) => {
            let min_profit = if let Some(min_profit) = min_profit {
                min_profit
            } else {
                0f64
            };

            let umin_profit = U256::from((min_profit * 1e9) as u64) * U256::from(1e9.as_u64());
            commands::export::run(
                filename,
                ArbFilterParams {
                    block_end,
                    block_start,
                    timestamp_end,
                    timestamp_start,
                    min_profit: Some(umin_profit),
                },
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
