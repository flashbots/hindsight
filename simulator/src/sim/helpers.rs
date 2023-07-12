#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use ethers::utils::Anvil;

    use crate::config::Config;
    use crate::util::{get_ws_client as get_ws_client_core, WsClient};
    use crate::Result;

    pub async fn get_ws_client() -> Result<Arc<WsClient>> {
        let config = Config::default();
        let node = Arc::new(Anvil::new().fork(config.rpc_url_ws.as_str()).spawn());
        let client = get_ws_client_core(Some(node.ws_endpoint())).await?;
        Ok(Arc::new(client))
    }
}
