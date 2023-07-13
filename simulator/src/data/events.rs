use crate::data::io::file::read_file;
use crate::info;
use crate::Result;
use mev_share_sse::EventHistory;
use tokio::fs;

const DEFAULT_FILENAME: &'static str = "events.json";

pub async fn write_events(events: &Vec<EventHistory>, filename: Option<String>) -> Result<()> {
    let filename = filename.unwrap_or(DEFAULT_FILENAME.to_string());
    fs::write(filename.to_owned(), serde_json::to_string_pretty(&events)?).await?;
    info!("Wrote {} events to {}", events.len(), filename);
    Ok(())
}

pub async fn read_events(filename: Option<String>) -> Result<Vec<EventHistory>> {
    let filename = filename.unwrap_or("events.json".to_string());
    let res = read_file(filename.to_owned()).await;
    if let Err(e) = res {
        return Err(anyhow::anyhow!(
            "failed to read cache events from {}: {:?}\nPlease run the `scan` command.",
            filename,
            e
        ));
    };
    res
}
