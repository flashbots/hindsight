use clap::{Parser, Subcommand};
use hindsight::data::db::DbEngine;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = "20")]
    pub ws_max_reconnects: Option<usize>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

/// Analyze historical events from MEV-Share to simulate past arbitrage opportunities and export the simulated profits.
#[derive(Subcommand)]
pub enum Commands {
    /// Scan previous MEV-Share events and simulate arbitrage opportunities. Automatically saves results to DB.
    Scan {
        /// Scan from this block.
        #[arg(short, long)]
        block_start: Option<u32>,
        /// Scan from this block.
        #[arg(short, long)]
        timestamp_start: Option<u32>,
        /// Scan until this block.
        #[arg(long)]
        block_end: Option<u32>,
        /// Scan until this timestamp.
        #[arg(long)]
        timestamp_end: Option<u32>,
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
        timestamp_start: Option<u32>,
        /// Stop exporting arbs at this timestamp.
        #[arg(long)]
        timestamp_end: Option<u32>,
        /// Export arbs starting from this block.
        #[arg(short, long)]
        block_start: Option<u32>,
        /// Stop exporting arbs at this block.
        #[arg(long)]
        block_end: Option<u32>,
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
