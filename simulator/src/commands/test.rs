use ethers::{providers::Middleware, types::H256};

use crate::hindsight::Hindsight;
use crate::Result;

pub async fn run(hindsight: Hindsight, batch_size: Option<usize>) -> Result<()> {
    let juicy_tx_hash: H256 =
        "0x85452de30c2cd7da3efdb0034fe928642ae67ac3ea508df1c563da2f5ed38044".parse::<H256>()?;
    println!("juicy_tx_hash: {:?}", juicy_tx_hash);
    let juicy_tx = hindsight
        .client
        .get_transaction(juicy_tx_hash)
        .await?
        .unwrap();
    println!("juicy_tx: {:?}", juicy_tx);

    hindsight
        .process_orderflow(Some(vec![juicy_tx]), batch_size.unwrap_or(1))
        .await?;
    Ok(())
}
