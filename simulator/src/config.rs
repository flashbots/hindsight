use crate::Result;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url_ws: String,
    pub auth_signer_key: String,
}

impl Config {
    pub fn load() -> Result<Config> {
        dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err))?;
        Ok(Config {
            rpc_url_ws: env::var("RPC_URL_WS")?,
            auth_signer_key: env::var("AUTH_SIGNER_KEY")?,
        })
    }
}
