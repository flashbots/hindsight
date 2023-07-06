use anyhow::Result;
use ethers::{
    prelude::abigen,
    providers::{Middleware, Provider, Ws},
    types::{Address, Transaction, H256},
};
use futures::future;
use rusty_sando::types::BlockInfo;
use std::sync::Arc;

use crate::data::HistoricalEvent;

pub type WsClient = Arc<Provider<Ws>>;

pub async fn get_ws_client(rpc_url: String) -> Result<WsClient> {
    let provider = Provider::<Ws>::connect(rpc_url).await?;
    Ok(Arc::new(provider))
}

pub async fn fetch_txs(
    client: &WsClient,
    events: Vec<HistoricalEvent>,
) -> Result<Vec<Transaction>> {
    let tx_hashes: Vec<H256> = events
        .into_iter()
        .map(|e: HistoricalEvent| e.hint.hash)
        .collect();
    let mut full_txs = vec![];
    let mut handles: Vec<_> = vec![];

    for tx_hash in tx_hashes.into_iter() {
        let client = client.clone();
        handles.push(tokio::spawn(future::lazy(move |_| async move {
            let tx = &client.get_transaction(tx_hash.to_owned()).await;
            if let Ok(tx) = tx {
                println!("tx found: {:?}", tx_hash.to_owned());
                if let Some(tx) = tx {
                    return Some(tx.clone());
                } else {
                    println!("tx not found: {:?}", tx_hash.to_owned());
                    None
                }
            } else {
                println!("error fetching tx: {:?}", tx);
                None
            }
        })));
    }

    for handle in handles.into_iter() {
        let tx = handle.await?.await;
        if let Some(tx) = tx {
            full_txs.push(tx);
        }
    }

    Ok(full_txs.to_vec())
}

pub async fn get_pair_tokens(client: &WsClient, pair: Address) -> Result<(Address, Address)> {
    abigen!(
        IPairTokens,
        r#"[
            function token0() external view returns (address)
            function token1() external view returns (address)
        ]"#
    );
    let contract = IPairTokens::new(pair, client.clone());
    let token0 = contract.token_0().call().await?;
    let token1 = contract.token_1().call().await?;
    Ok((token0, token1))
}

pub async fn get_block_info(client: &WsClient, block_num: u64) -> Result<BlockInfo> {
    let block = client
        .get_block(block_num)
        .await?
        .ok_or(anyhow::format_err!("failed to get block {:?}", block_num))?;
    Ok(BlockInfo {
        number: block_num.into(),
        timestamp: block.timestamp,
        base_fee: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
    })
}
