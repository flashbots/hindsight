use crate::{
    config::Config,
    data::Db,
    info,
    sim::processor::{simulate_backrun_arbs, H256Map},
    util::{get_ws_client, WsClient},
    Result,
};
use ethers::types::Transaction;
use futures::future;
use mev_share_sse::{EventHistory, EventHistoryParams};

mod factory {
    #[derive(Debug)]
    pub struct HindsightFactory {}
}
use factory::HindsightFactory;

#[derive(Clone, Debug)]
pub struct Hindsight {
    pub client: WsClient,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub block_start: Option<u64>,
    pub block_end: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    /// for saving
    pub filename_txs: Option<String>,
    pub filename_events: Option<String>,
    pub batch_size: Option<usize>,
}

impl Into<EventHistoryParams> for ScanOptions {
    fn into(self) -> EventHistoryParams {
        EventHistoryParams {
            block_start: self.block_start,
            block_end: self.block_end,
            timestamp_start: self.timestamp_start,
            timestamp_end: self.timestamp_end,
            limit: None,
            offset: None,
        }
    }
}

#[derive(Debug)]
pub struct LoadOptions {
    /// for loading
    pub filename: Option<String>,
}

#[derive(Debug)]
pub enum HindsightOptions {
    Scan(ScanOptions),
    Load(LoadOptions),
}

impl HindsightFactory {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn init(self, config: Config) -> Result<Hindsight> {
        let client = get_ws_client(Some(config.rpc_url_ws.to_owned())).await?;
        Ok(Hindsight { client })
    }
}

impl Hindsight {
    pub fn new() -> HindsightFactory {
        HindsightFactory::new()
    }
    /// Process all transactions in `txs` taking `batch_size` at a time to run
    /// in parallel.
    ///
    /// Saves results into DB after each batch.
    pub async fn process_orderflow(
        self,
        txs: &Vec<Transaction>,
        batch_size: usize,
        db: Option<Box<Db>>,
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
                handlers.push(tokio::spawn(async move {
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
            if let Some(db) = db.to_owned() {
                db.to_owned().write_arbs(results).await?;
            }
        }
        Ok(())
    }
}
