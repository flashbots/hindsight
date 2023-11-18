use crate::util::{get_price_v3, BRAINDANCE_ADDR, CONTROLLER_ADDR, ETH_DEV_ACCOUNT};
use ethers::{
    abi::{self, AbiDecode, AbiEncode, ParamType},
    prelude::abigen,
    types::{Address, Bytes as BytesEthers, Transaction, TransactionRequest, I256, U256, U64},
};
use foundry_contracts::brain_dance::{
    BrainDanceCalls, CalculateSwapV2Call, CalculateSwapV2Return, CalculateSwapV3Call,
    CalculateSwapV3Return,
};
use hindsight_core::{
    debug, err, error::HindsightError, evm::fork_db::ForkDB, interfaces::PoolVariant, Error, Result,
};
use revm::{
    db::CacheDB,
    primitives::{
        AccountInfo, Address as rAddress, Bytecode, Bytes, ExecutionResult, Output, ResultAndState,
        TransactTo, B256, U256 as rU256,
    },
    EVM,
};
use std::{ops::Mul, str::FromStr};

pub fn inject_contract<T: revm::db::DatabaseRef>(
    db: &mut CacheDB<T>,
    address: Address,
    bytecode: Bytecode,
    bytecode_hash: B256,
) -> Result<()> {
    // instantiate account at given address
    let account = AccountInfo::new(rU256::ZERO, 0, bytecode_hash, bytecode);

    // inject contract code into db
    db.insert_account_info(address.0.into(), account);
    Ok(())
}

/// Execute a braindance swap on the forked EVM, commiting its state changes to the EVM's ForkDB.
///
/// Returns balance of token_out after tx is executed.
#[allow(clippy::too_many_arguments)]
pub fn commit_braindance_swap(
    evm: &mut EVM<ForkDB>,
    pool_variant: PoolVariant,
    amount_in: U256,
    target_pool: Address,
    token_in: Address,
    token_out: Address,
    base_fee: U256,
    _nonce: Option<u64>,
) -> Result<U256> {
    let swap_data = match pool_variant {
        PoolVariant::UniswapV2 => BrainDanceCalls::CalculateSwapV2(CalculateSwapV2Call {
            amount_in: amount_in.into(),
            target_pair: target_pool.0.into(),
            input_token: token_in.0.into(),
            output_token: token_out.0.into(),
        }),
        PoolVariant::UniswapV3 => BrainDanceCalls::CalculateSwapV3(CalculateSwapV3Call {
            amount_in: I256::from_raw(amount_in),
            target_pool_address: target_pool.0.into(),
            input_token: token_in.0.into(),
            output_token: token_out.0.into(),
        }),
    };

    evm.env.tx.caller = CONTROLLER_ADDR.0.into();
    evm.env.tx.transact_to = TransactTo::Call(BRAINDANCE_ADDR.0.into());
    evm.env.tx.data = swap_data.encode().into();
    evm.env.tx.gas_limit = 700000;
    evm.env.tx.gas_price = u256_to_ru256(base_fee);
    evm.env.tx.value = rU256::ZERO;

    let res = match evm.transact_commit() {
        Ok(res) => res,
        Err(e) => return err!("failed to commit swap: {:?}", e),
    };
    let output = match res {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(o) => o,
            Output::Create(o, _) => o,
        },
        ExecutionResult::Revert { output, gas_used } => {
            return err!("swap reverted: {:?} (gas used: {:?})", output, gas_used)
        }
        ExecutionResult::Halt { reason, .. } => return err!("swap halted: {:?}", reason),
    };
    let (_amount_out, balance) = match pool_variant {
        PoolVariant::UniswapV2 => match CalculateSwapV2Return::decode(&output) {
            Ok(output) => (output.amount_out, output.real_after_balance),
            Err(e) => return err!("failed to decode swap result: {:?}", e),
        },
        PoolVariant::UniswapV3 => match CalculateSwapV3Return::decode(&output) {
            Ok(output) => (output.amount_out, output.real_after_balance),
            Err(e) => return err!("failed to decode swap result: {:?}", e),
        },
    };
    Ok(balance)
}

/// returns price of token1/token0 in forked EVM.
pub async fn sim_price_v3(
    target_pool: Address,
    input_token: Address,
    output_token: Address,
    evm: &mut EVM<ForkDB>,
) -> Result<U256> {
    abigen!(
        IUniswapV3Pool,
        r#"[
            function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked)
        ]"#
    );

    let output = call_function(evm, "0x3850c7bd", target_pool)?; // slot0()
    let slot0_tokens = abi::decode(
        &vec![
            ParamType::Uint(160), // sqrtPriceX96
            ParamType::Int(24),   // tick
            ParamType::Uint(16),  // observationIndex
            ParamType::Uint(16),  // observationCardinality
            ParamType::Uint(16),  // observationCardinalityNext
            ParamType::Uint(8),   // feeProtocol
            ParamType::Bool,      // unlocked
        ],
        &output,
    )?;
    let sqrt_price = slot0_tokens[0].clone().into_uint().expect("sqrt_price");

    let output = call_function(evm, "0x1a686502", target_pool)?; // liquidity()
    let liquidity_tokens = abi::decode(&[ParamType::Uint(128)], &output)?;
    let liquidity = liquidity_tokens[0].clone().into_uint().expect("liquidity");

    let token0 = match input_token < output_token {
        true => input_token,
        false => output_token,
    };
    let output = call_function(evm, "0x313ce567", token0)?; // decimals()
    let token0_decimals_tokens = abi::decode(&[ParamType::Uint(8)], &output)?;
    let token0_decimals = token0_decimals_tokens[0]
        .clone()
        .into_uint()
        .expect("token0_decimals");

    get_price_v3(liquidity, sqrt_price, token0_decimals)
}

/// returns price of token1/token0 in forked EVM.
pub async fn sim_price_v2(
    target_pool: Address,
    input_token: Address,
    output_token: Address,
    evm: &mut EVM<ForkDB>,
) -> Result<U256> {
    // getReserves
    evm.env.tx.transact_to = TransactTo::Call(target_pool.0.into());
    evm.env.tx.caller = ETH_DEV_ACCOUNT.address.0.into();
    evm.env.tx.value = rU256::ZERO;
    evm.env.tx.data = Bytes::from_str("0x0902f1ac")?; // getReserves()
    evm.env.tx.gas_price = rU256::from(100_000_000_000_i64);
    evm.env.tx.gas_limit = 900_000_u64;
    evm.env.tx.gas_priority_fee = Some(rU256::from(13_000_000_000_u64));
    let result = match evm.transact_ref() {
        Ok(result) => result.result,
        Err(e) => return err!("SimulationEvmError: {}", e), // TODO: add new error type
    };
    let output: Bytes = match result {
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

    let tokens = abi::decode(
        &[
            ParamType::Uint(128),
            ParamType::Uint(128),
            ParamType::Uint(32),
        ],
        &output,
    )?;

    let reserves_0 = tokens[0].clone().into_uint().ok_or::<Error>(
        HindsightError::MathError(format!(
            "reserves_0 failed to cast token to uint (token={})",
            tokens[0]
        ))
        .into(),
    )?;
    let reserves_1 = tokens[1].clone().into_uint().ok_or::<Error>(
        HindsightError::MathError(format!(
            "reserves_1 failed to cast token to uint (token={})",
            tokens[1]
        ))
        .into(),
    )?;

    let token0 = match input_token < output_token {
        true => input_token,
        false => output_token,
    };
    let output = call_function(evm, "0x313ce567", token0)?; // decimals()
    let token0_decimals_tokens = abi::decode(&[ParamType::Uint(8)], &output)?;
    let token0_decimals = token0_decimals_tokens[0]
        .clone()
        .into_uint()
        .ok_or::<Error>(HindsightError::CallError("token decimals not found".to_owned()).into())?;

    reserves_1
        .mul(U256::from(10).pow(token0_decimals))
        .checked_div(reserves_0)
        .ok_or::<Error>(
            HindsightError::MathError(format!(
                "failed to divide reserves (reserves_0, reserves_1)=({},{})",
                reserves_0, reserves_1
            ))
            .into(),
        )
}

pub fn call_function(evm: &mut EVM<ForkDB>, method: &str, contract: Address) -> Result<Bytes> {
    debug!("calling method {:?}", method);
    let tx: TransactionRequest = TransactionRequest {
        from: Some(ETH_DEV_ACCOUNT.address),
        to: Some(contract.into()),
        gas: Some(U256::from(900_000_u64)),
        gas_price: Some(U256::from(1_000_000_000_000_u64)),
        value: None,
        data: Some(BytesEthers::from_str(method)?),
        nonce: None,
        chain_id: Some(U64::from(1)),
    };
    sim_tx_request(evm, tx)
}

pub fn sim_tx_request(evm: &mut EVM<ForkDB>, tx: TransactionRequest) -> Result<Bytes> {
    evm.env.tx.caller = tx.from.unwrap_or(ETH_DEV_ACCOUNT.address).0.into();
    evm.env.tx.transact_to = TransactTo::Call(rAddress::from(
        tx.to
            .to_owned()
            .ok_or::<Error>(
                HindsightError::EvmParseError(format!("tx.to invalid ({:?})", tx.to)).into(),
            )?
            .as_address()
            .ok_or::<Error>(
                // TODO: find cleaner way to do this
                HindsightError::EvmParseError(format!(
                    "tx.to could not parse address ({:?})",
                    tx.to
                ))
                .into(),
            )?
            .0,
    ));
    evm.env.tx.data = tx
        .data
        .to_owned()
        .ok_or::<Error>(
            HindsightError::EvmParseError(format!("tx.data invalid ({:?})", tx.data)).into(),
        )?
        .0
        .into();

    // parse Ethers U256s `tx.value` and `tx.gas_price` to slices for rU256 encoding
    let mut value: [u8; 32] = [0; 32];
    let mut gas_price: [u8; 32] = [0; 32];
    tx.value.unwrap_or_default().to_big_endian(&mut value);
    tx.gas_price
        .unwrap_or_default()
        .to_big_endian(&mut gas_price);

    evm.env.tx.value = rU256::from_be_slice(&value);
    evm.env.tx.gas_price = rU256::from_be_slice(&gas_price);
    evm.env.tx.gas_limit = tx.gas.unwrap_or_default().as_u64();
    let res = match evm.transact_ref() {
        Ok(res) => res.result,
        Err(err) => {
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

/// is there not a better way?
fn u256_to_ru256(num: U256) -> rU256 {
    let mut bytes: [u8; 32] = [0; 32];
    num.to_big_endian(&mut bytes);
    rU256::from_be_slice(&bytes)
}

fn inject_tx(evm: &mut EVM<ForkDB>, tx: &Transaction) -> Result<()> {
    evm.env.tx.caller = tx.from.0.into();
    evm.env.tx.transact_to = TransactTo::Call(rAddress::from(tx.to.unwrap_or_default().0));
    evm.env.tx.data = tx.input.to_owned().0.into();
    let mut value: [u8; 32] = [0; 32];
    tx.value.to_big_endian(&mut value);
    evm.env.tx.value = rU256::from_be_slice(&value);
    evm.env.tx.chain_id = tx.chain_id.map(|id| id.as_u64());
    evm.env.tx.gas_limit = tx.gas.as_u64();
    let mut gas_price: [u8; 32] = [0; 32];
    tx.gas_price
        .unwrap_or_default()
        .to_big_endian(&mut gas_price);
    match tx.transaction_type {
        Some(ethers::types::U64([0])) => {
            evm.env.tx.gas_price = rU256::from_be_slice(&gas_price);
        }
        Some(_) => {
            // type-2 tx
            evm.env.tx.gas_priority_fee = tx.max_priority_fee_per_gas.map(u256_to_ru256);
            evm.env.tx.gas_price = u256_to_ru256(tx.max_fee_per_gas.unwrap_or_default());
        }
        None => {
            // legacy tx
            evm.env.tx.gas_price = u256_to_ru256(tx.gas_price.unwrap_or_default());
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::util::get_block_info;
    use ethers::{
        providers::Middleware,
        types::{Address, U256},
    };
    use hindsight_core::{eth_client::test::get_test_ws_client, Result};

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_gets_sim_price_v2() -> Result<()> {
        let client = get_test_ws_client().await?;
        let block_info = get_block_info(
            client.get_provider(),
            client.provider.get_block_number().await?.as_u64(),
        )
        .await?;
        let mut evm = client.fork_evm(block_info.number.as_u64()).await?;
        let target_pool = Address::from_str("0x811beEd0119b4AfCE20D2583EB608C6F7AF1954f")?; // UniV2 SHIB/WETH
        let token_in = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?; // WETH
        let token_out = Address::from_str("0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE")?; // SHIB
        let price = super::sim_price_v2(target_pool, token_in, token_out, &mut evm).await?;
        println!("price: {}", price);
        assert_ne!(price, U256::from(0));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_gets_sim_price_v3() -> Result<()> {
        let client = get_test_ws_client().await?;
        let block_info = get_block_info(
            client.get_provider(),
            client.provider.get_block_number().await?.as_u64(),
        )
        .await?;
        let mut evm = client.fork_evm(block_info.number.as_u64()).await?;
        let target_pool = Address::from_str("0x2F62f2B4c5fcd7570a709DeC05D68EA19c82A9ec")?; // UniV3 SHIB/WETH (fee=3000)
        let token_in = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?; // WETH
        let token_out = Address::from_str("0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE")?; // SHIB
        let price = super::sim_price_v3(target_pool, token_in, token_out, &mut evm).await?;
        println!("price: {}", price);
        assert_ne!(price, U256::from(0));
        Ok(())
    }
}
