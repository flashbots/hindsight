// use std::path::PathBuf;

use clap::{Parser, Subcommand};
use mev_share_sse::EventHistoryParams;
use simulator::{commands, config::Config, hindsight::HindsightFactory};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run arb simulator on one example transaction.
    Test {
        /// Simulate more than one tx at a time.
        #[arg(short, long)]
        batch_size: Option<usize>,
    },
    Scan {
        /// Scan events from MEV-Share event stream.
        #[arg(short, long)]
        block_start: Option<u64>,
        #[arg(short, long)]
        timestamp_start: Option<u64>,
        #[arg(long)]
        block_end: Option<u64>,
        #[arg(long)]
        timestamp_end: Option<u64>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::load()?;
    let cli = Cli::parse();

    // if let Some(name) = cli.name {
    //     println!("name: {}", name);
    // }
    // if let Some(config) = cli.config.as_deref() {
    //     println!("config: {:?}", config.display());
    // }

    println!(
        "oohh geeez\nauth signer\t{:?}\nrpc url\t\t{:?}",
        config.auth_signer_key, config.rpc_url_ws
    );

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
        Some(Commands::Test { batch_size }) => {
            println!("test command");
            let hindsight = HindsightFactory::new().init(config.to_owned()).await?;
            println!("cache events: {:?}", hindsight.event_map.len());
            println!("cache txs: {:?}", hindsight.cache_txs.len());
            commands::test::run(hindsight, batch_size).await?;
        }
        Some(Commands::Scan {
            block_start,
            timestamp_start,
            block_end,
            timestamp_end,
        }) => {
            println!("scan command");
            let params = EventHistoryParams {
                block_start,
                block_end,
                timestamp_start,
                timestamp_end,
                limit: None,
                offset: None,
            };
            commands::scan::run(params, None).await?;
        }
        None => {
            println!("for usage, run: cargo run -- --help");
        }
    }

    Ok(())
}
