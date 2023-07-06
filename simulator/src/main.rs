use std::collections::HashMap;

use ethers::{
    prelude::providers::Middleware,
    types::{Transaction, H256},
};
use rusty_sando::{simulate::braindance_starting_balance, types::BlockInfo};
use simulator::{
    config::Config,
    data::{read_events, read_txs, write_tx_data, HistoricalEvent},
    sim::find_optimal_backrun_amount_in_out,
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
    let txs: Vec<Transaction> = vec![
        serde_json::from_str(
            r#"{
            "hash": "0x9c04a13b7b4b2a05123ef6ee796a030d71f205b04a4ca279ff8085acb37e0b4e",
            "nonce": "0x3",
            "blockHash": "0x6ac3b696c81125e781e8931cb575a38a6a18e0fd244ad9aba6307b0cf640aa72",
            "blockNumber": "0x10cbd94",
            "transactionIndex": "0x52",
            "from": "0xd4f74edd038f9a25208a0680d75602a5ea41038d",
            "to": "0x1111111254eeb25477b68fb85ed929f73a960582",
            "value": "0x0",
            "gasPrice": "0x41dc418ac",
            "gas": "0x65f4d",
            "input": "0x12aa3caf00000000000000000000000092f3f71cef740ed5784874b8c70ff87ecdf33588000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000993864e43caa7f7f12953ad6feb1d1ca635b875f00000000000000000000000092f3f71cef740ed5784874b8c70ff87ecdf33588000000000000000000000000d4f74edd038f9a25208a0680d75602a5ea41038d000000000000000000000000000000000000000000000000000000000823a39d00000000000000000000000000000000000000000000000df3e1320d907fb53a000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000018200006800004e80206c4eca27a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48b4f34d09124b8c9712957b76707b42510041ecbb0000000000000000000000000000000000000000000000000000000000042ad20020d6bdbf78a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800a007e5c0d20000000000000000000000000000000000000000000000000000f600008f0c20a0b86991c6218b36c1d19d4a2e9eb0ce3606eb483aa370aacf4cb08c7e1e7aa8e8ff9418d73c7e0f6ae40711b8002dc6c03aa370aacf4cb08c7e1e7aa8e8ff9418d73c7e0f424485f89ea52839fdb30640eb7dd7e0078e12fb00000000000000000000000000000000000000000000000000f36d677c660ec7a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800206ae4071138002dc6c0424485f89ea52839fdb30640eb7dd7e0078e12fb1111111254eeb25477b68fb85ed929f73a96058200000000000000000000000000000000000000000000000df3e1320d907fb53ac02aaa39b223fe8d0a0e5c4f27ead9083c756cc213dbfa98",
            "v": "0x0",
            "r": "0x26a0afb5560b34a0f78921ad33625d71c457d75df088ba542abd501004f13214",
            "s": "0x10b1fefc81664e28526b47ca678c50a32c7d72b9ee4155297e1981a7e34a3daf",
            "type": "0x2",
            "accessList": [],
            "maxPriorityFeePerGas": "0x14dc9380",
            "maxFeePerGas": "0x65a03c400",
            "chainId": "0x1"
          }"#,
        )?,
        // serde_json::from_str(
        //     r#"{
        //     "hash": "0x9c04a13b7b4b2a05123ef6ee796a030d71f205b04a4ca279ff8085acb37e0b4e",
        //     "nonce": "0x3",
        //     "blockHash": "0x6ac3b696c81125e781e8931cb575a38a6a18e0fd244ad9aba6307b0cf640aa72",
        //     "blockNumber": "0x10cbd94",
        //     "transactionIndex": "0x52",
        //     "from": "0xd4f74edd038f9a25208a0680d75602a5ea41038d",
        //     "to": "0x1111111254eeb25477b68fb85ed929f73a960582",
        //     "value": "0x0",
        //     "gasPrice": "0x41dc418ac",
        //     "gas": "0x65f4d",
        //     "input": "0x12aa3caf00000000000000000000000092f3f71cef740ed5784874b8c70ff87ecdf33588000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000993864e43caa7f7f12953ad6feb1d1ca635b875f00000000000000000000000092f3f71cef740ed5784874b8c70ff87ecdf33588000000000000000000000000d4f74edd038f9a25208a0680d75602a5ea41038d000000000000000000000000000000000000000000000000000000000823a39d00000000000000000000000000000000000000000000000df3e1320d907fb53a000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000018200006800004e80206c4eca27a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48b4f34d09124b8c9712957b76707b42510041ecbb0000000000000000000000000000000000000000000000000000000000042ad20020d6bdbf78a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800a007e5c0d20000000000000000000000000000000000000000000000000000f600008f0c20a0b86991c6218b36c1d19d4a2e9eb0ce3606eb483aa370aacf4cb08c7e1e7aa8e8ff9418d73c7e0f6ae40711b8002dc6c03aa370aacf4cb08c7e1e7aa8e8ff9418d73c7e0f424485f89ea52839fdb30640eb7dd7e0078e12fb00000000000000000000000000000000000000000000000000f36d677c660ec7a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800206ae4071138002dc6c0424485f89ea52839fdb30640eb7dd7e0078e12fb1111111254eeb25477b68fb85ed929f73a96058200000000000000000000000000000000000000000000000df3e1320d907fb53ac02aaa39b223fe8d0a0e5c4f27ead9083c756cc213dbfa98",
        //     "v": "0x0",
        //     "r": "0x26a0afb5560b34a0f78921ad33625d71c457d75df088ba542abd501004f13214",
        //     "s": "0x10b1fefc81664e28526b47ca678c50a32c7d72b9ee4155297e1981a7e34a3daf",
        //     "type": "0x2",
        //     "accessList": [],
        //     "maxPriorityFeePerGas": "0x14dc9380",
        //     "maxFeePerGas": "0x65a03c400",
        //     "chainId": "0x1"
        //   }"#,
        // )?,
    ];
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
            let res = find_optimal_backrun_amount_in_out(&client, tx, &event, &block_info).await?;
            if res.1 > braindance_starting_balance() {
                println!("PROFIT: {:?}", res);
                break;
            }
        } else {
            println!("next block hash is none");
            continue;
        }
        // })
        // );
    }
    // for handle in thread_handlers.into_iter() {
    //     handle.await?;
    // }

    Ok(())
}
