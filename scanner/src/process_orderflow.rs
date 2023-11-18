use std::sync::Arc;

use async_trait::async_trait;
use data::arbs::ArbDatabase;
use ethers::{
    middleware::Middleware,
    types::{Transaction, U256},
};
use futures::future;
use hindsight_core::{
    error::HindsightError,
    eth_client::WsClient,
    info,
    interfaces::{BlockInfo, SimArbResultBatch},
    util::H256Map,
    Error, Result,
};
use mev_share_sse::EventHistory;
use sim::core::find_optimal_backrun_amount_in_out;

#[async_trait]
pub trait ArbClient {
    async fn simulate_arbs(
        &self,
        txs: &Vec<Transaction>,
        batch_size: usize,
        db: Option<ArbDatabase>,
        event_map: H256Map<EventHistory>,
    ) -> Result<()>;

    async fn backrun_tx(
        &self,
        tx: Transaction,
        event_map: &H256Map<EventHistory>,
    ) -> Result<SimArbResultBatch>;
}

#[async_trait]
impl ArbClient for WsClient {
    /// For each tx in `txs`, simulates an optimal backrun-arbitrage in a parallel thread,
    /// caching results in batches of size `batch_size`.
    ///
    /// Saves results into `db` after each batch is processed. Returns when all txs are processed.
    async fn simulate_arbs(
        &self,
        txs: &Vec<Transaction>,
        batch_size: usize,
        db: Option<ArbDatabase>,
        event_map: H256Map<EventHistory>,
    ) -> Result<()> {
        info!("loaded {} transactions total...", txs.len());
        let mut processed_txs = 0;
        while processed_txs < txs.len() {
            let mut handlers = vec![];
            let txs_batch = txs
                .iter()
                .skip(processed_txs)
                .take(batch_size)
                .map(|tx| tx.to_owned())
                .collect::<Vec<Transaction>>();
            processed_txs += txs_batch.len();
            info!("processing {} txs", txs_batch.len());
            for tx in txs_batch {
                let event_map = event_map.clone();
                let client = Arc::new(self.to_owned());
                handlers.push(tokio::task::spawn(async move {
                    client.backrun_tx(tx, &event_map).await.ok()
                }));
            }
            let results = future::join_all(handlers).await;
            let results = results
                .into_iter()
                .filter_map(|res| res.ok())
                .flatten()
                .collect::<Vec<_>>();
            info!("batch results: {:#?}", results);
            if let Some(db) = db.to_owned() {
                if !results.is_empty() {
                    db.to_owned().write_arbs(&results).await?;
                }
            }
        }
        Ok(())
    }

    /// Simulate a backrun-arbitrage on a single tx.
    ///
    /// `event_map` should be a map of tx hashes to their corresponding event history entries.
    async fn backrun_tx(
        &self,
        tx: Transaction,
        event_map: &H256Map<EventHistory>,
    ) -> Result<SimArbResultBatch> {
        let event = event_map
            .get(&tx.hash)
            .ok_or(HindsightError::EventNotCached(tx.hash))?;
        let sim_block_num = tx
            .block_number
            .ok_or(HindsightError::TxNotLanded(tx.hash))?;

        // we're simulating txs that have already landed, so we want the block prior to when the tx landed
        let sim_block_num = sim_block_num.as_u64() - 1;
        let block = self
            .provider
            .get_block(sim_block_num)
            .await?
            .ok_or::<Error>(HindsightError::BlockNotFound(sim_block_num).into())?;
        let block_info = BlockInfo {
            number: sim_block_num.into(),
            timestamp: block.timestamp,
            base_fee_per_gas: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
            gas_limit: Some(block.gas_limit),
            gas_used: Some(block.gas_used),
        };

        let res =
            find_optimal_backrun_amount_in_out(Arc::new(self.clone()), tx, event, &block_info)
                .await?;
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
}

#[cfg(test)]
mod tests {
    use ethers::{providers::Middleware, types::H256};
    use serde_json::json;

    use data::{
        arbs::ArbFilterParams,
        db::{Db, DbEngine},
        MongoConfig,
    };
    use hindsight_core::eth_client::test::get_test_ws_client;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_processes_orderflow() -> Result<()> {
        let client = get_test_ws_client().await?;
        // let hindsight = Hindsight::new(client).await?;

        // data from an actual juicy event
        let juicy_event: EventHistory = serde_json::from_value(json!({
          "block": 17637019,
          "timestamp": 1688673408,
          "hint": {
            "txs": null,
            "hash": "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a",
            "logs": [
              {
                "address": "0x5db3d38bd40c862ba1fdb2286c32a62ab954d36d",
                "topics": [
                  "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
                  "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "0x0000000000000000000000000000000000000000000000000000000000000000"
                ]
              },
              {
                "address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
                "topics": [
                  "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
                  "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "0x0000000000000000000000000000000000000000000000000000000000000000"
                ]
              },
              {
                "address": "0x36bcf57291a291a6e0e0bff7b12b69b556bcd9ed",
                "topics": [
                  "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
                  "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "0x0000000000000000000000000000000000000000000000000000000000000000"
                ]
              }
            ]
          }
        }))?;
        let juicy_tx_hash: H256 =
            "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a".parse::<H256>()?;
        let juicy_tx = get_test_ws_client()
            .await?
            .provider
            .get_transaction(juicy_tx_hash)
            .await?
            .expect("failed to find juicy tx on chain");
        let event_map = [juicy_event]
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();
        let test_db = Db::new(DbEngine::Mongo(MongoConfig::default())).await;

        // run the sim, it will save a result to the "test" DB
        client
            .simulate_arbs(
                vec![juicy_tx].as_ref(),
                1,
                Some(test_db.connect.clone()),
                event_map,
            )
            .await?;

        // check DB for result
        let arbs = test_db
            .connect
            .read_arbs(&ArbFilterParams::none(), None, None)
            .await?;
        assert!(arbs
            .into_iter()
            .map(|arb| arb.event.hint.hash)
            .collect::<Vec<_>>()
            .contains(&juicy_tx_hash));
        Ok(())
    }
}
