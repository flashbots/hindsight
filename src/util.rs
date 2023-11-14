use crate::{
    config::Config,
    info,
    interfaces::{PairPool, PoolVariant},
    Result,
};
use ethers::{
    prelude::{abigen, H160},
    providers::{Middleware, Provider, Ws},
    types::{transaction::eip2718::TypedTransaction, Address, Transaction, H256, U256},
};
use futures::future;
use mev_share_sse::EventHistory;
use rusty_sando::types::BlockInfo;
use std::sync::Arc;
use uniswap_v3_math::{full_math::mul_div, sqrt_price_math::Q96};

pub use ethers::utils::WEI_IN_ETHER as ETH;
pub type WsClient = Arc<Provider<Ws>>;

pub async fn get_ws_client(rpc_url: Option<String>, max_reconnects: usize) -> Result<WsClient> {
    let rpc_url = if let Some(rpc_url) = rpc_url {
        rpc_url
    } else {
        Config::default().rpc_url_ws
    };
    let provider = Provider::<Ws>::connect_with_reconnects(rpc_url, max_reconnects).await?;
    Ok(Arc::new(provider))
}

pub async fn fetch_txs(client: &WsClient, events: &[EventHistory]) -> Result<Vec<Transaction>> {
    let tx_hashes: Vec<H256> = events.iter().map(|e: &EventHistory| e.hint.hash).collect();
    let mut handles = vec![];

    for tx_hash in tx_hashes.into_iter() {
        let client = client.clone();
        handles.push(tokio::task::spawn(async move {
            let tx = &client.get_transaction(tx_hash.to_owned()).await;
            if let Ok(tx) = tx {
                if let Some(tx) = tx {
                    info!("tx found onchain\t{:?}", tx_hash.to_owned());
                    Some(tx.clone())
                } else {
                    info!("tx not found onchain\t{:?}", tx_hash.to_owned());
                    None
                }
            } else {
                info!("error fetching tx: {:?}", tx);
                None
            }
        }));
    }
    let results = future::join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect::<Vec<_>>();
    Ok(results)
}

pub async fn get_pair_tokens(client: &WsClient, pair: Address) -> Result<(Address, Address)> {
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

pub async fn get_block_info(client: &WsClient, block_num: u64) -> Result<BlockInfo> {
    let block = client
        .get_block(block_num)
        .await?
        .ok_or(anyhow::format_err!("failed to get block {:?}", block_num))?;
    Ok(BlockInfo {
        number: block_num.into(),
        timestamp: block.timestamp,
        base_fee: block.base_fee_per_gas.unwrap_or(1_000_000_000.into()),
    })
}

async fn get_v2_pairs(client: &WsClient, pair_tokens: (Address, Address)) -> Result<Vec<Address>> {
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

async fn get_v3_pair(client: &WsClient, pair_tokens: (Address, Address)) -> Result<Address> {
    abigen!(
        IUniswapV3Factory,
        r#"[
            function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
        ]"#
    );
    let contract = IUniswapV3Factory::new(
        "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse::<H160>()?,
        client.clone(),
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
        address: get_v3_pair(client, pair_tokens).await?,
        variant: PoolVariant::UniswapV3,
    });
    // v2 pairs pull from multiple v2 clones
    let v2_pairs = get_v2_pairs(client, pair_tokens).await?;
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

pub async fn get_decimals(client: &WsClient, token: Address) -> Result<U256> {
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

pub async fn get_balance_call(
    client: &WsClient,
    token: Address,
    account: Address,
) -> Result<TypedTransaction> {
    abigen!(
        IERC20,
        r#"[
            function balanceOf(address account) external view returns (uint256)
        ]"#
    );
    let contract = IERC20::new(token, client.clone());
    Ok(contract.balance_of(account).tx)
}

pub fn filter_events_by_topic(
    events: &[EventHistory],
    filter_topics: &[H256],
) -> Vec<EventHistory> {
    events
        .iter()
        .filter(|event| {
            event
                .hint
                .logs
                .iter()
                .map(|log| log.topics.to_owned())
                .any(|topics| {
                    topics
                        .iter()
                        .map(|topic| filter_topics.contains(topic))
                        .reduce(|a, b| a || b)
                        .unwrap_or(false)
                })
        })
        .map(|e| e.to_owned())
        .collect::<Vec<_>>()
}

#[cfg(test)]
pub mod test {
    use crate::util::{get_ws_client, WsClient};
    use crate::Result;

    pub async fn get_test_ws_client() -> Result<WsClient> {
        let ws_client = get_ws_client(None, 1).await?;
        Ok(ws_client)
    }
}
