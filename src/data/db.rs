use crate::{
    data::{
        arbs::ArbDatabase,
        mongo::{MongoConfig, MongoConnect},
        postgres::{PostgresConfig, PostgresConnect},
    },
    Result,
};
use std::sync::Arc;
use strum::{EnumIter, IntoEnumIterator};

pub struct Db {
    pub connect: ArbDatabase,
}

#[derive(Clone, Debug, EnumIter)]
pub enum DbEngine {
    Mongo(MongoConfig),
    Postgres(PostgresConfig),
}

impl DbEngine {
    pub fn enum_flags() -> String {
        DbEngine::iter()
            .map(|engine| engine.to_string())
            .reduce(|a, b| format!("{} | {}", a, b))
            .expect("failed to reduce db engines to string")
            
    }
}

impl Default for DbEngine {
    fn default() -> Self {
        DbEngine::Mongo(MongoConfig::default())
    }
}

impl std::fmt::Display for DbEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbEngine::Mongo(_) => write!(f, "mongo"),
            DbEngine::Postgres(_) => write!(f, "postgres"),
        }
    }
}

// serialize/deserialize from string
impl std::str::FromStr for DbEngine {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mongo" => Ok(DbEngine::Mongo(MongoConfig::default())),
            "postgres" => Ok(DbEngine::Postgres(PostgresConfig::default())),
            _ => Err(format!("invalid db engine: {}", s)),
        }
    }
}

impl Db {
    pub async fn new(engine: DbEngine) -> Self {
        match engine {
            DbEngine::Mongo(config) => {
                Db {
                    connect: Arc::new(MongoConnect::new(config.to_owned()).await.unwrap_or_else(
                        |_| panic!("failed to connect to mongo db at {}", config.url),
                    )),
                }
            }
            DbEngine::Postgres(config) => Db {
                connect: Arc::new(
                    PostgresConnect::new(config.to_owned())
                        .await
                        .unwrap_or_else(|_| {
                            panic!("failed to connect to postgres db at {:?}", config.url)
                        }),
                ),
            },
        }
    }
}
