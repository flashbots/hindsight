use crate::hindsight::Hindsight;
use crate::Result;
use ethers::types::H256;

pub async fn run(hindsight: Hindsight, batch_size: Option<usize>) -> Result<()> {
    let juicy_tx_hash: H256 =
        "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a".parse::<H256>()?;
    let juicy_tx = hindsight
        .cache_txs
        .iter()
        .find(|tx| tx.hash == juicy_tx_hash)
        .expect("juicy tx not found in cache")
        .to_owned();

    hindsight
        .process_orderflow(Some(vec![juicy_tx]), batch_size.unwrap_or(1))
        .await?;
    Ok(())
}
