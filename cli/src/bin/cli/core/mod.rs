use clap::{Parser, Subcommand};
use data::db::DbEngine;

pub(super) mod commands;

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
    /// Rescan previous MEV-Share events from a CSV file and simulate arbitrage opportunities, saving results to DB.
    Rescan {
        /// Path to CSV file.
        ///
        /// Row format: `tx_hash,profit_eth,event_block,event_timestamp`
        ///
        /// _example row:_
        /// `0x2b38211e0109bdf3b718f6cc1783fdd47c9e6b13858b0cfb9f528c6130c88ea4,0.003582784174380261,18042665,2023-09-01 15:42:22`
        #[arg(short, long, required = true)]
        file_path: String,
        /// DB Engine to use to store arb data. Defaults to "postgres".
        #[arg(short, long)]
        db_engine: Option<DbEngine>,
    },
    /// Export arbs from DB to a JSON file or another DB.
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
    /// Analyze arbs from DB and export the results to a JSON file or stdout.
    Analyze {
        // TODO: DRY these params
        /// File to save results to.
        ///
        /// All files are saved in `./analysis/`. (Default="analysis_{unix-timestamp}.json")
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
        #[arg(short = 'c', long)]
        token: Option<String>,
    },
}
