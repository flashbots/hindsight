use std::collections::BTreeMap;

use ethers::providers::Middleware;
use ethers::types::{AccountDiff, BlockNumber, H160};
use rusty_sando::forked_db::fork_factory::ForkFactory;
use rusty_sando::utils::state_diff;
use simulator::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let client = simulator::util::get_ws_provider(config.rpc_url_ws.to_owned()).await?;
    let block_num = BlockNumber::Number(client.get_block_number().await?);
    let fork_block = Some(ethers::types::BlockId::Number(block_num));
    // let mut backend = GlobalBackend::new();

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

    println!(
        "oohh geeez\nauth signer\t{:?}\nrpc url\t\t{:?}",
        config.auth_signer_key, config.rpc_url_ws
    );

    // prep vars for new thread
    let state_diffs = state_diffs.clone();
    let mut fork_factory = fork_factory.clone();

    Ok(())
}
