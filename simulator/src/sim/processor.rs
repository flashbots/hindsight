use std::collections::HashMap;

use ethers::{
    providers::Middleware,
    types::{Transaction, H256},
};
use rusty_sando::{simulate::braindance_starting_balance, types::BlockInfo};

use crate::{data::HistoricalEvent, sim::core::find_optimal_backrun_amount_in_out, util::WsClient};

use super::Result;

pub type H256Map<T> = HashMap<H256, T>;

pub async fn process_orderflow(
    client: &WsClient,
    txs: Vec<Transaction>,
    event_map: H256Map<HistoricalEvent>,
) -> Result<()> {
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
