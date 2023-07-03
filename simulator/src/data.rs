use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use tokio::fs;

//*******************************************//
// TODO: replace serde types w/ mev-share-rs //
//*******************************************//

#[derive(Deserialize, Debug)]
pub struct MevShareTx {
    pub to: Option<ethers::types::Address>,
    #[serde(rename(deserialize = "callData"))]
    pub calldata: Option<ethers::types::Bytes>,
    #[serde(rename(deserialize = "functionSelector"))]
    pub function_selector: Option<ethers::types::Bytes>,
}

#[derive(Deserialize, Debug)]
pub struct HistoricalEventHint {
    pub txs: Option<Vec<MevShareTx>>,
    pub hash: ethers::types::H256,
    pub logs: Vec<ethers::types::Log>,
    #[serde(rename(deserialize = "gasUsed"))]
    pub gas_used: u64,
    #[serde(rename(deserialize = "mevGasPrice"))]
    pub mev_gas_price: u64,
}

#[derive(Deserialize, Debug)]
pub struct HistoricalEvent {
    pub block: u64,
    pub timestamp: u64,
    pub hint: HistoricalEventHint,
}

#[derive(Deserialize, Debug)]
pub struct CachedEvents {
    pub events: Vec<HistoricalEvent>,
}

async fn read_file<'de, T: serde::de::DeserializeOwned>(filename: String) -> Result<T> {
    let raw_data = fs::read_to_string(filename).await?;
    let s = Arc::new(raw_data.as_str());
    let data: T = serde_json::from_str(&s)?;
    Ok(data)
}

pub async fn read_events(filename: Option<String>) -> Result<CachedEvents> {
    let filename = filename.unwrap_or("events.json".to_string());
    read_file(filename).await
}

pub async fn read_txs(filename: Option<String>) -> Result<Vec<ethers::types::Transaction>> {
    let filename = filename.unwrap_or("txs.json".to_string());
    read_file(filename).await
}

pub async fn write_tx_data(filename: Option<String>, data: String) -> Result<()> {
    let filename = filename.unwrap_or("txs.json".to_string());
    // open file for writing, then write the data to the file
    fs::write(filename, data).await?;
    Ok(())
}
