use super::arbs::{ArbFilterParams, ArbInterface, WriteEngine};
use crate::{
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use ethers::utils::format_ether;
use futures::future::join_all;
use rust_decimal::prelude::*;
use std::sync::Arc;
use tokio_postgres::{connect, Client, NoTls};
// use postgres_openssl; // TODO: support postgres tls

const ARBS_TABLE: &'static str = "hindsight";

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
                        profit__eth__ NUMERIC NOT NULL
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
impl ArbInterface for PostgresConnect {
    async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()> {
        let handles = arbs.iter().map(|arb| {
            let txhash = format!("{:?}", arb.event.hint.hash); // must be a better way than this :\
            let profit = Decimal::from_str(&format_ether(arb.max_profit)).expect("failed to encode profit");

            println!("writing arb to postgres: {} {}", txhash.to_string(), arb.max_profit);
            let client = self.client.clone();
            tokio::task::spawn(async move {
                client
                .execute(
                    &format!("INSERT INTO {} (tx_hash, profit__eth__) VALUES ($1, $2) ON CONFLICT (tx_hash) DO UPDATE SET profit__eth__ = $2", ARBS_TABLE),
                    &[&txhash, &profit],
                )
                .await.expect("failed to write arb to postgres");
            })
        }).collect::<Vec<_>>();
        join_all(handles).await;
        Ok(())
    }

    async fn get_num_arbs(&self, _filter_params: &ArbFilterParams) -> Result<u64> {
        todo!()
    }

    async fn read_arbs(
        &self,
        _filter_params: &ArbFilterParams,
        _offset: Option<u64>,
        _limit: Option<i64>,
    ) -> Result<Vec<SimArbResultBatch>> {
        todo!()
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
        assert!(res.len() > 0);
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
