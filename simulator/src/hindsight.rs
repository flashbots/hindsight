use crate::{
    config::Config,
    data::{read_events, read_txs, write_tx_data, CachedEvents, HistoricalEvent},
    sim::processor::{process_orderflow, H256Map},
    util::{fetch_txs, get_ws_client, WsClient},
};
use ethers::types::Transaction;

#[derive(Debug)]
pub struct HindsightFactory {}

#[derive(Clone, Debug)]
pub struct Hindsight {
    pub client: WsClient,
    pub cache_events: CachedEvents,
    pub cache_txs: Vec<Transaction>,
    pub event_map: H256Map<HistoricalEvent>,
}

impl HindsightFactory {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn init(self, config: Config) -> anyhow::Result<Hindsight> {
        let client = get_ws_client(config.rpc_url_ws.to_owned()).await?;
        let cache_events = read_events(None).await?;
        let event_map = cache_events
            .events
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<HistoricalEvent>>();
        let cache_txs = read_txs(None).await?;
        if cache_txs.len() == 0 {
            println!("no txs found in cache, quitting");
        }

        Ok(Hindsight {
            client,
            cache_events,
            cache_txs,
            event_map,
        })
    }
}

impl Hindsight {
    pub async fn fetch_txs(self, filename: Option<&str>) -> anyhow::Result<()> {
        println!("fetching txs");
        let cached_txs = fetch_txs(&self.client, self.cache_events.events).await?;
        write_tx_data(filename, serde_json::to_string_pretty(&cached_txs)?).await?;
        Ok(())
    }

    pub async fn process_orderflow(self, txs: Option<Vec<Transaction>>) -> anyhow::Result<()> {
        println!("processing orderflow");
        let txs = if let Some(txs) = txs {
            txs
        } else {
            self.cache_txs
        };
        process_orderflow(&self.client, txs, self.event_map).await?;
        Ok(())
    }
}
