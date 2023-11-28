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

        let res = find_optimal_backrun_amount_in_out(self, tx, event, &block_info).await?;
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
    use serde_json::json;

    use data::{
        arbs::ArbFilterParams,
        db::{Db, DbEngine},
        MongoConfig,
    };
    use hindsight_core::eth_client::test::get_test_ws_client;

    use super::*;

    fn get_juicy_tx() -> Transaction {
        serde_json::from_value(json!({
            "hash": "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a",
            "nonce": "0x273",
            "blockHash": "0x319833ccb287c0b9bf65f2f4f8df10327e22e38bd7e1c350af3f96a7829dca68",
            "blockNumber": "0x10d1e9d",
            "transactionIndex": "0x5b",
            "from": "0x8228c693548151805f0e704b5cdf522be95d96a6",
            "to": "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad",
            "value": "0x0",
            "gasPrice": "0x584f095e9",
            "gas": "0x5ab14",
            "input": "0x3593564c000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000064a7237b0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000a2a15d09519be0000000000000000000000000000000000000000000000000007d6a7b23e1586e422100000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000005915f74458ae0bfdaa1a96ca1aa779d715cc1eefe40001f4a0b86991c6218b36c1d19d4a2e9eb0ce3606eb480001f4c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8f16e81dce15b08f326220742020379b855b87df900000000000000",
            "v": "0x0",
            "r": "0xf1acfb5a92077d41ffd708affb12b27235b6d32467cdf372b53ea9d245c18d2c",
            "s": "0x732f25823fc9714ff12135c89697101f517b421cbad07c8aa4b07c8dc0cfe779",
            "type": "0x2",
            "accessList": [],
            "maxPriorityFeePerGas": "0x5f5e100",
            "maxFeePerGas": "0x82fc195db",
            "chainId": "0x1"
        })).unwrap()
    }

    fn get_juicy_event() -> EventHistory {
        serde_json::from_value(json!({
          "block": 17637019,
          "timestamp": 1688673408,
          "hint": {
            "txs": [],
            "hash": "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a",
            "logs": [
              {
                "address": "0x5db3d38bd40c862ba1fdb2286c32a62ab954d36d",
                "data": "0x",
                "topics": [
                  "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
                  "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "0x0000000000000000000000000000000000000000000000000000000000000000"
                ]
              },
              {
                "address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
                "data": "0x",
                "topics": [
                  "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
                  "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "0x0000000000000000000000000000000000000000000000000000000000000000"
                ]
              },
              {
                "address": "0x36bcf57291a291a6e0e0bff7b12b69b556bcd9ed",
                "data": "0x",
                "topics": [
                  "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
                  "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "0x0000000000000000000000000000000000000000000000000000000000000000"
                ]
              }
            ]
          }
        }))
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_processes_orderflow() -> Result<()> {
        let client = get_test_ws_client().await?;

        // data from an actual juicy event
        let juicy_event: EventHistory = get_juicy_event();
        println!("juicy_event: {:#?}", juicy_event);
        let juicy_tx = get_juicy_tx();
        println!("juicy_tx: {:#?}", juicy_tx);

        let event_map = [juicy_event]
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();
        let test_db = Db::new(DbEngine::Mongo(MongoConfig::default())).await;

        // run the sim, it will save a result to the "test" DB
        client
            .simulate_arbs(
                &vec![juicy_tx.to_owned()],
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
            .contains(&juicy_tx.hash));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_finds_juicy_arb() -> Result<()> {
        let client = get_test_ws_client().await?;
        let juicy_tx = get_juicy_tx();
        let mut event_map = H256Map::new();
        event_map.insert(juicy_tx.hash, get_juicy_event());
        let res = client.backrun_tx(juicy_tx, &event_map).await?;
        println!("res: {:#?}", res);
        assert!(res.max_profit > 0.into());
        Ok(())
    }
}
