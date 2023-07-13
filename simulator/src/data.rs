use crate::info;
use crate::Result;
use ethers::types::Transaction;
use mev_share_sse::EventHistory;
use std::sync::Arc;
use tokio::fs;

const DEFAULT_FILENAME: &'static str = "events.json";

async fn read_file<'de, T: serde::de::DeserializeOwned>(filename: String) -> Result<T> {
    let raw_data = fs::read_to_string(filename).await?;
    let s = Arc::new(raw_data.as_str());
    let data: T = serde_json::from_str(&s)?;
    Ok(data)
}

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

pub async fn read_txs(filename: Option<String>) -> Result<Vec<ethers::types::Transaction>> {
    let filename = filename.unwrap_or("txs.json".to_string());
    let res = read_file(filename.to_owned()).await;
    if let Err(e) = res {
        return Err(anyhow::format_err!(
            "failed to read txs from file {}: {:?}\nPlease run the `scan` command.",
            filename,
            e
        ));
    }
    res
}

pub async fn write_txs(filename: Option<String>, txs: &Vec<Transaction>) -> Result<()> {
    let filename = filename.unwrap_or("txs.json".to_owned());
    // open file for writing, then write the data to the file
    fs::write(filename.to_owned(), serde_json::to_string_pretty(txs)?).await?;
    info!("Wrote {} txs to {}", txs.len(), filename);
    Ok(())
}
