use crate::config::Config;
use crate::Result;

use mongodb::{options::ClientOptions, Client as DbClient, Database};
use std::sync::Arc;

const DB_NAME: &'static str = "hindsight";
const PROJECT_NAME: &'static str = "simulator";

#[derive(Clone, Debug)]
pub struct DbConnect {
    // _client: Arc<DbClient>,
    pub db: Arc<Database>,
}

pub struct DbFactory {
    name: String,
    url: String,
}

/// Creates a connected Db instance.
impl DbFactory {
    pub async fn init(self) -> Result<DbConnect> {
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
        Ok(DbConnect { db })
    }
}

/// Talks to the database.
impl DbConnect {
    pub fn new(name: Option<String>) -> DbFactory {
        let url = Config::default().db_url;
        DbFactory {
            name: name.unwrap_or(DB_NAME.to_owned()),
            url,
        }
    }
}
