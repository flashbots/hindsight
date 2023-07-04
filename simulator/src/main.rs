use std::sync::Arc;

use ethers::{prelude::providers::Middleware, types::Transaction};
use rusty_sando::types::BlockInfo;
use simulator::{
    config::Config,
    data::{read_events, read_txs, write_tx_data},
    sim::sim_bundle,
    util::fetch_txs,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let client = simulator::util::get_ws_client(config.rpc_url_ws.to_owned()).await?;
    let cache_events = read_events(None).await?;
    let cache_txs = read_txs(None).await?;

    println!("cache events: {:?}", cache_events.events.len());
    println!("cache txs: {:?}", cache_txs.len());

    println!(
        "oohh geeez\nauth signer\t{:?}\nrpc url\t\t{:?}",
        config.auth_signer_key, config.rpc_url_ws
    );

    if cache_txs.len() > 0 {
        println!("txs found in cache, skipping tx fetch");
    } else {
        println!("fetching txs");
        let cached_txs = fetch_txs(&client, cache_events.events).await?;
        write_tx_data(None, serde_json::to_string_pretty(&cached_txs)?).await?;
    }

    let signed_txs = vec![
        cache_txs[0].clone(),
        cache_txs[1].clone(),
        cache_txs[2].clone(),
        cache_txs[3].clone(),
        cache_txs[4].clone(),
    ];

    let sim_block_num = signed_txs[0].block_number;
    if let Some(sim_block_num) = sim_block_num {
        // we're simulating txs that have already landed, so we want the block prior to that
        let sim_block_num = sim_block_num.as_u64() - 1;
        println!("sim block num: {:?}", sim_block_num);
        let block = client.get_block(sim_block_num).await?.unwrap();
        let next_block = BlockInfo {
            number: sim_block_num.into(),
            timestamp: block.timestamp,
            base_fee: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
        };
        let _sim_result = sim_bundle(&client, signed_txs, &next_block).await?;
    } else {
        panic!("next block hash is none");
    }

    Ok(())
}
