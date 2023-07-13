use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub auth_signer_key: String,
    pub rpc_url_ws: String,
    pub db_url: String,
}

impl Default for Config {
    fn default() -> Config {
        dotenvy::dotenv()
            .map_err(|err| anyhow::anyhow!("Failed to load .env file. Error: {}", err))
            .unwrap(); // keep this unwrap -- we want to fail hard if .env is not loaded
        Config {
            auth_signer_key: env::var("AUTH_SIGNER_KEY").expect("AUTH_SIGNER_KEY must be set"),
            db_url: env::var("DB_URL").expect("DB_URL must be set"),
            rpc_url_ws: env::var("RPC_URL_WS").expect("RPC_URL_WS must be set"),
        }
    }
}
