use crate::config::Config;
use crate::interfaces::SimArbResultBatch;
use crate::Result;
use futures::stream::TryStreamExt;
use mongodb::{options::ClientOptions, Client as DbClient, Database};
use std::sync::Arc;

const DB_NAME: &'static str = "hindsight";
const PROJECT_NAME: &'static str = "simulator";
const ARB_COLLECTION: &'static str = "arbs";

#[derive(Clone, Debug)]
pub struct Db {
    // _client: Arc<DbClient>,
    pub db: Arc<Database>,
}

pub struct DbFactory {
    name: String,
    url: String,
}

/// Creates a connected Db instance.
impl DbFactory {
    pub async fn init(self) -> Result<Db> {
        let mut options = ClientOptions::parse(self.url).await?;
        options.app_name = Some(PROJECT_NAME.to_owned());
        // options.default_database = Some(DB_NAME.to_owned());
        options.credential = Some(
            mongodb::options::Credential::builder()
                .username("root".to_owned())
                .password("example".to_owned())
                .build(),
        );
        let db = Arc::new(DbClient::with_options(options)?.database(&self.name));
        Ok(Db { db })
    }
}

/// Talks to the database.
impl Db {
    pub fn new(name: Option<String>) -> DbFactory {
        let url = Config::default().db_url;
        DbFactory {
            name: name.unwrap_or(DB_NAME.to_owned()),
            url,
        }
    }

    pub async fn read_arbs(
        self,
        // limit: Option<i64>,
        // filter: Option<SimArbResultBatch>,
    ) -> Result<Vec<SimArbResultBatch>> {
        // let limit = limit.unwrap_or(100);
        // let mut options;
        // if let Some(filter) = filter {
        //     options = FindOptions::builder().filter(filter).build();
        // }
        // options = options.build();
        let collection = self.db.collection::<SimArbResultBatch>(ARB_COLLECTION);
        let mut cursor = collection.find(None, None).await?;
        let mut results = vec![];
        while let Some(res) = cursor.try_next().await? {
            results.push(res);
        }
        Ok(results)
    }

    pub async fn write_arbs(self, arbs: Vec<SimArbResultBatch>) -> Result<()> {
        let collection = self.db.collection::<SimArbResultBatch>(ARB_COLLECTION);
        collection.insert_many(arbs, None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Result;

    const TEST_DB: &'static str = "test_hindsight";

    #[tokio::test]
    async fn it_writes_to_db() -> Result<()> {
        let db = Db::new(Some(TEST_DB.to_owned())).init().await?;
        let arbs = vec![SimArbResultBatch::test_example()];
        db.write_arbs(arbs).await?;
        Ok(())
    }

    #[tokio::test]
    async fn it_reads_from_db() -> Result<()> {
        let db = Db::new(Some(TEST_DB.to_owned())).init().await?;
        let arbs = db.read_arbs().await?;
        println!("arbs: {:?}", arbs);
        Ok(())
    }
}
