use std::collections::HashMap;

use ethers::{prelude::providers::Middleware, types::H256};
use rusty_sando::types::BlockInfo;
use simulator::{
    config::Config,
    data::{read_events, read_txs, write_tx_data, HistoricalEvent},
    sim::{find_optimal_backrun_amount_in, sim_arb},
    util::fetch_txs,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let client = simulator::util::get_ws_client(config.rpc_url_ws.to_owned()).await?;
    let cache_events = read_events(None).await?;
    let cache_txs = read_txs(None).await?;

    // map tx_hash => event
    let event_map = cache_events
        .events
        .clone()
        .into_iter()
        .map(|e| (e.hint.hash, e))
        .collect::<HashMap<H256, HistoricalEvent>>();

    println!("cache events: {:?}", event_map.len());
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

    // let mut thread_handlers = vec![];
    let txs = cache_txs.split_at(18).1.to_vec();
    println!("txs: {:?}", txs.len());
    for tx in txs {
        let client = client.clone();
        let event = event_map.get(&tx.hash).unwrap().to_owned();
        // thread_handlers.push(
        // tokio::spawn(async move {
        println!("tx block: {:?}", tx.block_number);
        let sim_block_num = tx.block_number;
        if let Some(sim_block_num) = sim_block_num {
            // we're simulating txs that have already landed, so we want the block prior to that
            let sim_block_num = sim_block_num.as_u64() - 1;
            println!("sim block num: {:?}", sim_block_num);
            // TODO: clean up all these unwraps!
            // TODO: clean up all these unwraps!
            // TODO: clean up all these unwraps!
            let block = client.get_block(sim_block_num).await.unwrap().unwrap();
            let block_info = BlockInfo {
                number: sim_block_num.into(),
                timestamp: block.timestamp,
                base_fee: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
            };
            // let bundle = vec![
            //     tx.to_owned(),
            //     // my backrun here
            // ];
            // let mut evm = fork_evm(&client, &block_info).await.unwrap();
            let res = find_optimal_backrun_amount_in(&client, tx, &event, &block_info).await;
            println!("res: {:?}", res);
            if res.is_ok() {
                break;
            }
            // let sim_result = sim_bundle(&mut evm, bundle).await.unwrap();
            // let success = sim_result
            //     .iter()
            //     .map(|r| r.is_success())
            //     .reduce(|a, b| a && b)
            //     .unwrap_or(false);
            // println!(
            //     "{}{:?}",
            //     if success {
            //         "sim result".green()
            //     } else {
            //         "sim result".red()
            //     },
            //     sim_result
            // );
        } else {
            panic!("next block hash is none");
        }
        // })
        // );
    }
    // for handle in thread_handlers.into_iter() {
    //     handle.await?;
    // }

    Ok(())
}
