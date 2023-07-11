use std::str::FromStr;

use crate::{util::get_price_v3, Result};
use ethers::{
    abi::{self, ParamType},
    prelude::abigen,
    types::{Address, Bytes, TransactionRequest, U256, U64},
};
use revm::{
    primitives::{ExecutionResult, Output, TransactTo, B160, U256 as rU256},
    EVM,
};
use rusty_sando::{
    prelude::fork_db::ForkDB, types::SimulationError, utils::constants::get_eth_dev,
};

/// returns (amount_out, real_after_balance)
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
    let liquidity_tokens = abi::decode(&vec![ParamType::Uint(128)], &output)?;
    let liquidity = liquidity_tokens[0].clone().into_uint().expect("liquidity");

    let token0 = match input_token < output_token {
        true => input_token,
        false => output_token,
    };
    let output = call_function(evm, "0x313ce567", token0)?; // decimals()
    let token0_decimals_tokens = abi::decode(&vec![ParamType::Uint(8)], &output)?;
    let token0_decimals = token0_decimals_tokens[0]
        .clone()
        .into_uint()
        .expect("token0_decimals");

    get_price_v3(liquidity, sqrt_price, token0_decimals)
}

pub async fn sim_price_v2(target_pool: Address, evm: &mut EVM<ForkDB>) -> Result<U256> {
    // get reserves
    evm.env.tx.transact_to = TransactTo::Call(target_pool.0.into());
    evm.env.tx.caller = get_eth_dev().0.into();
    evm.env.tx.value = rU256::ZERO;
    evm.env.tx.data = Bytes::from_str("0x0902f1ac").unwrap().0; // getReserves()
    let result = match evm.transact_ref() {
        Ok(result) => result.result,
        Err(e) => return Err(anyhow::format_err!(SimulationError::EvmError(e))),
    };
    let output: Bytes = match result {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(o) => o.into(),
            Output::Create(o, _) => o.into(),
        },
        ExecutionResult::Revert { output, .. } => {
            return Err(anyhow::format_err!(SimulationError::EvmReverted(output)))
        }
        ExecutionResult::Halt { reason, .. } => {
            return Err(anyhow::format_err!(SimulationError::EvmHalted(reason)))
        }
    };

    let tokens = abi::decode(
        &vec![
            ParamType::Uint(128),
            ParamType::Uint(128),
            ParamType::Uint(32),
        ],
        &output,
    )
    .unwrap();

    let reserves_0 = tokens[0].clone().into_uint().unwrap();
    let reserves_1 = tokens[1].clone().into_uint().unwrap();

    Ok(reserves_1
        .checked_div(reserves_0)
        .ok_or_else(|| anyhow::format_err!("failed to divide reserves"))?)
}

pub fn call_function(evm: &mut EVM<ForkDB>, method: &str, contract: Address) -> Result<Bytes> {
    let tx: TransactionRequest = TransactionRequest {
        from: Some(get_eth_dev()),
        to: Some(contract.into()),
        gas: None,
        gas_price: None,
        value: None,
        data: Some(Bytes::from_str(method)?),
        nonce: None,
        chain_id: Some(U64::from(1)),
    };
    sim_tx_request(evm, tx)
}

pub fn sim_tx_request(evm: &mut EVM<ForkDB>, tx: TransactionRequest) -> Result<Bytes> {
    evm.env.tx.caller = B160::from(tx.from.unwrap_or(get_eth_dev()));
    evm.env.tx.transact_to = TransactTo::Call(B160::from(tx.to.unwrap().as_address().unwrap().0));
    evm.env.tx.data = tx.data.to_owned().unwrap().0;
    evm.env.tx.value = tx.value.unwrap_or_default().into();
    let res = match evm.transact_ref() {
        Ok(res) => res.result,
        Err(err) => {
            return Err(anyhow::anyhow!("failed to simulate tx request: {:?}", err));
        }
    };
    let output: Bytes = match res {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(o) => o.into(),
            Output::Create(o, _) => o.into(),
        },
        ExecutionResult::Revert { output, .. } => {
            return Err(anyhow::format_err!(SimulationError::EvmReverted(output)))
        }
        ExecutionResult::Halt { reason, .. } => {
            return Err(anyhow::format_err!(SimulationError::EvmHalted(reason)))
        }
    };
    Ok(output)
}
