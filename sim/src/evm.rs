use crate::util::ETH_DEV_ADDRESS;
use ethers::types::{Address, Bytes as BytesEthers, Transaction, TransactionRequest, U256, U64};
use hindsight_core::{
    debug, err,
    error::HindsightError,
    evm::{fork_db::ForkDB, fork_factory::ForkFactory},
    util::u256_to_ru256,
    warn, Result,
};
use revm::{
    primitives::keccak256 as rkeccak256,
    primitives::{
        AccountInfo, Address as rAddress, Bytecode, Bytes, ExecutionResult, Output, ResultAndState,
        TransactTo, U256 as rU256,
    },
    EVM,
};
use std::str::FromStr;

pub fn inject_contract(
    db: &mut ForkFactory, // TODO: generalize this to `impl Database`
    address: Address,
    bytecode: Bytes,
    deployed_bytecode: Bytes,
) {
    // calculate bytecode hash
    let bytecode_hash = rkeccak256(&bytecode);

    // instantiate account at given address
    let account = AccountInfo::new(
        rU256::ZERO,
        0,
        bytecode_hash,
        Bytecode::new_raw(deployed_bytecode),
    );

    // inject contract code into db
    db.insert_account_info(address.0.into(), account);
}

/// Calls a function specified by `function_selector` assuming it has no args.
/// Calls from `ETH_DEV_ADDRESS` to `contract`.
pub fn call_function(evm: &mut EVM<ForkDB>, fn_selector: &str, contract: Address) -> Result<Bytes> {
    debug!("calling function {:?}", fn_selector);
    let tx: TransactionRequest = TransactionRequest {
        from: Some(*ETH_DEV_ADDRESS),
        to: Some(contract.into()),
        gas: Some(U256::from(90_000_u64)),
        gas_price: Some(evm.env.block.basefee.to_be_bytes().into()),
        value: None,
        data: Some(BytesEthers::from_str(fn_selector)?),
        nonce: None,
        chain_id: Some(U64::from(1)),
    };
    sim_tx_request(evm, tx)
}

pub fn sim_tx_request(evm: &mut EVM<ForkDB>, tx: TransactionRequest) -> Result<Bytes> {
    evm.env.tx.caller = tx.from.unwrap_or(*ETH_DEV_ADDRESS).0.into();
    evm.env.tx.transact_to = TransactTo::Call(rAddress::from(
        tx.to
            .to_owned()
            .ok_or(HindsightError::EvmParseError(format!(
                "tx.to invalid ({:?})",
                tx.to
            )))?
            .as_address()
            .ok_or(
                // TODO: find cleaner way to do this
                HindsightError::EvmParseError(format!(
                    "tx.to could not parse address ({:?})",
                    tx.to
                )),
            )?
            .0,
    ));
    evm.env.tx.data = tx
        .data
        .to_owned()
        .ok_or(HindsightError::EvmParseError(format!(
            "tx.data invalid ({:?})",
            tx.data
        )))?
        .0
        .into();
    evm.env.tx.value = tx.value.map(u256_to_ru256).unwrap_or_default();
    evm.env.tx.gas_price = tx.gas_price.map(u256_to_ru256).unwrap_or_default();
    evm.env.tx.gas_limit = tx.gas.unwrap_or_default().as_u64();
    let res = match evm.transact_ref() {
        Ok(res) => res.result,
        Err(err) => {
            warn!("failed to simulate tx request: {:?}", err);
            return err!("failed to simulate tx request: {:?}", err);
        }
    };
    let output: Bytes = match res {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(o) => o.into(),
            Output::Create(o, _) => o.into(),
        },
        ExecutionResult::Revert { output, .. } => {
            return err!("SimulationEvmRevertedError: {}", output)
        }
        ExecutionResult::Halt { reason, .. } => {
            return err!("SimulationEvmHaltedError: {:?}", reason)
        }
    };
    Ok(output)
}

fn inject_tx(evm: &mut EVM<ForkDB>, tx: &Transaction) -> Result<()> {
    evm.env.tx.caller = tx.from.0.into();
    evm.env.tx.transact_to = TransactTo::Call(rAddress::from(tx.to.unwrap_or_default().0));
    evm.env.tx.data = tx.input.to_owned().0.into();
    evm.env.tx.value = u256_to_ru256(tx.value);
    evm.env.tx.chain_id = tx.chain_id.map(|id| id.as_u64());
    evm.env.tx.gas_limit = tx.gas.as_u64();
    match tx.transaction_type {
        Some(ethers::types::U64([0])) => {
            evm.env.tx.gas_price = tx.gas_price.map(u256_to_ru256).unwrap_or_default();
        }
        Some(_) => {
            // type-2 tx
            evm.env.tx.gas_priority_fee = tx.max_priority_fee_per_gas.map(u256_to_ru256);
            evm.env.tx.gas_price = tx.max_fee_per_gas.map(u256_to_ru256).unwrap_or_default();
        }
        None => {
            // legacy tx
            evm.env.tx.gas_price = tx.gas_price.map(u256_to_ru256).unwrap_or_default();
        }
    }
    Ok(())
}

/// Simulate a bundle of transactions, commiting each tx to the EVM's ForkDB.
///
/// Returns array containing each tx's simulation result.
pub async fn sim_bundle(
    evm: &mut EVM<ForkDB>,
    signed_txs: Vec<Transaction>,
) -> Result<Vec<ExecutionResult>> {
    let mut results = vec![];
    for tx in signed_txs {
        let res = commit_tx(evm, tx).await;
        if let Ok(res) = res {
            results.push(res.to_owned());
        }
    }

    Ok(results)
}

/// Execute a transaction on the forked EVM, commiting its state changes to the EVM's ForkDB.
pub async fn commit_tx(evm: &mut EVM<ForkDB>, tx: Transaction) -> Result<ExecutionResult> {
    inject_tx(evm, &tx)?;
    let res = evm.transact_commit();
    res.map_err(|err| {
        hindsight_core::anyhow::anyhow!("failed to simulate tx {:?}: {:?}", tx.hash, err)
    })
}

pub async fn call_tx(evm: &mut EVM<ForkDB>, tx: Transaction) -> Result<ResultAndState> {
    inject_tx(evm, &tx)?;
    let res = evm.transact();
    res.map_err(|err| {
        hindsight_core::anyhow::anyhow!("failed to simulate tx {:?}: {:?}", tx.hash, err)
    })
}
