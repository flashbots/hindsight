use crate::Result;
use ethers::providers::{Provider, Ws};
use std::sync::Arc;

pub type WsClient = Arc<Provider<Ws>>;

pub async fn get_ws_client(rpc_url: Option<String>, max_reconnects: usize) -> Result<WsClient> {
    let rpc_url = if let Some(rpc_url) = rpc_url {
        rpc_url
    } else {
        "ws://localhost:8545".to_owned()
    };
    let provider = Provider::<Ws>::connect_with_reconnects(rpc_url, max_reconnects).await?;
    Ok(Arc::new(provider))
}

// TODO: make a wrapper struct and migrate all methods from util.rs that require a WsClient
// to its impl

// #[cfg(test)]
pub mod test {
    use super::{get_ws_client, WsClient};
    use crate::Result;

    pub async fn get_test_ws_client() -> Result<WsClient> {
        let ws_client = get_ws_client(None, 1).await?;
        Ok(ws_client)
    }
}
