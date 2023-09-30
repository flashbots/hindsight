use super::arbs::{ArbDb, ArbFilterParams, WriteEngine};
use crate::{
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use ethers::{
    types::{H256, U256},
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
        params.push(format!("timestamp >= {}", timestamp_start));
    }
    if let Some(timestamp_end) = filter.timestamp_end {
        params.push(format!("timestamp <= {}", timestamp_end));
    }
    if let Some(min_profit) = filter.min_profit {
        params.push(format!("profit__eth__ >= {}", format_ether(min_profit)));
    }
    params.join(" AND ")
}

fn select_arbs_query(filter: &ArbFilterParams) -> String {
    let mut query = "SELECT * FROM ".to_string();
    query.push_str(ARBS_TABLE);
    query.push_str(" WHERE ");
    query.push_str(&where_filter(filter));
    query
}

fn count_arbs_query(filter: &ArbFilterParams) -> String {
    let mut query = "SELECT COUNT(*) FROM ".to_string();
    query.push_str(ARBS_TABLE);
    query.push_str(" WHERE ");
    query.push_str(&where_filter(filter));
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

        // create arbs table pessimistically (simplified version for now: {hash, profit})
        client
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                        tx_hash VARCHAR(66) NOT NULL PRIMARY KEY,
                        profit__eth__ NUMERIC,
                        event_block INTEGER NOT NULL,
                        event_timestamp TIMESTAMP NOT NULL
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

                tokio::task::spawn(async move {
                    client
                .execute(
                    &format!("INSERT INTO {} (tx_hash, profit__eth__, event_block, event_timestamp)
                        VALUES ($1, $2, $3, $4)
                        ON CONFLICT (tx_hash) DO UPDATE SET profit__eth__ = $2",
                        ARBS_TABLE
                    ),
                    &[
                        &txhash,
                        &max_profit,
                        &(arb.event.block as i32),
                        &timestamp,
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
        _offset: Option<u64>,
        _limit: Option<i64>,
    ) -> Result<Vec<SimArbResultBatch>> {
        let query = select_arbs_query(filter_params);
        let rows = self.client.query(&query, &[]).await?;
        let arbs = rows
            .into_iter()
            .map(|row| SimArbResultBatch {
                event: EventHistory {
                    // TODO: change this once the rest of the fields are added to postgres
                    block: row.get::<usize, u32>(2) as u64,
                    timestamp: row.get::<usize, u32>(3) as u64,
                    hint: Hint {
                        txs: vec![],
                        hash: H256::from_str(&row.get::<_, String>(0)).unwrap(),
                        logs: vec![],
                        gas_used: None,
                        mev_gas_price: None,
                    },
                },
                max_profit: parse_ether(row.get::<usize, f64>(1).to_string())
                    .unwrap_or(U256::zero()),
                results: vec![],
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
    use super::*;
    use crate::config::Config;

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

    /// sends a test arb to the db
    async fn inject_test_arb(connect: &PostgresConnect) -> Result<()> {
        let arbs = vec![SimArbResultBatch::test_example()];
        connect.write_arbs(&arbs).await?;
        Ok(())
    }

    #[tokio::test]
    async fn it_writes_arbs_postgres() -> Result<()> {
        let config = Config::default();
        if config.postgres_url.is_none() {
            println!("no postgres url, skipping test");
            return Ok(());
        }
        let connect = PostgresConnect::new(PostgresConfig {
            url: config.postgres_url.unwrap(),
        })
        .await?;
        inject_test_arb(&connect).await?;
        let res = connect
            .client
            .query(&format!("SELECT * FROM {}", ARBS_TABLE), &[])
            .await
            .expect("failed to read arbs from postgres");
        assert!(!res.is_empty());
        Ok(())
    }

    // #[tokio::test]
    // async fn it_reads_from_db() -> Result<()> {
    //     let config = Config::default();
    //     let connect = PostgresConnect::new(config.postgres_url).await?;
    //     let arbs = connect.read_arbs(ArbFilterParams::default()).await?;
    //     println!("arbs: {:?}", arbs);
    //     Ok(())
    // }
}
