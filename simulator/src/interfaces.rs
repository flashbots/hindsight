use ethers::types::{Address, I256, U256};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SimArbResult {
    pub amount_in: U256,
    pub balance_end: U256,
    pub trade_params: UserTradeParams,
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
    pub pool_variant: PoolVariant,
    pub token_in: Address,
    pub token_out: Address,
    pub amount0_sent: I256,
    pub amount1_sent: I256,
    pub token0_is_weth: bool,
    pub pool: Address,
    pub price: U256,
    pub tokens: TokenPair,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenPair {
    pub weth: Address,
    pub token: Address,
}
