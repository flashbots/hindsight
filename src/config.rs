use crate::debug;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url_ws: String,
    pub mongo_url: String,
    pub postgres_url: Option<String>,
    pub tls_enabled_mongo: bool,
    pub tls_enabled_postgres: bool,
}

impl Default for Config {
    fn default() -> Config {
        let env_file_res = dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err));
        if let Err(err) = env_file_res {
            debug!("{}", err);
        }
        let parse_bool = |x: String| x.parse::<bool>().expect("invalid boolean");
        Config {
            mongo_url: env::var("MONGO_URL").expect("MONGO_URL must be set"),
            postgres_url: env::var("POSTGRES_URL").ok(),
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
            tls_enabled_mongo: env::var("TLS_ENABLED_MONGO").map_or(false, parse_bool),
            tls_enabled_postgres: env::var("TLS_ENABLED_POSTGRES").map_or(false, parse_bool),
        }
    }
}
