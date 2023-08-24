use std::sync::Arc;
use super::arbs::{ArbFilterParams, ArbInterface, WriteEngine};
use crate::{
    err,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use ethers::utils::format_ether;
use futures::future::join_all;
use tokio_postgres::{connect, Client, NoTls};
use ethers::abi::AbiEncode;
use rust_decimal::prelude::*;
// use postgres_openssl::;

const ARBS_TABLE: &'static str = "mev_lower_bound";

pub struct PostgresConnect {
    client: Arc<Client>,
}

impl PostgresConnect {
    pub async fn new(db_url: String) -> Result<Self> {
        // TODO: add env var
        // let pg_tls = false;
        // let tls = if pg_tls {
        //     OpenSsl...
        // } else {
        //     NoTls
        // };
        let (client, connection) = connect(&db_url, NoTls).await?;
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
                        profit NUMERIC NOT NULL
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

    // TODO: DELETE THIS; ONLY USED FOR TESTING
    // pub async fn drop_arbs(&self) -> Result<()> {
    //     self.client.execute("DROP TABLE arbs", &[]).await?;
    //     Ok(())
    // }
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
                    &format!("INSERT INTO {} (tx_hash, profit) VALUES ($1, $2) ON CONFLICT (tx_hash) DO UPDATE SET profit = $2", ARBS_TABLE),
                    &[&txhash, &profit],
                )
                .await.expect("failed to write arb to postgres");
            })
        }).collect::<Vec<_>>();
        join_all(handles).await;
        Ok(())
    }

    async fn read_arbs(&self, _filter_params: ArbFilterParams) -> Result<Vec<SimArbResultBatch>> {
        err!("unimplemented")
    }

    async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges> {
        err!("unimplemented")
    }

    async fn export_arbs(
        &self,
        _write_dest: WriteEngine,
        _filter_params: ArbFilterParams,
    ) -> Result<()> {
        err!("unimplemented")
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
        let connect = PostgresConnect::new(config.postgres_url.unwrap()).await?;
        let res = connect
            .client
            .execute("CREATE TABLE test001 (id serial)", &[])
            .await;
        assert!(res.is_ok());
        let res = connect.client.execute("DROP TABLE test001", &[]).await;
        assert!(res.is_ok());
        Ok(())
    }

    // TODO: DELETE THIS; ONLY USED FOR TESTING
    // #[tokio::test]
    // async fn it_drops_arbs_postgres() -> Result<()> {
    //     let config = Config::default();
    //     if config.postgres_url.is_none() {
    //         println!("no postgres url, skipping test");
    //         return Ok(());
    //     }
    //     let connect = PostgresConnect::new(config.postgres_url.unwrap()).await?;
    //     connect.drop_arbs().await?;
    //     Ok(())
    // }

    // #[tokio::test]
    // async fn it_writes_to_db() -> Result<()> {
    //     let config = Config::default();
    //     let connect = PostgresConnect::new(config.postgres_url).await?;
    //     let arbs = vec![SimArbResultBatch::test_example()];
    //     connect.write_arbs(&arbs).await?;
    //     Ok(())
    // }

    async fn inject_test_arbs(connect: &PostgresConnect) -> Result<()> {
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
        let connect = PostgresConnect::new(config.postgres_url.unwrap()).await?;
        inject_test_arbs(&connect).await?;
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
