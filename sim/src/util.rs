use std::{str::FromStr, sync::Arc};

use data::interfaces::{PairPool, PoolVariant};
use ethers::{
    abi,
    middleware::Middleware,
    prelude::{abigen, H160},
    types::{Address, U256},
    utils::parse_ether,
};
use foundry_contracts::brain_dance::{BRAINDANCE_BYTECODE, BRAINDANCE_DEPLOYED_BYTECODE};
use hindsight_core::{
    eth_client::WsProvider, evm::fork_factory::ForkFactory, interfaces::BlockInfo, util::WETH,
    Result,
};
use lazy_static::lazy_static;
use revm::primitives::{keccak256 as rkeccak256, AccountInfo, Address as rAddress, U256 as rU256};
use uniswap_v3_math::{full_math::mul_div, sqrt_price_math::Q96};

pub use ethers::utils::WEI_IN_ETHER as ETH;

use crate::evm::inject_contract;

lazy_static! {
    pub static ref ETH_DEV_ADDRESS: Address = "0x9999999999999999999999999999999999999991"
        .parse::<Address>()
        .expect("invalid address");
    pub static ref BRAINDANCE_ADDR: Address = "0xc4b1333333333333333333333333333333333353"
        .parse::<Address>()
        .expect("invalid address");
    pub static ref CONTROLLER_ADDR: Address = "0xf00d00000000000000000000000000000000f00d"
        .parse::<Address>()
        .expect("invalid address");
    pub static ref BRAINDANCE_START_BALANCE: rU256 =
        rU256::from_str("0x16C4ABBEBEA0100000").expect("invalid start balance");
}

/// Returns the price (token1 per token0).
pub fn get_price_v2(reserves0: U256, reserves1: U256, token0_decimals: U256) -> Result<U256> {
    Ok((reserves1 * U256::from(10).pow(token0_decimals)) / reserves0)
}

/// Returns the price (token1 per token0).
pub fn get_price_v3(liquidity: U256, sqrt_price_x96: U256, token0_decimals: U256) -> Result<U256> {
    let reserves0 = mul_div(liquidity, Q96, sqrt_price_x96)?;
    let reserves1 = mul_div(liquidity, sqrt_price_x96, Q96)?;

    Ok((reserves1 * U256::from(10).pow(token0_decimals)) / reserves0)
}

pub async fn get_pair_tokens(client: Arc<WsProvider>, pair: Address) -> Result<(Address, Address)> {
    abigen!(
        IPairTokens,
        r#"[
            function token0() external view returns (address)
            function token1() external view returns (address)
        ]"#
    );
    let contract = IPairTokens::new(pair, client.clone());
    let token0 = contract.token_0().call().await?;
    let token1 = contract.token_1().call().await?;
    Ok((token0, token1))
}

pub async fn get_decimals(client: Arc<WsProvider>, token: Address) -> Result<U256> {
    abigen!(
        IERC20,
        r#"[
            function decimals() external view returns (uint256)
        ]"#
    );
    let contract = IERC20::new(token, client.clone());
    let decimals = contract.decimals().call().await?;
    Ok(decimals)
}

/// Get all v2 pair addresses by calling `getPair` on each supported
/// factory contract via `eth_call`.
async fn get_v2_pairs(
    client: Arc<WsProvider>,
    pair_tokens: (Address, Address),
) -> Result<Vec<Address>> {
    abigen!(
        IUniswapV2Factory,
        r#"[
            function getPair(address tokenA, address tokenB) external view returns (address pair)
        ]"#
    );
    let uni_factory = IUniswapV2Factory::new(
        "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse::<H160>()?,
        client.clone(),
    );
    let sushi_factory = IUniswapV2Factory::new(
        "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse::<H160>()?,
        client.clone(),
    );

    let uni_pair: Result<Address, _> = uni_factory
        .get_pair(pair_tokens.0, pair_tokens.1)
        .call()
        .await;
    let sushi_pair: Result<Address, _> = sushi_factory
        .get_pair(pair_tokens.0, pair_tokens.1)
        .call()
        .await;
    let mut pairs = vec![];
    if let Ok(uni_pair) = uni_pair {
        pairs.push(uni_pair);
    }
    if let Ok(sushi_pair) = sushi_pair {
        pairs.push(sushi_pair);
    }

    Ok(pairs)
}

/// Get the v3 pair address by calling `get_pool` on the contract via `eth_call`.
async fn get_v3_pair(
    provider: Arc<WsProvider>,
    pair_tokens: (Address, Address),
) -> Result<Address> {
    abigen!(
        IUniswapV3Factory,
        r#"[
            function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
        ]"#
    );
    let contract = IUniswapV3Factory::new(
        "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse::<H160>()?,
        provider,
    );
    Ok(contract
        .get_pool(pair_tokens.0, pair_tokens.1, 3000)
        .call()
        .await?)
}

/// Get pair address from all supported factories, including the given pair.
/// Filter what I return if you need to.
pub async fn get_all_trading_pools(
    client: Arc<WsProvider>,
    pair_tokens: (Address, Address),
) -> Result<Vec<PairPool>> {
    let mut all_pairs = vec![];
    // push v3 pair (there should only be one for a given fee, which we hard-code to 3000 in get_v3_pair)
    all_pairs.push(PairPool {
        address: get_v3_pair(client.clone(), pair_tokens).await?,
        variant: PoolVariant::UniswapV3,
    });
    // v2 pairs pull from multiple v2 clones
    let v2_pairs = get_v2_pairs(client.clone(), pair_tokens).await?;
    all_pairs.append(
        &mut v2_pairs
            .into_iter()
            .map(|pair| PairPool {
                address: pair,
                variant: PoolVariant::UniswapV2,
            })
            .collect::<Vec<_>>(),
    );
    Ok(all_pairs)
}

pub async fn get_block_info(provider: Arc<WsProvider>, block_num: u64) -> Result<BlockInfo> {
    let block = provider
        .get_block(block_num)
        .await?
        .ok_or(hindsight_core::anyhow::format_err!(
            "failed to get block {:?}",
            block_num
        ))?;
    Ok(BlockInfo {
        number: block_num.into(),
        timestamp: block.timestamp,
        base_fee_per_gas: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
        gas_limit: Some(block.gas_limit),
        gas_used: Some(block.gas_used),
    })
}

fn inject_braindance_code(fork_factory: &mut ForkFactory) {
    // put contract onchain
    inject_contract(
        fork_factory,
        BRAINDANCE_ADDR.0.into(),
        BRAINDANCE_BYTECODE.0.to_owned().into(),
        BRAINDANCE_DEPLOYED_BYTECODE.0.to_owned().into(),
    );
}

pub fn set_weth_balance(fork_factory: &mut ForkFactory, address: rAddress, amount: rU256) {
    // Get balance mapping of address inside of weth contract
    let slot: rU256 = rkeccak256(abi::encode(&[
        abi::Token::Address((address.0).0.into()),
        abi::Token::Uint(U256::from(3)),
    ]))
    .into();

    fork_factory
        .insert_account_storage(WETH.0.into(), slot, amount)
        .expect(&format!("failed to insert account storage. slot={}", slot));
}

pub fn attach_braindance_module(fork_factory: &mut ForkFactory) {
    inject_braindance_code(fork_factory);

    // setup controller account & eth dev account w/ a bunch of eth
    let mut value: [u8; 32] = [0; 32];
    parse_ether(1337).unwrap().to_big_endian(&mut value);
    let account_info = AccountInfo::from_balance(rU256::from_be_bytes(value));
    fork_factory.insert_account_info(CONTROLLER_ADDR.0.into(), account_info.to_owned());
    fork_factory.insert_account_info(ETH_DEV_ADDRESS.0.into(), account_info);

    set_weth_balance(
        fork_factory,
        BRAINDANCE_ADDR.0.into(),
        *BRAINDANCE_START_BALANCE,
    );
    set_weth_balance(
        fork_factory,
        ETH_DEV_ADDRESS.0.into(),
        *BRAINDANCE_START_BALANCE,
    );
    set_weth_balance(
        fork_factory,
        CONTROLLER_ADDR.0.into(),
        *BRAINDANCE_START_BALANCE,
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use hindsight_core::{
        eth_client::test::get_test_ws_client,
        evm::state_diff::{StateDiff, ToCacheDb},
    };
    use revm::db::DatabaseRef;

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_sets_weth_balance() {
        let client = get_test_ws_client().await.unwrap();
        let block_num = client.provider.get_block_number().await.unwrap();

        ////////////////////// this should all be a function ////////////////
        let block_traces = client
            .get_block_traces(
                &client
                    .provider
                    .get_block_with_txs(block_num)
                    .await
                    .unwrap()
                    .unwrap(),
            )
            .await
            .unwrap();
        let state_diff = StateDiff::from(block_traces);
        let initial_db = state_diff
            .to_cache_db(&client, Some(0.into()))
            .await
            .unwrap();
        let mut fork_factory = ForkFactory::new_sandbox_factory(
            client.arc_provider(),
            initial_db,
            Some(block_num.into()),
        );
        /////////////////////////////////////////////////////////////////////

        set_weth_balance(
            &mut fork_factory,
            ETH_DEV_ADDRESS.0.into(),
            *BRAINDANCE_START_BALANCE,
        );
        let db = fork_factory.new_sandbox_fork();
        let eth_dev_balance = db
            .storage(ETH_DEV_ADDRESS.0.into(), rU256::from(3))
            .unwrap();
        assert_eq!(eth_dev_balance, *BRAINDANCE_START_BALANCE);
    }
}
