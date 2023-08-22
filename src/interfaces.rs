use ethers::types::{Address, I256, U256};
use mev_share_sse::EventHistory;
use serde::{self, Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimArbResult {
    pub user_trade: UserTradeParams,
    pub backrun_trade: BackrunResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackrunResult {
    pub amount_in: U256,
    pub balance_end: U256,
    pub profit: U256,
    pub start_pool: Address,
    pub end_pool: Address,
    pub start_variant: PoolVariant,
    pub end_variant: PoolVariant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimArbResultBatch {
    pub event: EventHistory,
    pub results: Vec<SimArbResult>,
    pub max_profit: U256,
}

/// Information derived from user's trade tx.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
    pub arb_pools: Vec<PairPool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPair {
    pub weth: Address,
    pub token: Address,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct PairPool {
    pub variant: PoolVariant,
    pub address: Address,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredArbsRanges {
    pub earliest_timestamp: u64,
    pub latest_timestamp: u64,
    pub earliest_block: u64,
    pub latest_block: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq)]
pub enum PoolVariant {
    UniswapV2,
    UniswapV3,
}

#[cfg(test)]
mod test {
    use super::*;
    use ethers::types::H256;
    use mev_share_sse::Hint;
    use rand::Rng;
    impl SimArbResultBatch {
        pub fn test_example() -> Self {
            // get random u64
            let mut rng = rand::thread_rng();
            let rnum = rng.gen_range(0..100000);
            Self {
                event: EventHistory {
                    block: 9001,
                    timestamp: 9001,
                    hint: Hint {
                        txs: vec![],
                        hash: H256::from_low_u64_be(rnum),
                        logs: vec![],
                        gas_used: None,
                        mev_gas_price: None,
                    },
                },
                results: vec![],
                max_profit: 0x1337.into(),
            }
        }
    }
}
