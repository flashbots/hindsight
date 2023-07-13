use ethers::types::{Address, I256, U256};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SimArbResult {
    #[serde(rename = "userTrade")]
    pub user_trade: UserTradeParams,
    #[serde(rename = "backrunTrade")]
    pub backrun_trade: BackrunResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BackrunResult {
    #[serde(rename = "amountIn")]
    pub amount_in: U256,
    #[serde(rename = "balanceEnd")]
    pub balance_end: U256,
    pub profit: U256,
    #[serde(rename = "startPool")]
    pub start_pool: Address,
    #[serde(rename = "endPool")]
    pub end_pool: Address,
    #[serde(rename = "startVariant")]
    pub start_variant: PoolVariant,
    #[serde(rename = "endVariant")]
    pub end_variant: PoolVariant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SimArbResultBatch {
    pub results: Vec<SimArbResult>,
    #[serde(rename = "maxProfit")]
    pub max_profit: U256,
}

impl SimArbResultBatch {
    pub fn test_example() -> Self {
        Self {
            results: vec![],
            max_profit: 0x1337.into(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub enum PoolVariant {
    UniswapV2,
    UniswapV3,
}

impl PoolVariant {
    pub fn other(&self) -> Self {
        match self {
            PoolVariant::UniswapV2 => PoolVariant::UniswapV3,
            PoolVariant::UniswapV3 => PoolVariant::UniswapV2,
        }
    }
}

/// Information derived from user's trade tx.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserTradeParams {
    #[serde(rename = "poolVariant")]
    pub pool_variant: PoolVariant,
    #[serde(rename = "tokenIn")]
    pub token_in: Address,
    #[serde(rename = "tokenOut")]
    pub token_out: Address,
    #[serde(rename = "amount0Sent")]
    pub amount0_sent: I256,
    #[serde(rename = "amount1Sent")]
    pub amount1_sent: I256,
    #[serde(rename = "token0IsWeth")]
    pub token0_is_weth: bool,
    pub pool: Address,
    pub price: U256,
    pub tokens: TokenPair,
    #[serde(rename = "arbPools")]
    pub arb_pools: Vec<Address>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenPair {
    pub weth: Address,
    pub token: Address,
}
