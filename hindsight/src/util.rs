use crate::{config::Config, info, Result};
use data::interfaces::{BlockInfo, PairPool, PoolVariant};
use ethers::{
    prelude::{abigen, H160},
    providers::{Middleware, Provider, Ws},
    types::{transaction::eip2718::TypedTransaction, Address, Transaction, H256, U256},
};
use futures::future;
use mev_share_sse::EventHistory;
use std::sync::Arc;

pub use ethers::utils::WEI_IN_ETHER as ETH;

/*
    TODO: break out the remaining contents of this file; they don't relate to each other.
    Then get rid of this module.
    A lot of this could go in a wallet wrapper.
*/

pub async fn fetch_txs(client: &WsClient, events: &[EventHistory]) -> Result<Vec<Transaction>> {
    let tx_hashes: Vec<H256> = events.iter().map(|e: &EventHistory| e.hint.hash).collect();
    let mut handles = vec![];

    for tx_hash in tx_hashes.into_iter() {
        let client = client.clone();
        handles.push(tokio::task::spawn(async move {
            let tx = &client.get_transaction(tx_hash.to_owned()).await;
            if let Ok(tx) = tx {
                if let Some(tx) = tx {
                    info!("tx found onchain\t{:?}", tx_hash.to_owned());
                    Some(tx.clone())
                } else {
                    info!("tx not found onchain\t{:?}", tx_hash.to_owned());
                    None
                }
            } else {
                info!("error fetching tx: {:?}", tx);
                None
            }
        }));
    }
    let results = future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect::<Vec<_>>();
    Ok(results)
}

pub async fn get_balance_call(
    client: &WsClient,
    token: Address,
    account: Address,
) -> Result<TypedTransaction> {
    abigen!(
        IERC20,
        r#"[
            function balanceOf(address account) external view returns (uint256)
        ]"#
    );
    let contract = IERC20::new(token, client.clone());
    Ok(contract.balance_of(account).tx)
}

pub fn filter_events_by_topic(
    events: &[EventHistory],
    filter_topics: &[H256],
) -> Vec<EventHistory> {
    events
        .iter()
        .filter(|event| {
            event
                .hint
                .logs
                .iter()
                .map(|log| log.topics.to_owned())
                .any(|topics| {
                    topics
                        .iter()
                        .map(|topic| filter_topics.contains(topic))
                        .reduce(|a, b| a || b)
                        .unwrap_or(false)
                })
        })
        .map(|e| e.to_owned())
        .collect::<Vec<_>>()
}
