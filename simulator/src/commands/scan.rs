use crate::config::Config;
use crate::data::arbs::ArbDb;
use crate::hindsight::{Hindsight, ScanOptions};
use crate::info;

use crate::scanner::event_history_url;
use crate::sim::processor::H256Map;
use crate::util::{fetch_txs, get_ws_client};
use crate::Result;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};

pub async fn run(params: ScanOptions, config: Config) -> Result<()> {
    info!(
        "scanning events starting at block={}",
        params.block_start.unwrap_or(0)
    );
    let ws_client = get_ws_client(None).await?;
    let mevshare = EventClient::default();
    let hindsight = Hindsight::new().init(config).await?;
    let mut done = false;
    let mut event_params: EventHistoryParams = params.clone().into();

    let batch_size = params.batch_size.unwrap_or(5);
    event_params.limit = Some(batch_size as u64);
    event_params.offset = Some(0);

    let db = ArbDb::new(None).await?;
    info!("batch size: {}", batch_size);
    while !done {
        // fetch events
        let events = mevshare
            .event_history(&event_history_url(), event_params.to_owned())
            .await?;
        let txs = fetch_txs(&ws_client, &events).await?;
        info!("found {} events", events.len());
        // process arbs
        let event_map = events
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();
        hindsight
            .to_owned()
            .process_orderflow(&txs, batch_size, Some(Box::new(db.to_owned())), event_map)
            .await?;
        info!("simulated arbs for {} transactions", txs.len());
        event_params.offset = Some(event_params.offset.unwrap() + events.len() as u64);
        println!("offset: {}", event_params.offset.unwrap());
        done = events.len() < event_params.limit.unwrap_or(500) as usize;
    }

    Ok(())
}
