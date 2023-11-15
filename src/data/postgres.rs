use super::arbs::{ArbDb, ArbFilterParams, WriteEngine};
use crate::{
    interfaces::{
        BackrunResult, PoolVariant, SimArbResult, SimArbResultBatch, StoredArbsRanges, TokenPair,
        UserTradeParams,
    },
    Result,
};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use ethers::{
    types::{Address, H256, I256, U256},
    utils::{format_ether, parse_ether},
};
use futures::future::join_all;
use mev_share_sse::{EventHistory, Hint};
use rust_decimal::prelude::*;
use std::sync::Arc;
use tokio_postgres::{connect, Client, NoTls};

const ARBS_TABLE: &str = "hindsight";

pub struct PostgresConnect {
    client: Arc<Client>,
}

#[derive(Clone, Debug)]
pub struct PostgresConfig {
    pub url: String,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        let config = crate::config::Config::default();
        Self {
            url: config.postgres_url.unwrap(),
        }
    }
}

fn where_filter(filter: &ArbFilterParams) -> String {
    let mut params = vec![];
    if let Some(block_start) = filter.block_start {
        params.push(format!("block_number >= {}", block_start));
    }
    if let Some(block_end) = filter.block_end {
        params.push(format!("block_number <= {}", block_end));
    }
    if let Some(timestamp_start) = filter.timestamp_start {
        params.push(format!("event_timestamp >= {}", timestamp_start));
    }
    if let Some(timestamp_end) = filter.timestamp_end {
        params.push(format!("event_timestamp <= {}", timestamp_end));
    }
    if let Some(min_profit) = filter.min_profit {
        params.push(format!("profit__eth__ >= {}", format_ether(min_profit)));
    }
    if let Some(token_pair) = &filter.token_pair {
        let token0 = if token_pair.token < token_pair.weth {
            token_pair.token
        } else {
            token_pair.weth
        };
        let token1 = if token_pair.token > token_pair.weth {
            token_pair.token
        } else {
            token_pair.weth
        };
        params.push(format!(
            "token_0 = '{:?}' AND token_1 = '{:?}'",
            token0, token1
        ));
    }
    params.join(" AND ")
}

fn select_arbs_query(filter: &ArbFilterParams, limit: Option<i64>, offset: Option<u64>) -> String {
    let mut query = "SELECT * FROM ".to_string();
    let where_clause = where_filter(filter);
    query.push_str(ARBS_TABLE);
    if where_clause.len() > 0 {
        query.push_str(" WHERE ");
        query.push_str(&where_clause);
    }
    query.push_str(" ORDER BY event_timestamp");
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {}", limit));
    }
    if let Some(offset) = offset {
        query.push_str(&format!(" OFFSET {}", offset));
    }
    println!("query: {}", query);
    query
}

fn count_arbs_query(filter: &ArbFilterParams) -> String {
    let mut query = "SELECT COUNT(*) FROM ".to_string();
    query.push_str(ARBS_TABLE);
    let where_clause = where_filter(filter);
    if where_clause.len() > 0 {
        query.push_str(" WHERE ");
        query.push_str(&where_clause);
    }
    query
}

impl PostgresConnect {
    pub async fn new(config: PostgresConfig) -> Result<Self> {
        // TODO: add env var for postgres tls if/when implemented
        // let pg_tls = false;
        // let tls = if pg_tls {
        //     OpenSsl...
        // } else {
        //     NoTls
        // };
        let (client, connection) = connect(&config.url, NoTls).await?;
        // The connection object performs the actual communication with the database,
        // so spawn it off to run on its own.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // create arbs table pessimistically (simplified version for now)
        client
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                        tx_hash VARCHAR(66) NOT NULL PRIMARY KEY,
                        profit__eth__ NUMERIC,
                        event_block INTEGER NOT NULL,
                        event_timestamp TIMESTAMP NOT NULL,
                        token_0 VARCHAR(42),
                        token_1 VARCHAR(42)
                    )",
                    ARBS_TABLE
                ),
                &[],
            )
            .await?;

        Ok(Self {
            client: Arc::new(client),
        })
    }
}

#[async_trait]
impl ArbDb for PostgresConnect {
    async fn write_arbs(&self, arbs: &[SimArbResultBatch]) -> Result<()> {
        let handles = arbs
            .iter()
            .map(|arb| {
                let txhash = format!("{:?}", arb.event.hint.hash); // must be a better way than this :\
                let max_profit = Decimal::from_str(&format_ether(arb.max_profit))
                    .expect("failed to encode profit");
                let timestamp =
                    NaiveDateTime::from_timestamp_millis(arb.event.timestamp as i64 * 1000)
                        .expect("failed to parse timestamp");

                println!("writing arb to postgres: {} {} eth", txhash, max_profit);
                // clone these to give to the tokio thread
                let client = self.client.clone();
                let arb = arb.clone();

                let trade = &arb.results[0].user_trade;
                let token0 = if trade.tokens.token < trade.tokens.weth { trade.tokens.token } else { trade.tokens.weth };
                let token1 = if trade.tokens.token > trade.tokens.weth { trade.tokens.token } else { trade.tokens.weth };

                tokio::task::spawn(async move {
                    client
                .execute(
                    &format!("INSERT INTO {} (tx_hash, profit__eth__, event_block, event_timestamp, token_0, token_1)
                        VALUES ($1, $2, $3, $4, $5, $6)
                        ON CONFLICT (tx_hash) DO UPDATE SET profit__eth__ = $2",
                        ARBS_TABLE
                    ),
                    &[
                        &txhash,
                        &max_profit,
                        &(arb.event.block as i32),
                        &timestamp,
                        &format!("{:?}", token0),
                        &format!("{:?}", token1),
                    ],
                )
                .await.expect("failed to write arb to postgres");
                })
            })
            .collect::<Vec<_>>();
        join_all(handles).await;
        Ok(())
    }

    async fn get_num_arbs(&self, filter_params: &ArbFilterParams) -> Result<u64> {
        let query = count_arbs_query(filter_params);
        let row = self.client.query_one(&query, &[]).await?;
        let count: u32 = row.get(0);
        Ok(count as u64)
    }

    async fn read_arbs(
        &self,
        filter_params: &ArbFilterParams,
        offset: Option<u64>,
        limit: Option<i64>,
    ) -> Result<Vec<SimArbResultBatch>> {
        let query = select_arbs_query(filter_params, limit, offset);
        let rows = self.client.query(&query, &[]).await?;
        let arbs = rows
            .into_iter()
            .map(|row| {
                let token0 = row
                    .get::<_, String>(4)
                    .parse::<Address>()
                    .expect("invalid token address");
                let token1 = row
                    .get::<_, String>(5)
                    .parse::<Address>()
                    .expect("invalid token address");
                SimArbResultBatch {
                    event: EventHistory {
                        // TODO: change this once the rest of the fields are added to postgres
                        block: row.get::<_, i32>(2) as u64,
                        timestamp: row.get::<_, NaiveDateTime>(3).timestamp() as u64,
                        hint: Hint {
                            txs: vec![],
                            hash: H256::from_str(&row.get::<_, String>(0)).unwrap(),
                            logs: vec![],
                            gas_used: None,
                            mev_gas_price: None,
                        },
                    },
                    max_profit: parse_ether(row.get::<_, Decimal>(1).to_string())
                        .unwrap_or(U256::zero()),
                    results: vec![SimArbResult {
                        user_trade: UserTradeParams {
                            pool_variant: PoolVariant::UniswapV2,
                            // TODO: support tokenIn/Out instead of token0/1
                            token_in: Address::zero(),
                            token_out: Address::zero(),
                            amount0_sent: I256::zero(),
                            amount1_sent: I256::zero(),
                            token0_is_weth: false,
                            pool: Address::zero(),
                            price: U256::zero(),
                            tokens: TokenPair {
                                token: token0,
                                weth: token1,
                            },
                            arb_pools: vec![],
                        },
                        backrun_trade: BackrunResult {
                            // TODO: fill with real data once it's added to the postgres table schema
                            amount_in: U256::from(0),
                            balance_end: U256::from(1),
                            profit: U256::from(1),
                            start_pool: Address::zero(),
                            end_pool: Address::zero(),
                            start_variant: PoolVariant::UniswapV2,
                            end_variant: PoolVariant::UniswapV2,
                        },
                    }],
                }
            })
            .collect::<Vec<_>>();
        Ok(arbs)
    }

    async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges> {
        todo!()
    }

    async fn export_arbs(
        &self,
        _write_dest: WriteEngine,
        _filter_params: &ArbFilterParams,
    ) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use ethers::types::H160;

    use super::*;
    use crate::{config::Config, interfaces::TokenPair};

    #[tokio::test]
    async fn it_connects_postgres() -> Result<()> {
        let config = Config::default();
        if config.postgres_url.is_none() {
            println!("no postgres url, skipping test");
            return Ok(());
        }
        let connect = PostgresConnect::new(PostgresConfig {
            url: config.postgres_url.unwrap(),
        })
        .await?;
        let res = connect
            .client
            .execute("CREATE TABLE test001 (id serial)", &[])
            .await;
        assert!(res.is_ok());
        let res = connect.client.execute("DROP TABLE test001", &[]).await;
        assert!(res.is_ok());
        Ok(())
    }

    async fn get_db() -> Result<PostgresConnect> {
        let config = Config::default();
        PostgresConnect::new(PostgresConfig {
            url: config.postgres_url.unwrap(),
        })
        .await
    }

    #[tokio::test]
    async fn it_writes_arbs_postgres() -> Result<()> {
        let db = get_db().await?;
        let res = db
            .write_arbs(&vec![SimArbResultBatch::test_example()])
            .await;
        assert!(res.is_ok());

        let res = db.read_arbs(&ArbFilterParams::none(), None, None).await?;
        assert!(res.len() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn it_reads_from_db() -> Result<()> {
        let db = get_db().await?;
        let res = db
            .write_arbs(&vec![SimArbResultBatch::test_example()])
            .await;
        assert!(res.is_ok());
        let token_pair = TokenPair {
            token: "0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE"
                .to_lowercase()
                .parse::<H160>()
                .unwrap(),
            weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
                .to_lowercase()
                .parse::<H160>()
                .unwrap(),
        };
        let arbs = db
            .read_arbs(
                &ArbFilterParams {
                    block_start: None,
                    block_end: None,
                    timestamp_start: None,
                    timestamp_end: None,
                    min_profit: None,
                    token_pair: Some(token_pair.to_owned()),
                },
                None,
                Some(1),
            )
            .await?;
        println!("arbs (it_reads_from_db): {:?}", arbs);
        assert!(arbs.len() == 1);
        assert!(arbs[0].results[0].user_trade.tokens.token == token_pair.token);
        Ok(())
    }
}
