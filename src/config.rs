use crate::debug;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url_ws: String,
    pub db_url: String,
    pub log_to_file: bool,
}

impl Config {
    pub fn new(rpc_url_ws: String, db_url: String, log_to_file: bool) -> Config {
        Config {
            rpc_url_ws,
            db_url,
            log_to_file,
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        let env_file_res = dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err));
        if let Err(err) = env_file_res {
            debug!("{}", err);
        }
        Config {
            db_url: env::var("DB_URL").expect("DB_URL must be set"),
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
            log_to_file: env::var("LOG_TO_FILE")
                .map(|v| v.to_string() == "true")
                .unwrap_or(false),
        }
    }
}
