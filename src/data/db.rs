// TODO: TEST, THEN ADD POSTGRES SUPPORT
use std::sync::Arc;
use strum::{EnumIter, IntoEnumIterator};

use super::{arbs::ArbDatabase, mongo::MongoConnect};

pub struct Db {
    pub connect: ArbDatabase,
}

#[derive(Clone, Debug, EnumIter)]
pub enum DbEngine {
    Mongo,
    // Postgres,
}

impl DbEngine {
    pub fn enum_flags() -> String {
        format!(
            "{}",
            DbEngine::iter()
                .map(|engine| engine.to_string())
                .reduce(|a, b| format!("{} | {}", a, b))
                .expect("failed to reduce db engines to string")
        )
    }
}

impl Default for DbEngine {
    fn default() -> Self {
        // TODO: make this postgres
        DbEngine::Mongo
    }
}

impl std::fmt::Display for DbEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbEngine::Mongo => write!(f, "mongo"),
            // DbEngine::Postgres => write!(f, "postgres"),
        }
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
            },
            // DbEngine::Postgres => Db {
            //     connect: Arc::new(MongoConnect::new(db_name).await.expect(&format!(
            //         "failed to connect to mongo db={}",
            //         db_name.unwrap_or("(default)")
            //     ))),
            // },
        }
    }
}
