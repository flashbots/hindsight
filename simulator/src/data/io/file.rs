use crate::Result;
use std::sync::Arc;
use tokio::fs;

const DEFAULT_FILENAME: &'static str = "events.json";

pub async fn read_file<'de, T: serde::de::DeserializeOwned>(filename: String) -> Result<T> {
    let raw_data = fs::read_to_string(filename).await?;
    let s = Arc::new(raw_data.as_str());
    let data: T = serde_json::from_str(&s)?;
    Ok(data)
}
