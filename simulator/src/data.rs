use anyhow::Result;
use serde::Deserialize;
use tokio::fs;

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
pub struct CachedData {
    pub events: Vec<HistoricalEvent>,
}

pub async fn read_data(filename: Option<String>) -> Result<CachedData> {
    let filename = filename.unwrap_or("cache.json".to_string());
    let raw_data = fs::read_to_string(filename).await?;
    Ok(serde_json::from_str(&raw_data)?)
}
