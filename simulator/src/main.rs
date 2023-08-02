use clap::{Parser, Subcommand};
use ethers::types::U256;
use hindsight::{
    commands::{self},
    config::Config,
    data::arbs::ArbFilterParams,
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
