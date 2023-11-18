use std::sync::Arc;

use data::interfaces::{PairPool, PoolVariant};
use ethers::{
    middleware::Middleware,
    prelude::{abigen, H160},
    types::{Address, H256, U256},
};
use hindsight_core::{
    eth_client::{WsClient, WsProvider},
    interfaces::BlockInfo,
    Result,
};
use lazy_static::lazy_static;
use uniswap_v3_math::{full_math::mul_div, sqrt_price_math::Q96};

pub use ethers::utils::WEI_IN_ETHER as ETH;

pub struct DevAccount {
    pub address: Address,
    pub private_key: H256,
}

pub fn eth_dev_account() -> DevAccount {
    DevAccount {
        address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse::<Address>()
            .expect("invalid address"),
        private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse::<H256>()
            .expect("invalid private key"),
    }
}

lazy_static! {
    pub static ref ETH_DEV_ACCOUNT: DevAccount = eth_dev_account();
    pub static ref BRAINDANCE_ADDR: Address = "0xc433333333333333333333333333333333333353"
        .parse::<Address>()
        .expect("invalid address");
    pub static ref CONTROLLER_ADDR: Address = "0xf00000000000000000000000000000000000000d"
        .parse::<Address>()
        .expect("invalid address");
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

pub async fn get_pair_tokens(client: &WsClient, pair: Address) -> Result<(Address, Address)> {
    abigen!(
        IPairTokens,
        r#"[
            function token0() external view returns (address)
            function token1() external view returns (address)
        ]"#
    );
    let contract = IPairTokens::new(pair, client.get_provider());
    let token0 = contract.token_0().call().await?;
    let token1 = contract.token_1().call().await?;
    Ok((token0, token1))
}

pub async fn get_decimals(client: &WsClient, token: Address) -> Result<U256> {
    abigen!(
        IERC20,
        r#"[
            function decimals() external view returns (uint256)
        ]"#
    );
    let contract = IERC20::new(token, client.get_provider());
    let decimals = contract.decimals().call().await?;
    Ok(decimals)
}

/// Get all v2 pair addresses by calling `getPair` on each supported
/// factory contract via `eth_call`.
async fn get_v2_pairs(client: &WsClient, pair_tokens: (Address, Address)) -> Result<Vec<Address>> {
    abigen!(
        IUniswapV2Factory,
        r#"[
            function getPair(address tokenA, address tokenB) external view returns (address pair)
        ]"#
    );
    let uni_factory = IUniswapV2Factory::new(
        "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse::<H160>()?,
        client.get_provider(),
    );
    let sushi_factory = IUniswapV2Factory::new(
        "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse::<H160>()?,
        client.get_provider(),
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
    client: &WsClient,
    pair_tokens: (Address, Address),
) -> Result<Vec<PairPool>> {
    let mut all_pairs = vec![];
    // push v3 pair (there should only be one for a given fee, which we hard-code to 3000 in get_v3_pair)
    all_pairs.push(PairPool {
        address: get_v3_pair(client.get_provider(), pair_tokens).await?,
        variant: PoolVariant::UniswapV3,
    });
    // v2 pairs pull from multiple v2 clones
    let v2_pairs = get_v2_pairs(&client, pair_tokens).await?;
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
