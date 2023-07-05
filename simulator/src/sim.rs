use anyhow::Result;
use ethers::types::{AccountDiff, Address, BlockNumber, Transaction, H160, U256};
use revm::primitives::{ExecutionResult, TransactTo, B160, U256 as rU256};
use revm::EVM;
use rusty_sando::prelude::fork_db::ForkDB;
use rusty_sando::prelude::PoolVariant;
use rusty_sando::simulate::{
    attach_braindance_module, braindance_address, braindance_controller_address, setup_block_state,
};
use rusty_sando::types::BlockInfo;
use rusty_sando::utils::tx_builder::braindance;
use std::collections::BTreeMap;

use crate::util::WsClient;
use rusty_sando::{forked_db::fork_factory::ForkFactory, utils::state_diff};

/// Return an evm instance forked from the provided block info and client state
/// with braindance module initialized.
pub async fn fork_evm(client: &WsClient, block_info: &BlockInfo) -> Result<EVM<ForkDB>> {
    let fork_block_num = BlockNumber::Number(block_info.number);
    let fork_block = Some(ethers::types::BlockId::Number(fork_block_num));

    let state_diffs =
        if let Some(sd) = state_diff::get_from_txs(&client, &vec![], fork_block_num).await {
            sd
        } else {
            BTreeMap::<H160, AccountDiff>::new()
        };
    let initial_db = state_diff::to_cache_db(&state_diffs, fork_block, &client)
        .await
        .unwrap();
    let mut fork_factory = ForkFactory::new_sandbox_factory(client.clone(), initial_db, fork_block);
    attach_braindance_module(&mut fork_factory);

    let mut evm = EVM::new();
    evm.database(fork_factory.new_sandbox_fork());
    setup_block_state(&mut evm, block_info);

    Ok(evm)
}

// pub async fn find_optimal_backrun(
//     client: &WsClient,
//     user_tx: Transaction,
//     block_info: &BlockInfo,
// ) -> Result<()> {
//     Ok(())
// }

/// Simulate a bundle of transactions, returns array containing each tx's simulation result.
pub async fn sim_bundle(
    evm: &mut EVM<ForkDB>,
    signed_txs: Vec<Transaction>,
) -> Result<Vec<ExecutionResult>> {
    let mut results = vec![];
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
        let res = evm.transact_commit();
        if let Ok(res) = res {
            results.push(res);
        } else {
            println!("failed to simulate transaction: {:?}", res);
        }
    }

    Ok(results)
}

pub fn commit_braindance_swap(
    evm: &mut EVM<ForkDB>,
    pool_variant: PoolVariant,
    amount_in: U256,
    target_pool: Address,
    startend_token: Address,
    intermediary_token: Address,
    base_fee: U256,
) -> Result<()> {
    let swap_data = match pool_variant {
        PoolVariant::UniswapV2 => braindance::build_swap_v2_data(
            amount_in,
            target_pool,
            startend_token,
            intermediary_token,
        ),
        PoolVariant::UniswapV3 => braindance::build_swap_v3_data(
            amount_in.as_u128().into(),
            target_pool,
            startend_token,
            intermediary_token,
        ),
    };

    evm.env.tx.caller = braindance_controller_address();
    evm.env.tx.transact_to = TransactTo::Call(braindance_address().0.into());
    evm.env.tx.data = swap_data.0;
    evm.env.tx.gas_limit = 700000;
    evm.env.tx.gas_price = base_fee.into();
    evm.env.tx.value = rU256::ZERO;

    let _ = evm.transact_commit();
    Ok(())
}
