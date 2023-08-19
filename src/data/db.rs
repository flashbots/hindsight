// TODO: TEST, THEN ADD POSTGRES SUPPORT
use std::sync::Arc;

use super::{arbs::ArbDatabase, mongo::MongoConnect};

pub struct Db {
    pub connect: ArbDatabase,
}

#[derive(Clone, Debug)]
pub enum DbEngine {
    Mongo,
    // Postgres,
}

impl Default for DbEngine {
    fn default() -> Self {
        // TODO: make this postgres
        DbEngine::Mongo
    }
}

// serialize/deserialize from string
impl std::str::FromStr for DbEngine {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mongo" => Ok(DbEngine::Mongo),
            // "postgres" => Ok(DbEngine::Postgres),
            _ => Err(format!("invalid db engine: {}", s)),
        }
    }
}

impl Db {
    pub async fn new(engine: DbEngine, db_name: Option<&str>) -> Self {
        match engine {
            DbEngine::Mongo => Db {
                connect: Arc::new(MongoConnect::new(db_name).await.expect(&format!(
                    "failed to connect to mongo db={}",
                    db_name.unwrap_or("(default)")
                ))),
            }, // DbEngine::Postgres => Db { connect: }
        }
    }
}
