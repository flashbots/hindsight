use hindsight_core::{debug, format_err, interfaces::Config};
use std::env;

pub trait DotEnv {
    fn from_env() -> Self;
}

impl DotEnv for Config {
    fn from_env() -> Self {
        let env_file_res = dotenvy::dotenv()
            .map_err(|err| format_err!("Failed to load .env file. Error: {}", err));
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
