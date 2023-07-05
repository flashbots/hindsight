use anyhow::Result;
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{Transaction, H256},
};
use futures::future;
use std::sync::Arc;

use crate::data::HistoricalEvent;

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

pub type WsClient = Arc<Provider<Ws>>;
