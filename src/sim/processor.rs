use crate::error::HindsightError;
use crate::interfaces::SimArbResultBatch;
use crate::{info, Error, Result};
use crate::{sim::core::find_optimal_backrun_amount_in_out, util::WsClient};
use ethers::{
    providers::Middleware,
    types::{Transaction, H256, U256},
};
use mev_share_sse::EventHistory;
use rusty_sando::types::BlockInfo;
use std::collections::HashMap;

pub type H256Map<T> = HashMap<H256, T>;

pub async fn simulate_backrun_arbs(
    client: &WsClient,
    tx: Transaction,
    event_map: &H256Map<EventHistory>,
) -> Result<SimArbResultBatch> {
    let event = event_map
        .get(&tx.hash)
        .ok_or::<Error>(HindsightError::EventNotCached(tx.hash).into())?;
    let sim_block_num = tx
        .block_number
        .ok_or::<Error>(HindsightError::TxNotLanded(tx.hash).into())?;

    // we're simulating txs that have already landed, so we want the block prior to when the tx landed
    let sim_block_num = sim_block_num.as_u64() - 1;
    let block = client
        .get_block(sim_block_num)
        .await?
        .ok_or::<Error>(HindsightError::BlockNotFound(sim_block_num).into())?;
    let block_info = BlockInfo {
        number: sim_block_num.into(),
        timestamp: block.timestamp,
        base_fee: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
    };

    let res = find_optimal_backrun_amount_in_out(client, tx, event, &block_info).await?;
    let mut max_profit = U256::from(0);
    /*
       Sum up the profit from each result. Generally there should only be one result, but if
       there are >1 results, we assume that we'd do both backruns in one tx.
    */
    for res in &res {
        if res.backrun_trade.profit > max_profit {
            info!(
                "sim was profitable: input={:?}\tend_balance={:?}",
                res.backrun_trade.amount_in, res.backrun_trade.balance_end
            );
            max_profit = res.backrun_trade.profit;
        }
    }
    Ok(SimArbResultBatch {
        event: event.to_owned(),
        max_profit,
        results: res,
    })
}
