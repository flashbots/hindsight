use mev_share_sse::EventClient;

use crate::scanner::fetch_latest_events;
use crate::Result;

pub async fn run(start_block: Option<u64>, start_timestamp: Option<u64>) -> Result<()> {
    let client = EventClient::default();
    let events = fetch_latest_events(&client, start_block, start_timestamp).await?;
    println!("Found {} events", events.len());
    Ok(())
}
