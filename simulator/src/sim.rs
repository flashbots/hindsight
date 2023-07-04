use anyhow::Result;
use ethers::providers::Middleware;
use ethers::types::{AccountDiff, BlockNumber, Transaction, H160};
use revm::primitives::{TransactTo, B160};
use revm::EVM;
use rusty_sando::simulate::setup_block_state;
use rusty_sando::types::BlockInfo;
use std::collections::BTreeMap;

use crate::util::WsClient;
use rusty_sando::{forked_db::fork_factory::ForkFactory, utils::state_diff};

pub async fn sim_bundle(
    client: &WsClient,
    signed_txs: Vec<Transaction>,
    next_block: &BlockInfo,
) -> Result<()> {
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

    // do something ...? rusty-sando evaluates target pools here, but we don't need to do that

    // prep vars for new thread (only relevant when we add sub-threads for each tx to calculate profit; we'll need a slightly modified version of this function to do that)
    // let state_diffs = state_diffs.clone();
    // let mut fork_factory = fork_factory.clone();

    let mut evm = EVM::new();
    evm.database(fork_factory.new_sandbox_fork());
    setup_block_state(&mut evm, next_block);

    // TODO: build transactions from signed_txs and push them to evm
    for tx in signed_txs {
        evm.env.tx.caller = B160::from(tx.from);
        evm.env.tx.transact_to = TransactTo::Call(B160::from(tx.to.unwrap_or_default().0));
        evm.env.tx.data = tx.input.0;
        evm.env.tx.value = tx.value.into();
        evm.env.tx.chain_id = tx.chain_id.map(|id| id.as_u64());
        evm.env.tx.nonce = Some(tx.nonce.as_u64());
        evm.env.tx.gas_limit = tx.gas.as_u64();
        match tx.transaction_type {
            Some(ethers::types::U64([0])) => {
                evm.env.tx.gas_price = tx.gas_price.unwrap_or_default().into();
            }
            Some(_) => {
                // type-2 tx
                evm.env.tx.gas_priority_fee = tx.max_priority_fee_per_gas.map(|fee| fee.into());
                evm.env.tx.gas_price = tx.max_fee_per_gas.unwrap_or_default().into();
            }
            None => {
                // legacy tx
                evm.env.tx.gas_price = tx.gas_price.unwrap_or_default().into();
            }
        }
        let _res = evm.transact_commit();
        println!("res: {:?}", _res);
    }

    // TODO: return something useful ((WHAT DO WE RETURN?))
    Ok(())
}
