use crate::warn;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url_ws: String,
    pub db_url: String,
}

impl Default for Config {
    fn default() -> Config {
        let env_file_res = dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err));
        if let Err(err) = env_file_res {
            warn!("{}", err);
        }
        Config {
            db_url: env::var("DB_URL").expect("DB_URL must be set"),
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
        }
    }
}
