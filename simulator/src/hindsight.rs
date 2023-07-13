use crate::{
    config::Config,
    data::{read_events, read_txs},
    info,
    sim::processor::{simulate_backrun, H256Map},
    util::{get_ws_client, WsClient},
    Result,
};
use ethers::types::Transaction;
use mev_share_sse::EventHistory;
use tracing::warn;

#[derive(Debug)]
pub struct HindsightFactory {}

#[derive(Clone, Debug)]
pub struct Hindsight {
    pub client: WsClient,
    pub cache_events: Vec<EventHistory>,
    pub cache_txs: Vec<Transaction>,
    pub event_map: H256Map<EventHistory>,
}

// pub struct HindsightOptions {
//     pub init_procedure: InitProcedure,
// }

// impl HindsightOptions {
//     pub fn new(init_procedure: InitProcedure) -> Self {
//         Self { init_procedure }
//     }

//     pub fn default() -> Self {
//         Self {
//             init_procedure: InitProcedure::Load(LoadOptions { filename: None }),
//         }
//     }
// }

#[derive(Debug)]
pub struct ScanOptions {
    pub start_block: Option<u64>,
    pub end_block: Option<u64>,
    pub start_timestamp: Option<u64>,
    pub end_timestamp: Option<u64>,
}

#[derive(Debug)]
pub struct LoadOptions {
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
    pub async fn init(
        self,
        config: Config,
        procedure_options: HindsightOptions,
    ) -> Result<Hindsight> {
        let client = get_ws_client(Some(config.rpc_url_ws.to_owned())).await?;

        match procedure_options {
            HindsightOptions::Scan(options) => {
                // sdasd
                let cache_events = vec![];
                let cache_txs = vec![];
                warn!("scan not implemented yet {:?}", options);
                Ok(Hindsight {
                    client,
                    cache_events,
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
    pub async fn process_orderflow(
        self,
        txs: Option<Vec<Transaction>>,
        batch_size: usize,
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
                    simulate_backrun(&client, tx, event_map).await
                }));
            }
            for handler in handlers {
                let res = handler.await?;
                info!("{:#?}", res);
            }
        }
        Ok(())
    }

    pub async fn scan_and_process_events(self, batch_size: usize) -> Result<()> {
        Ok(())
    }
}
