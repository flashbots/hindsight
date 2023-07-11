use crate::Result;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};

const FLASHBOTS_EVENTS_API_URL: &'static str = "https://mev-share.flashbots.net/api/v1";
fn event_history_info_url() -> String {
    format!("{}/{}", FLASHBOTS_EVENTS_API_URL, "history/info")
}
fn event_history_url() -> String {
    format!("{}/{}", FLASHBOTS_EVENTS_API_URL, "history")
}

pub async fn fetch_latest_events(
    client: &EventClient,
    block_start: Option<u64>,
    timestamp_start: Option<u64>,
) -> Result<Vec<EventHistory>> {
    let info = client.event_history_info(&event_history_info_url()).await?;
    let mut current_offset = 0;
    let mut done = false;
    let mut events = vec![];
    while !done {
        let mut chunk = client
            .event_history(
                &event_history_url(),
                EventHistoryParams {
                    block_start,
                    block_end: None,
                    timestamp_start,
                    timestamp_end: None,
                    limit: Some(info.max_limit),
                    offset: Some(current_offset),
                },
            )
            .await?;
        let chunk_len = chunk.len() as u64;
        let limit = info.max_limit;
        current_offset += chunk_len;
        events.append(&mut chunk);
        done = chunk_len < limit;
        println!(
            "Fetched {} events ({} events total)",
            chunk_len,
            events.len()
        );
    }
    Ok(events)
}
