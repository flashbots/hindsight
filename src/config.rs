use crate::debug;
use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url_ws: String,
    pub mongo_url: String,
    pub postgres_url: Option<String>,
    pub tls_ca_file_mongo: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Config {
        let env_file_res = dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err));
        if let Err(err) = env_file_res {
            debug!("{}", err);
        }
        Config {
            mongo_url: env::var("MONGO_URL").expect("MONGO_URL must be set"),
            postgres_url: env::var("POSTGRES_URL").ok(),
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
            tls_ca_file_mongo: env::var("TLS_CA_FILE_MONGO").map(|s| s.into()).ok(),
        }
    }
}
