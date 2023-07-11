use crate::{
    config::Config,
    data::{read_events, read_txs, write_txs},
    sim::processor::{simulate_backrun, H256Map},
    util::{fetch_txs, get_ws_client, WsClient},
};
use ethers::types::Transaction;
use mev_share_sse::EventHistory;

#[derive(Debug)]
pub struct HindsightFactory {}

#[derive(Clone, Debug)]
pub struct Hindsight {
    pub client: WsClient,
    pub cache_events: Vec<EventHistory>,
    pub cache_txs: Vec<Transaction>,
    pub event_map: H256Map<EventHistory>,
}

impl HindsightFactory {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn init(self, config: Config) -> anyhow::Result<Hindsight> {
        let client = get_ws_client(Some(config.rpc_url_ws.to_owned())).await?;
        let cache_events = read_events(None).await?;
        println!("cache events: {:?}", cache_events.len());
        let event_map = cache_events
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();
        let cache_txs = read_txs(None).await?;
        // let cache_txs = if let Ok(cache_txs) = cache_txs {
        //     println!("found {} cached txs", cache_txs.len());
        //     cache_txs
        // } else {
        //     println!("Fetching event txs from RPC provider...");
        //     fetch_and_write_txs(&client.clone(), &cache_events, None).await?
        // };

        Ok(Hindsight {
            client,
            cache_events,
            cache_txs,
            event_map,
        })
    }
}

impl Hindsight {
    pub async fn process_orderflow(
        self,
        txs: Option<Vec<Transaction>>,
        batch_size: usize,
    ) -> anyhow::Result<()> {
        println!("processing orderflow");
        let txs = if let Some(txs) = txs {
            txs
        } else {
            self.cache_txs
        };
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
            println!("processing {} txs", txs_batch.len());
            for tx in txs_batch {
                let client = self.client.clone();
                let event_map = self.event_map.clone();
                handlers.push(tokio::spawn(async move {
                    simulate_backrun(&client, tx, event_map).await
                }));
            }
            for handler in handlers {
                let res = handler.await?;
                println!("res: {:?}", res);
            }
        }
        // simulate_backrun(&self.client, txs, self.event_map).await?;
        Ok(())
    }
}
