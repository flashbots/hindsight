use super::arbs::ArbInterface;
use crate::{config::Config, interfaces::SimArbResultBatch, Result};
use async_trait::async_trait;
use tokio_postgres::{connect, Client, NoTls};
// use postgres_openssl::;

pub struct PostgresConnect {
    client: Client,
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
        Ok(Self { client })
    }
}

// #[async_trait]
// impl ArbInterface for PostgresConnect {
//     async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()> {
//         Ok(())
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

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
            .execute("CREATE TABLE test001 (id serial)", &vec![])
            .await;
        assert!(res.is_ok());
        let res = connect.client.execute("DROP TABLE test001", &vec![]).await;
        assert!(res.is_ok());
        Ok(())
    }

    // #[tokio::test]
    // async fn it_writes_to_db() -> Result<()> {
    //     let config = Config::default();
    //     let connect = PostgresConnect::new(config.postgres_url).await?;
    //     let arbs = vec![SimArbResultBatch::test_example()];
    //     connect.write_arbs(&arbs).await?;
    //     Ok(())
    // }

    // #[tokio::test]
    // async fn it_reads_from_db() -> Result<()> {
    //     let config = Config::default();
    //     let connect = PostgresConnect::new(config.postgres_url).await?;
    //     let arbs = connect.read_arbs(ArbFilterParams::default()).await?;
    //     println!("arbs: {:?}", arbs);
    //     Ok(())
    // }
}
