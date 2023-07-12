use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url_ws: String,
    pub auth_signer_key: String,
}

impl Default for Config {
    fn default() -> Config {
        dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err))
            .unwrap();
        Config {
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
            auth_signer_key: env::var("AUTH_SIGNER_KEY").expect("AUTH_SIGNER_KEY must be set"),
        }
    }
}
