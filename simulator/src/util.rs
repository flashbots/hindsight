use anyhow::Result;
use ethers::providers::{Provider, Ws};
use std::sync::Arc;

pub async fn get_ws_provider(rpc_url: String) -> Result<Arc<Provider<Ws>>> {
    let provider = Provider::<Ws>::connect(rpc_url).await?;
    Ok(Arc::new(provider))
}
