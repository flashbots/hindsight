use crate::{
    evm::call_function,
    util::{get_price_v3, BRAINDANCE_ADDR, CONTROLLER_ADDR},
};
use ethers::{
    abi::{self, AbiDecode, AbiEncode, ParamType},
    prelude::abigen,
    types::{Address, I256, U256},
};
use foundry_contracts::brain_dance::{
    BrainDanceCalls, CalculateSwapV2Call, CalculateSwapV2Return, CalculateSwapV3Call,
    CalculateSwapV3Return,
};
use hindsight_core::{
    err, error::HindsightError, evm::fork_db::ForkDB, interfaces::PoolVariant, util::u256_to_ru256,
    Error, Result,
};
use revm::{
    primitives::{ExecutionResult, Output, TransactTo, U256 as rU256},
    EVM,
};
use std::ops::Mul;

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
    let output = call_function(evm, "0x0902f1ac", target_pool)?; // getReserves()
    let tokens = abi::decode(
        &[
            ParamType::Uint(112),
            ParamType::Uint(112),
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
    let token0_decimals =
        token0_decimals_tokens[0]
            .clone()
            .into_uint()
            .ok_or(HindsightError::CallError(
                "token decimals not found".to_owned(),
            ))?;

    reserves_1
        .mul(U256::from(10).pow(token0_decimals))
        .checked_div(reserves_0)
        .ok_or(
            HindsightError::MathError(format!(
                "failed to divide reserves (reserves_0, reserves_1)=({},{})",
                reserves_0, reserves_1
            ))
            .into(),
        )
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
