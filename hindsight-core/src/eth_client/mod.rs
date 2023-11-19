use crate::Result;
use ethers::{
    middleware::Middleware,
    providers::{Provider, Ws},
    types::{Block, BlockTrace, TraceType, Transaction},
};
use std::sync::Arc;

pub type WsProvider = Provider<Ws>;

#[derive(Clone, Debug)]
pub struct WsClient {
    pub provider: WsProvider,
}

/*
TODO: implement this in a cooler, more edgier way
*/
impl WsClient {
    pub async fn new(rpc_url: Option<String>, max_reconnects: usize) -> Result<Self> {
        let rpc_url = if let Some(rpc_url) = rpc_url {
            rpc_url
        } else {
            "ws://localhost:18545".to_owned()
        };
        let provider = Provider::<Ws>::connect_with_reconnects(rpc_url, max_reconnects).await?;
        Ok(Self { provider })
    }

    /// this feels hacky... is there a better way to do this?
    pub fn get_provider(&self) -> Arc<WsProvider> {
        Arc::new(self.provider.clone())
    }

    /// Get the state diff produced by executing all transactions in the provided block.
    pub async fn get_block_traces(&self, block: &Block<Transaction>) -> Result<Vec<BlockTrace>> {
        let req = block
            .transactions
            .iter()
            .map(|tx| (tx, vec![TraceType::StateDiff]))
            .collect();
        Ok(self
            .provider
            .trace_call_many(req, block.number.map(|b| b.into()))
            .await?)
    }
}

impl From<WsProvider> for WsClient {
    fn from(provider: WsProvider) -> Self {
        Self { provider }
    }
}

pub mod test {
    use super::WsClient;
    use crate::Result;

    pub async fn get_test_ws_client() -> Result<WsClient> {
        let ws_client = WsClient::new(None, 1).await?;
        Ok(ws_client)
    }
}
