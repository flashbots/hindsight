use anyhow::Result;
use ethers::providers::Middleware;
use ethers::types::{AccountDiff, BlockNumber, H160};
use std::collections::BTreeMap;

use crate::util::WsClient;
use rusty_sando::{forked_db::fork_factory::ForkFactory, utils::state_diff};

pub async fn sim_bundle(signed_txs: Vec<String>, client: WsClient) -> Result<()> {
    let block_num = BlockNumber::Number(client.get_block_number().await?);
    let fork_block = Some(ethers::types::BlockId::Number(block_num));
    // let mut backend = GlobalBackend::new();

    println!("bundle: {:?}", signed_txs);

    let state_diffs = if let Some(sd) = state_diff::get_from_txs(&client, &vec![], block_num).await
    {
        sd
    } else {
        // panic!("no state diff found");
        BTreeMap::<H160, AccountDiff>::new()
    };
    let initial_db = state_diff::to_cache_db(&state_diffs, fork_block, &client)
        .await
        .unwrap();
    let fork_factory = ForkFactory::new_sandbox_factory(client.clone(), initial_db, fork_block);

    // do something ...?

    // prep vars for new thread
    let state_diffs = state_diffs.clone();
    let mut fork_factory = fork_factory.clone();

    // TODO: return something useful
    Ok(())
}
