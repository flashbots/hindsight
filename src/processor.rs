use crate::{
    data::arbs::ArbDb,
    info,
    sim::processor::{simulate_backrun_arbs, H256Map},
    util::{get_ws_client, WsClient},
    Result,
};
use ethers::types::Transaction;
use futures::future;
use mev_share_sse::EventHistory;

#[derive(Clone, Debug)]
pub struct Processor {
    pub client: WsClient,
}

impl Processor {
    pub async fn new(rpc_url_ws: String) -> Result<Self> {
        let client = get_ws_client(Some(rpc_url_ws)).await?;
        Ok(Self { client })
    }
    /// Process all transactions in `txs` taking `batch_size` at a time to run
    /// in parallel.
    ///
    /// Saves results into DB after each batch.
    pub async fn process_orderflow(
        self,
        txs: &Vec<Transaction>,
        batch_size: usize,
        connect: Option<Box<ArbDb>>,
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
                let client = self.client.clone();
                handlers.push(tokio::task::spawn(async move {
                    simulate_backrun_arbs(&client, tx, &event_map).await.ok()
                }));
            }
            let results = future::join_all(handlers).await;
            let results = results
                .into_iter()
                .filter(|res| res.is_ok())
                .map(|res| res.unwrap())
                .filter(|res| res.is_some())
                .map(|res| res.unwrap())
                .collect::<Vec<_>>();
            info!("batch results: {:#?}", results);
            if let Some(db) = connect.to_owned() {
                // can't do && with a `let` in the conditional
                if !results.is_empty() {
                    db.to_owned().write_arbs(&results).await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ethers::{providers::Middleware, types::H256};
    use serde_json::json;

    use crate::{config::Config, data::arbs::ArbFilterParams};

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_processes_orderflow() -> Result<()> {
        let config = Config::default();
        let hindsight = Processor::new(config.rpc_url_ws).await?;

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
        let juicy_tx = get_ws_client(None)
            .await?
            .get_transaction(juicy_tx_hash)
            .await?
            .expect("failed to find juicy tx on chain");
        let event_map = vec![juicy_event]
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();
        let test_db = Box::new(ArbDb::new(Some("test".to_owned())).await?);

        // run the sim, it will save a result to the "test" DB
        hindsight
            .process_orderflow(
                vec![juicy_tx].as_ref(),
                1,
                Some(test_db.to_owned()),
                event_map,
            )
            .await?;

        // check DB for result
        let arbs = test_db.read_arbs(ArbFilterParams::none()).await?;
        assert!(arbs
            .into_iter()
            .map(|arb| arb.event.hint.hash)
            .collect::<Vec<_>>()
            .contains(&juicy_tx_hash));
        Ok(())
    }
}
