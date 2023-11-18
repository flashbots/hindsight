use super::evm::state_diff::StateDiff;
use crate::{
    error::HindsightError,
    evm::{
        fork_db::ForkDB, fork_factory::ForkFactory, state_diff::ToCacheDb, util::set_block_state,
    },
    Result,
};
use ethers::{
    middleware::Middleware,
    providers::{Provider, Ws},
    types::{Block, BlockNumber, BlockTrace, TraceType, Transaction},
};
use revm::EVM;
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
            "ws://localhost:8545".to_owned()
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

    /// Return an evm instance forked from the provided block info and client state
    /// with braindance module initialized.
    /// Braindance contracts starts w/ braindance_starting_balance, which is 420 WETH.
    pub async fn fork_evm(&self, block_num: u64) -> Result<EVM<ForkDB>> {
        let fork_block_num = BlockNumber::Number(block_num.into());
        let fork_block = Some(ethers::types::BlockId::Number(fork_block_num));

        // get txs from block
        let block = self
            .provider
            .get_block_with_txs(block_num)
            .await?
            .ok_or(HindsightError::BlockNotFound(block_num))?;

        let block_traces = self.get_block_traces(&block).await?;
        let state_diff = StateDiff::from_block_traces(block_traces).await?;
        let cache_db = state_diff
            .to_cache_db(&self.get_provider(), Some(block_num.into()))
            .await?;

        let mut fork_factory =
            ForkFactory::new_sandbox_factory(self.get_provider(), cache_db, fork_block);

        // TODO: replace this
        // attach_braindance_module(&mut fork_factory);

        let mut evm = EVM::new();
        evm.database(fork_factory.new_sandbox_fork());
        set_block_state(&mut evm, &block.into());
        Ok(evm)
    }
    // TODO: migrate from util.rs to this impl: all methods
    // taking a WsClient argument
}

impl From<WsProvider> for WsClient {
    fn from(provider: WsProvider) -> Self {
        Self { provider }
    }
}

// #[cfg(test)]
pub mod test {
    use super::WsClient;
    use crate::Result;

    pub async fn get_test_ws_client() -> Result<WsClient> {
        let ws_client = WsClient::new(None, 1).await?;
        Ok(ws_client)
    }
}
