use data::{MongoConfig, PostgresConfig};
use dotenvy::dotenv;
use hindsight_core::{debug, interfaces::Config};
use lazy_static::lazy_static;
use std::env;

lazy_static! {
    static ref CONFIG: Config = DotEnv::from_env();
}

pub trait DotEnv {
    fn from_env() -> Self;
}

impl DotEnv for Config {
    fn from_env() -> Self {
        let env_file_res = dotenv();
        if let Err(err) = env_file_res {
            debug!("{}", err);
        }
        println!("env {:#?}", env::vars());
        Config {
            mongo_url: env::var("MONGO_URL").expect("MONGO_URL must be set"),
            postgres_url: env::var("POSTGRES_URL").ok(),
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
            tls_ca_file_mongo: env::var("TLS_CA_FILE_MONGO").map(|s| s.into()).ok(),
        }
    }
}

impl DotEnv for MongoConfig {
    fn from_env() -> Self {
        MongoConfig {
            url: env::var("MONGO_URL").expect("MONGO_URL must be set"),
            tls_ca_file_path: env::var("TLS_CA_FILE_MONGO").map(|s| s.into()).ok(),
        }
    }
}

impl DotEnv for PostgresConfig {
    fn from_env() -> Self {
        PostgresConfig {
            url: env::var("POSTGRES_URL").expect("POSTGRES_URL must be set"),
        }
    }
}
