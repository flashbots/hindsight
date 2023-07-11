use ethers::types::Transaction;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};

use crate::data::write_txs;
use crate::info;
use crate::scanner::fetch_latest_events;
use crate::util::{fetch_txs, get_ws_client, WsClient};
use crate::{data, Result};

async fn fetch_and_write_txs(
    client: &WsClient,
    events: &Vec<EventHistory>,
    filename: Option<String>,
) -> anyhow::Result<Vec<Transaction>> {
    let cached_txs = fetch_txs(client, events).await?;
    write_txs(filename, &cached_txs).await?;
    Ok(cached_txs)
}

/// params.{offset, limit} are ignored; this function automagically fetches all events in your specified range.
pub async fn run(params: EventHistoryParams, filename: Option<String>) -> Result<()> {
    let event_client = EventClient::default();
    let events = fetch_latest_events(&event_client, params).await?;
    let ws_client = get_ws_client(None).await?;
    info!("Found {} events", events.len());

    // save events to file
    data::write_events(&events, filename.to_owned()).await?;

    // fetch txs for events, save to file
    fetch_and_write_txs(&ws_client, &events, filename).await?;

    Ok(())
}
