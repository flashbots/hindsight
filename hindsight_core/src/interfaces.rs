use ethers::types::{Address, Block, Transaction, I256, U256, U64};
use mev_share_sse::EventHistory;
use serde::{self, Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimArbResult {
    pub user_trade: UserTradeParams,
    pub backrun_trade: BackrunResult,
}

#[derive(Clone, Debug)]
pub struct SimArb {
    pub pair_tokens: TokenPair,
    pub amount_in: U256,
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

#[derive(Default, Clone, Copy)]
pub struct BlockInfo {
    pub number: U64,
    pub base_fee_per_gas: U256,
    pub timestamp: U256,
    // These are optional because we don't know these values for `next_block`
    pub gas_used: Option<U256>,
    pub gas_limit: Option<U256>,
}

impl From<Block<Transaction>> for BlockInfo {
    fn from(block: Block<Transaction>) -> Self {
        Self {
            number: block.number.unwrap_or_default(),
            base_fee_per_gas: block.base_fee_per_gas.unwrap_or_default(),
            timestamp: block.timestamp,
            gas_used: Some(block.gas_used),
            gas_limit: Some(block.gas_limit),
        }
    }
}

// #[cfg(test)]
pub mod test {
    use super::*;
    use ethers::{types::H256, utils::parse_ether};
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
                results: vec![
                    SimArbResult::test_example(0f64),
                    SimArbResult::test_example(0.1f64),
                    SimArbResult::test_example(1f64),
                ],
                max_profit: 0x1337.into(),
            }
        }
    }

    impl SimArbResult {
        pub fn test_example(profit_d: f64) -> Self {
            Self {
                user_trade: UserTradeParams {
                    pool_variant: PoolVariant::UniswapV2,
                    token_in: Address::zero(),
                    token_out: Address::zero(),
                    amount0_sent: 0.into(),
                    amount1_sent: 0.into(),
                    token0_is_weth: false,
                    pool: Address::zero(),
                    price: 0.into(),
                    tokens: TokenPair {
                        weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
                            .parse::<Address>()
                            .unwrap(),
                        token: "0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE"
                            // SHIB
                            .parse::<Address>()
                            .unwrap(),
                    },
                    arb_pools: vec![
                        PairPool {
                            variant: PoolVariant::UniswapV2,
                            address: "0x811beEd0119b4AfCE20D2583EB608C6F7AF1954f"
                                // uniV2 SHIB/WETH
                                .parse::<Address>()
                                .unwrap(),
                        },
                        PairPool {
                            variant: PoolVariant::UniswapV2,
                            address: "0x24D3dD4a62e29770cf98810b09F89D3A90279E7a"
                                // sushi SHIB/WETH
                                .parse::<Address>()
                                .unwrap(),
                        },
                    ],
                },
                backrun_trade: BackrunResult {
                    amount_in: parse_ether(1.1).unwrap(),
                    balance_end: parse_ether(1.1).unwrap(),
                    profit: parse_ether(0.1 + profit_d).unwrap(),
                    start_pool: "0x811beEd0119b4AfCE20D2583EB608C6F7AF1954f"
                        // uniV2 SHIB/WETH
                        .parse::<Address>()
                        .unwrap(),
                    end_pool: "0x24D3dD4a62e29770cf98810b09F89D3A90279E7a"
                        // sushi SHIB/WETH
                        .parse::<Address>()
                        .unwrap(),
                    start_variant: PoolVariant::UniswapV2,
                    end_variant: PoolVariant::UniswapV2,
                },
            }
        }
    }
}
