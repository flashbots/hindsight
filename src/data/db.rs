// TODO: TEST, THEN ADD POSTGRES SUPPORT
use std::sync::Arc;

use super::{arbs::ArbDatabase, mongo::MongoConnect};

pub struct Db {
    pub connect: ArbDatabase,
}

pub enum DbEngine {
    Mongo,
    // Postgres,
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
