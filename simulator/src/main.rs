use std::path::PathBuf;

use clap::{Parser, Subcommand};
use simulator::{commands, config::Config, hindsight::HindsightFactory};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    // Test {
    //     /// lists test values
    //     #[arg(short, long)]
    //     list: bool,
    // },
    /// Run arb simulator on one example transaction.
    Test,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
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

    let hindsight = HindsightFactory::new().init(config.to_owned()).await?;
    println!("cache events: {:?}", hindsight.event_map.len());
    println!("cache txs: {:?}", hindsight.cache_txs.len());

    match cli.command {
        Some(Commands::Test {}) => {
            println!("test command");
            commands::test::run(hindsight).await?;
        }
        None => {
            println!("for usage, run: cargo run -- --help");
        }
    }

    if let Some(name) = cli.name {
        println!("name: {}", name);
    }
    if let Some(config) = cli.config.as_deref() {
        println!("config: {:?}", config.display());
    }

    println!(
        "oohh geeez\nauth signer\t{:?}\nrpc url\t\t{:?}",
        config.auth_signer_key, config.rpc_url_ws
    );

    // let mut thread_handlers = vec![];

    Ok(())
}
