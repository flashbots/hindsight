use crate::data::io::file::read_file;
use crate::info;
use crate::Result;
use ethers::types::Transaction;
use tokio::fs;

const DEFAULT_FILENAME: &'static str = "txs.json";

pub async fn read_txs(filename: Option<String>) -> Result<Vec<ethers::types::Transaction>> {
    let filename = filename.unwrap_or(DEFAULT_FILENAME.to_owned());
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
    let filename = filename.unwrap_or(DEFAULT_FILENAME.to_owned());
    // open file for writing, then write the data to the file
    fs::write(filename.to_owned(), serde_json::to_string_pretty(txs)?).await?;
    info!("Wrote {} txs to {}", txs.len(), filename);
    Ok(())
}
