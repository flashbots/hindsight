use crate::{
    config::Config,
    data::{
        self,
        events::read_events,
        txs::{read_txs, write_txs},
        Db,
    },
    info,
    scanner::fetch_latest_events,
    sim::processor::{simulate_backrun_arbs, H256Map},
    util::{fetch_txs, get_ws_client, WsClient},
    Result,
};
use ethers::types::Transaction;
use futures::future;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};

mod factory {
    #[derive(Debug)]
    pub struct HindsightFactory {}
}
use factory::HindsightFactory;

#[derive(Clone, Debug)]
pub struct Hindsight {
    pub client: WsClient,
    pub cache_events: Vec<EventHistory>,
    pub cache_txs: Vec<Transaction>,
    pub event_map: H256Map<EventHistory>,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub start_block: Option<u64>,
    pub end_block: Option<u64>,
    pub start_timestamp: Option<u64>,
    pub end_timestamp: Option<u64>,
    /// for saving
    pub filename: Option<String>,
}

impl Into<EventHistoryParams> for ScanOptions {
    fn into(self) -> EventHistoryParams {
        EventHistoryParams {
            block_start: self.start_block,
            block_end: self.end_block,
            timestamp_start: self.start_timestamp,
            timestamp_end: self.end_timestamp,
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

async fn fetch_and_write_txs(
    client: &WsClient,
    events: &Vec<EventHistory>,
    filename: Option<String>,
) -> anyhow::Result<Vec<Transaction>> {
    let cached_txs = fetch_txs(client, events).await?;
    write_txs(filename, &cached_txs).await?;
    Ok(cached_txs)
}

impl HindsightFactory {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn init(
        self,
        config: Config,
        procedure_options: HindsightOptions,
    ) -> Result<Hindsight> {
        let client = get_ws_client(Some(config.rpc_url_ws.to_owned())).await?;

        match procedure_options {
            HindsightOptions::Scan(options) => {
                // sdasd
                let event_client = EventClient::default();
                let events = fetch_latest_events(&event_client, options.clone().into()).await?;
                info!("Found {} events", events.len());
                // save events to file
                data::events::write_events(&events, options.filename.to_owned()).await?;
                // fetch txs for events, save to file
                let cache_txs = fetch_and_write_txs(&client, &events, options.filename).await?;
                Ok(Hindsight {
                    client,
                    cache_events: events,
                    cache_txs,
                    event_map: H256Map::new(),
                })
            }
            HindsightOptions::Load(options) => {
                let cache_events = read_events(options.filename.clone()).await?;
                let event_map = cache_events
                    .iter()
                    .map(|event| (event.hint.hash, event.to_owned()))
                    .collect::<H256Map<EventHistory>>();
                let cache_txs = read_txs(options.filename).await?;

                Ok(Hindsight {
                    client,
                    cache_events,
                    cache_txs,
                    event_map,
                })
            }
        }
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
        txs: Option<Vec<Transaction>>,
        batch_size: usize,
        db: Option<Box<Db>>,
    ) -> Result<()> {
        let txs = if let Some(txs) = txs {
            txs
        } else {
            self.cache_txs
        };
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
                let client = self.client.clone();
                let event_map = self.event_map.clone();
                handlers.push(tokio::spawn(async move {
                    simulate_backrun_arbs(&client, tx, event_map).await.ok()
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
