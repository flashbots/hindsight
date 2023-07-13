use crate::error::HindsightError;
use crate::interfaces::{PoolVariant, SimArbResult, TokenPair, UserTradeParams};
use crate::sim::evm::{sim_price_v2, sim_price_v3};
use crate::Result;
use crate::{debug, info};
use async_recursion::async_recursion;
use ethers::providers::Middleware;
use ethers::types::{AccountDiff, Address, BlockNumber, Transaction, H160, H256, I256, U256};
use futures::future;
use mev_share_sse::{EventHistory, EventTransactionLog};
use revm::primitives::{ExecutionResult, Output, ResultAndState, TransactTo, B160, U256 as rU256};
use revm::EVM;
use rusty_sando::prelude::fork_db::ForkDB;
use rusty_sando::simulate::{
    attach_braindance_module, braindance_address, braindance_controller_address, setup_block_state,
};
use rusty_sando::types::BlockInfo;
use rusty_sando::utils::tx_builder::braindance;
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use uniswap_v3_math::utils::RUINT_MAX_U256;

// use crate::data::HistoricalEvent;
use crate::util::{
    get_other_pair_addresses, get_pair_tokens, get_price_v2, get_price_v3, WsClient,
};
use rusty_sando::{forked_db::fork_factory::ForkFactory, utils::state_diff};

const MAX_DEPTH: usize = 4;
// number
const STEP_INTERVALS: usize = 15;

/// Return an evm instance forked from the provided block info and client state
/// with braindance module initialized.
pub async fn fork_evm(client: &WsClient, block_info: &BlockInfo) -> Result<EVM<ForkDB>> {
    let fork_block_num = BlockNumber::Number(block_info.number);
    let fork_block = Some(ethers::types::BlockId::Number(fork_block_num));

    let state_diffs =
        if let Some(sd) = state_diff::get_from_txs(&client, &vec![], fork_block_num).await {
            sd
        } else {
            BTreeMap::<H160, AccountDiff>::new()
        };
    let initial_db = state_diff::to_cache_db(&state_diffs, fork_block, &client)
        .await
        .unwrap(); // TODO: handle unwrap
    let mut fork_factory = ForkFactory::new_sandbox_factory(client.clone(), initial_db, fork_block);
    attach_braindance_module(&mut fork_factory);

    let mut evm = EVM::new();
    evm.database(fork_factory.new_sandbox_fork());
    setup_block_state(&mut evm, block_info);

    Ok(evm)
}

/// Returns None if trade params can't be derived.
///
/// May derive multiple trades from a single tx.
async fn derive_trade_params(
    client: &WsClient,
    tx: Transaction,
    event: &EventHistory,
) -> Result<Vec<UserTradeParams>> {
    let uniswap_topics = vec![
        // univ3
        // Swap(address,address,int256,int256,uint160,uint128,int24)
        H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67")
            .unwrap(),
        // univ2
        // Swap(address,uint256,uint256,uint256,uint256,address)
        H256::from_str("0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822")
            .unwrap(),
    ];

    // get potential pool addresses from event, relying on mev-share hints
    let swap_logs = event
        .hint
        .logs
        .iter()
        .filter(|log| uniswap_topics.contains(&log.topics[0]))
        .map(|log| log.to_owned())
        .collect::<Vec<EventTransactionLog>>();
    debug!("swap logs {:?}", swap_logs);

    // collect trade params for each pair derived from swap logs
    let mut trade_params = vec![];
    for swap_log in swap_logs {
        let pool_address = swap_log.address;
        let swap_topic = swap_log.topics[0];
        debug!("pool address: {:?}", pool_address);

        // derive pool variant from event log topics
        debug!("swap topic: {:?}", swap_topic);
        let univ3 =
            H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67")
                .unwrap();
        let pool_variant = if swap_topic == univ3 {
            PoolVariant::UniswapV3
        } else {
            PoolVariant::UniswapV2 // assume events are pre-screened, so all non-V3 events are V2
        };
        debug!("pool variant: {:?}", pool_variant);

        // get token addrs from pool address
        let (token0, token1) = get_pair_tokens(client, pool_address).await?;
        debug!("token0\t{:?}\ntoken1\t{:?}", token0, token1);
        let token0_is_weth =
            token0 == "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".parse::<H160>()?;

        // derive trade direction from (full) tx logs
        let tx_receipt = client.get_transaction_receipt(tx.hash).await?.unwrap(); // TODO: handle unwrap
        let swap_log = tx_receipt
            .logs
            .iter()
            .find(|log| log.topics.contains(&swap_topic) && log.address == pool_address);

        let swap_log = swap_log.ok_or(anyhow::format_err!(
            "no swap logs found for tx {:?}",
            tx.hash
        ))?;

        // if a Sync event (UniV2) is detected from the tx logs, it can be used to get the new price
        let sync_log: Option<_> = tx_receipt.logs.iter().find(|log| {
            log.topics[0]
                == H256::from_str(
                    "0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1",
                )
                .unwrap()
                && log.address == pool_address
        });

        let (amount0_sent, amount1_sent, new_price) = match pool_variant {
            PoolVariant::UniswapV3 => {
                let amount0 = I256::from_raw(U256::from_big_endian(&swap_log.data[0..32]));
                let amount1 = I256::from_raw(U256::from_big_endian(&swap_log.data[32..64]));
                let sqrt_price = U256::from_big_endian(&swap_log.data[64..96]); // u160
                let liquidity = U256::from_big_endian(&swap_log.data[96..128]); // u128
                let new_price = get_price_v3(liquidity, sqrt_price, U256::from(18))?;
                (
                    /* amount0_sent */
                    if amount0.le(&0.into()) {
                        0.into()
                    } else {
                        amount0
                    },
                    /* amount1_sent */
                    if amount1.le(&0.into()) {
                        0.into()
                    } else {
                        amount1
                    },
                    /* new_price */
                    new_price,
                )
            }
            PoolVariant::UniswapV2 => {
                let amount0_out = I256::from_raw(U256::from_big_endian(&swap_log.data[64..96]));
                let amount1_out = I256::from_raw(U256::from_big_endian(&swap_log.data[96..128]));
                let mut price = U256::zero();
                if let Some(sync_log) = sync_log {
                    let reserve0 = U256::from_big_endian(&sync_log.data[0..32]);
                    let reserve1 = U256::from_big_endian(&sync_log.data[32..64]);
                    price = get_price_v2(reserve0, reserve1, U256::from(18))?;
                }
                (amount0_out, amount1_out, price)
            }
        };

        let swap_0_for_1 = amount0_sent.gt(&0.into());

        debug!(
            "***\nuser swaps {} for {}\n***",
            if swap_0_for_1 { token0 } else { token1 },
            if swap_0_for_1 { token1 } else { token0 }
        );

        trade_params.push(UserTradeParams {
            pool_variant,
            token_in: if swap_0_for_1 { token0 } else { token1 },
            token_out: if swap_0_for_1 { token1 } else { token0 },
            amount0_sent,
            amount1_sent,
            pool: pool_address,
            price: new_price,
            token0_is_weth,
            tokens: TokenPair {
                weth: if token0_is_weth { token0 } else { token1 },
                token: if token0_is_weth { token1 } else { token0 },
            },
        })
    }
    Ok(trade_params)
}

/// Recursively finds the best possible arbitrage trade for a given set of params.
#[async_recursion]
async fn step_arb(
    client: WsClient,
    user_tx: Transaction,
    block_info: BlockInfo,
    params: UserTradeParams,
    best_amount_in_out: Option<(U256, U256)>,
    range: [U256; 2],
    intervals: usize,
    depth: Option<usize>,
) -> Result<(U256, U256)> {
    info!(
        "step_arb
        best (in, bal)\t{:?}
        depth:\t{:?}
        range:\t{:?}
        user_tx:\t{:?}
    ",
        best_amount_in_out, depth, range, user_tx.hash
    );
    if (range[1] - range[0]) < U256::from(500_000) * 1_000_000_000 {
        debug!("range tight enough, finishing early");
        return best_amount_in_out.ok_or_else(|| {
            anyhow::anyhow!(
                "No arbitrage opportunity found for trade {:?} at depth {:?}",
                params,
                depth
            )
        });
    }
    /*
        (eth_into_arb,
        eth_balance_after_arb)
    */
    let mut best_amount_in_out = best_amount_in_out.unwrap_or((0.into(), 0.into())); // (0, 0) is default assignment on initial call

    if let Some(depth) = depth {
        // stop case: we hit the max depth, or the best amount of WETH in is lower than the gas cost of the backrun tx
        if depth > MAX_DEPTH
            || (best_amount_in_out.0 > U256::from(0)
                && best_amount_in_out.0 < (U256::from(180_000) * block_info.base_fee))
        {
            debug!("depth limit reached or profit too low, finishing early");
            return Ok(best_amount_in_out);
        } else {
            // run sims with current params
            let mut handles = vec![];
            let band_width = (range[1] - range[0]) / U256::from(intervals);
            for i in 0..intervals {
                let amount_in = range[0] + band_width * U256::from(i);
                let client = client.clone();
                let user_tx = user_tx.clone();
                let block_info = block_info.clone();
                let params = params.clone();

                handles.push(tokio::task::spawn(async move {
                    sim_arb(&client.clone(), user_tx, &block_info, &params, amount_in).await
                }));
            }
            let revenues = future::join_all(handles).await;
            let revenue_len = revenues.len();
            let mut num_reverts = 0;

            for result in revenues {
                if let Ok(result) = result {
                    // info!("*** revenue result {:?}", result);
                    if let Ok(result) = result {
                        let (amount_in, balance_out) = result;
                        if balance_out > best_amount_in_out.1 {
                            best_amount_in_out = (amount_in, balance_out);
                            debug!(
                                "new best (amount_in, balance_out): {:?}",
                                best_amount_in_out
                            );
                        }
                    } else {
                        let err = result.as_ref().unwrap_err().to_string();
                        debug!("{}", err);
                        if err.contains("no other pool found") {
                            return result;
                        } else if err.contains("swap reverted") {
                            num_reverts += 1;
                        }
                        // TODO: use real error types, not this garbage
                    }
                } else {
                    return Err(anyhow::anyhow!("system error in step_arb"));
                }
                if num_reverts == revenue_len {
                    return Err(anyhow::anyhow!("all swaps reverted"));
                }
            }

            // refine params and recurse
            let r_amount: rU256 = best_amount_in_out.0.into();
            let range = [
                if best_amount_in_out.0 < band_width {
                    0.into()
                } else {
                    best_amount_in_out.0 - band_width
                },
                if RUINT_MAX_U256 - r_amount < band_width.into() {
                    RUINT_MAX_U256.into()
                } else {
                    best_amount_in_out.0 + band_width
                },
            ];
            return step_arb(
                client,
                user_tx,
                block_info,
                params,
                Some(best_amount_in_out),
                range,
                intervals,
                Some(depth + 1),
            )
            .await;
        }
    } else {
        return step_arb(
            client,
            user_tx,
            block_info,
            params,
            Some(best_amount_in_out),
            range,
            intervals,
            Some(0),
        )
        .await;
    }
}

/// Find the optimal backrun for a given tx.
pub async fn find_optimal_backrun_amount_in_out(
    client: &WsClient,
    user_tx: Transaction,
    event: &EventHistory,
    block_info: &BlockInfo,
) -> Result<Vec<SimArbResult>> {
    let params = derive_trade_params(client, user_tx.to_owned(), event).await?;
    // let mut results_inner = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));
    let tasks: Vec<_> = params
        .iter()
        .map(|params| async {
            // set amount_in_start to however much eth the user sent. If the user sent a token, convert it to eth.
            let amount_in_start = if params.token_in == params.tokens.weth {
                if params.token0_is_weth {
                    params.amount0_sent.into_raw()
                } else {
                    params.amount1_sent.into_raw()
                }
            } else {
                if params.token0_is_weth {
                    params.amount1_sent.into_raw() * params.price
                } else {
                    params.amount0_sent.into_raw() * params.price
                }
            };
            let initial_range = [0.into(), amount_in_start];

            let client = client.clone();
            let user_tx = user_tx.clone();
            let block_info = block_info.clone();
            let params = params.clone();
            let results = results.clone();
            tokio::spawn(async move {
                /*





                        hey





                */
                let res = step_arb(
                    client.clone(),
                    user_tx,
                    block_info,
                    params.to_owned(),
                    None,
                    initial_range,
                    STEP_INTERVALS,
                    None,
                )
                .await;
                // TODO: here, log the result to a file
                debug!("*** step_arb complete: {:?}", res);
                if let Ok(res) = res {
                    let mut results = results.lock().await;
                    // let mut results_cp = results.to_vec();
                    results.push(SimArbResult {
                        amount_in: res.0,
                        balance_end: res.1,
                        trade_params: params,
                    });
                    // *results = results_cp;
                }
            })
            .await
        })
        .collect();
    future::join_all(tasks).await;
    let results = results.lock().await;
    Ok(results.to_vec())
}

/// Simulate a two-step arbitrage on a forked EVM.
pub async fn sim_arb(
    client: &WsClient,
    user_tx: Transaction,
    block_info: &BlockInfo,
    params: &UserTradeParams,
    amount_in: U256,
) -> Result<(U256, U256)> {
    let mut evm = fork_evm(client, block_info).await?;
    sim_bundle(&mut evm, vec![user_tx.to_owned()]).await?;

    // look at price (TKN/ETH) on each exchange to determine trade direction
    // if priceA > priceB after user tx creates price impact, then buy TKN on exchange B and sell on exchange A
    let mut other_pools = get_other_pair_addresses(
        client,
        (params.token_in, params.token_out),
        params.pool_variant,
    )
    .await?;
    other_pools.sort_by(|a, b| b.cmp(a)); // sort so that 0 address would be last; if it exists, non-0 address will be first
    let other_pool = other_pools[0];
    if other_pool == H160::zero() {
        return Err(HindsightError::PoolNotFound(params.pool).into());
    } else {
        debug!("other pool found: {:?}", other_pool);
    }
    // params.price
    let alt_price = match params.pool_variant {
        PoolVariant::UniswapV2 => {
            sim_price_v3(other_pool, params.token_in, params.token_out, &mut evm).await?
        }
        PoolVariant::UniswapV3 => {
            sim_price_v2(other_pool, params.token_in, params.token_out, &mut evm).await?
        }
    };
    debug!("alt price {:?}", alt_price);

    // if the price is denoted in TKN/ETH, we want to buy where the price is highest
    // if the price is denoted in ETH/TKN, we want to buy where the price is lowest
    // price is always denoted in tkn1/tkn0

    let (start_pool, start_pool_variant, end_pool) = if params.token0_is_weth {
        // if tkn0 is weth, then price is denoted in tkn1/eth, so look for highest price
        if params.price.gt(&alt_price) {
            (params.pool, params.pool_variant, other_pool)
        } else {
            (other_pool, params.pool_variant.other(), params.pool)
        }
    } else {
        // else if tkn1 is weth, then price is denoted in eth/tkn0, so look for lowest price
        if params.price.gt(&alt_price) {
            (other_pool, params.pool_variant.other(), params.pool)
        } else {
            (params.pool, params.pool_variant, other_pool)
        }
    };

    /* Buy tokens on one exchange. */
    let res = commit_braindance_swap(
        &mut evm,
        start_pool_variant,
        amount_in,
        start_pool,
        params.tokens.weth,
        params.tokens.token,
        block_info.base_fee,
        None,
    );
    debug!("braindance 1 completed. {:?}", res);
    let amount_received = res.unwrap_or(0.into());
    debug!("amount received {:?}", amount_received);

    /* Sell them on other exchange. */
    let res = commit_braindance_swap(
        &mut evm,
        start_pool_variant.other(),
        amount_received,
        end_pool,
        params.tokens.token,
        params.tokens.weth,
        block_info.base_fee + (block_info.base_fee * 2500) / 10000,
        None,
    )?;
    debug!("braindance 2 completed. {:?}", res);
    Ok((amount_in, res))
}

fn inject_tx(evm: &mut EVM<ForkDB>, tx: &Transaction) -> Result<()> {
    evm.env.tx.caller = B160::from(tx.from);
    evm.env.tx.transact_to = TransactTo::Call(B160::from(tx.to.unwrap_or_default().0));
    evm.env.tx.data = tx.input.to_owned().0;
    evm.env.tx.value = tx.value.into();
    evm.env.tx.chain_id = tx.chain_id.map(|id| id.as_u64());
    evm.env.tx.gas_limit = tx.gas.as_u64();
    match tx.transaction_type {
        Some(ethers::types::U64([0])) => {
            evm.env.tx.gas_price = tx.gas_price.unwrap_or_default().into();
        }
        Some(_) => {
            // type-2 tx
            evm.env.tx.gas_priority_fee = tx.max_priority_fee_per_gas.map(|fee| fee.into());
            evm.env.tx.gas_price = tx.max_fee_per_gas.unwrap_or_default().into();
        }
        None => {
            // legacy tx
            evm.env.tx.gas_price = tx.gas_price.unwrap_or_default().into();
        }
    }
    Ok(())
}

/// Execute a transaction on the forked EVM, commiting its state changes to the EVM's ForkDB.
pub async fn commit_tx(evm: &mut EVM<ForkDB>, tx: Transaction) -> Result<ExecutionResult> {
    inject_tx(evm, &tx)?;
    let res = evm.transact_commit();
    Ok(res.map_err(|err| anyhow::anyhow!("failed to simulate tx {:?}: {:?}", tx.hash, err))?)
}

pub async fn call_tx(evm: &mut EVM<ForkDB>, tx: Transaction) -> Result<ResultAndState> {
    inject_tx(evm, &tx)?;
    let res = evm.transact();
    Ok(res.map_err(|err| anyhow::anyhow!("failed to simulate tx {:?}: {:?}", tx.hash, err))?)
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

/// Execute a braindance swap on the forked EVM, commiting its state changes to the EVM's ForkDB.
///
/// Returns balance of token_out after tx is executed.
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
        PoolVariant::UniswapV2 => {
            braindance::build_swap_v2_data(amount_in, target_pool, token_in, token_out)
        }
        PoolVariant::UniswapV3 => braindance::build_swap_v3_data(
            I256::from_raw(amount_in),
            target_pool,
            token_in,
            token_out,
        ),
    };

    evm.env.tx.caller = braindance_controller_address();
    evm.env.tx.transact_to = TransactTo::Call(braindance_address().0.into());
    evm.env.tx.data = swap_data.0;
    evm.env.tx.gas_limit = 700000;
    evm.env.tx.gas_price = base_fee.into();
    evm.env.tx.value = rU256::ZERO;

    let res = match evm.transact_commit() {
        Ok(res) => res,
        Err(e) => return Err(anyhow::anyhow!("failed to commit swap: {:?}", e)),
    };
    let output = match res.to_owned() {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(o) => o,
            Output::Create(o, _) => o,
        },
        ExecutionResult::Revert { output, gas_used } => {
            return Err(anyhow::anyhow!(
                "swap reverted: {:?} (gas used: {:?})",
                output,
                gas_used
            ))
        }
        ExecutionResult::Halt { reason, .. } => {
            return Err(anyhow::anyhow!("swap halted: {:?}", reason))
        }
    };
    let (_amount_out, balance) = match pool_variant {
        PoolVariant::UniswapV2 => match braindance::decode_swap_v2_result(output.into()) {
            Ok(output) => output,
            Err(e) => return Err(anyhow::anyhow!("failed to decode swap result: {:?}", e)),
        },
        PoolVariant::UniswapV3 => match braindance::decode_swap_v3_result(output.into()) {
            Ok(output) => output,
            Err(e) => return Err(anyhow::anyhow!("failed to decode swap result: {:?}", e)),
        },
    };
    Ok(balance)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::{get_block_info, get_ws_client, ETH};
    use anyhow::Result;
    use ethers::providers::Middleware;
    use rusty_sando::simulate::braindance_starting_balance;

    async fn setup_test_evm(client: &WsClient, block_num: u64) -> Result<EVM<ForkDB>> {
        let block_info = get_block_info(&client, block_num).await?;
        fork_evm(&client, &block_info).await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_simulates_tx() -> Result<()> {
        let client = get_ws_client(Some("ws://localhost:8545".to_owned())).await?;
        let block_num = client.get_block_number().await?;
        let mut evm = setup_test_evm(&client, block_num.as_u64() - 1).await?;
        let block = client.get_block(block_num).await?.unwrap();
        let tx_hash = block.transactions[0];
        let tx = client.get_transaction(tx_hash).await?.unwrap();
        let res = sim_bundle(&mut evm, vec![tx]).await;
        assert!(res.is_ok());
        let res = res.unwrap();
        assert!(res[0].is_success());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_simulates_swaps() -> Result<()> {
        let client = get_ws_client(Some("ws://localhost:8545".to_owned())).await?;
        let block_num = client.get_block_number().await?;
        let mut evm = setup_test_evm(&client, block_num.as_u64() - 1).await?;
        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>()?;
        let tkn = "0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE".parse::<Address>()?; // SHIB
        let pool = get_other_pair_addresses(&client, (weth, tkn), PoolVariant::UniswapV3).await?[0];
        debug!("starting balance: {:?}", braindance_starting_balance());
        // buy 10 ETH worth of SHIB
        let res = commit_braindance_swap(
            &mut evm,
            PoolVariant::UniswapV2,
            ETH * 10,
            pool,
            weth,
            tkn,
            U256::from(1000000000) * 420,
            None,
        )?;
        // sell 10 ETH worth of SHIB
        let _ = commit_braindance_swap(
            &mut evm,
            PoolVariant::UniswapV2,
            res,
            pool,
            tkn,
            weth,
            U256::from(1000000000) * 420,
            None,
        )?;
        Ok(())
    }
}
