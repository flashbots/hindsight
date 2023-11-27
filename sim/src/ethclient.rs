use async_trait::async_trait;
use ethers::middleware::Middleware;
use ethers::types::BlockNumber;
use hindsight_core::{
    error::HindsightError,
    eth_client::WsClient,
    evm::{
        fork_db::ForkDB,
        fork_factory::ForkFactory,
        state_diff::{StateDiff, ToCacheDb},
        util::set_block_state,
    },
    Result,
};
use revm::EVM;

use crate::util::attach_braindance_module;

#[async_trait]
pub trait ForkEVM {
    async fn fork_evm(&self, block_num: u64) -> Result<EVM<ForkDB>>;
}

#[async_trait]
impl ForkEVM for WsClient {
    /// Return an evm instance forked from the provided block info and client state
    /// with braindance module initialized.
    /// Braindance contracts starts w/ braindance_starting_balance, which is 420 WETH.
    async fn fork_evm(&self, block_num: u64) -> Result<EVM<ForkDB>> {
        let fork_block_num = BlockNumber::Number(block_num.into());
        let fork_block = Some(ethers::types::BlockId::Number(fork_block_num));

        // get txs from block
        let block = self
            .provider
            .get_block_with_txs(block_num)
            .await?
            .ok_or(HindsightError::BlockNotFound(block_num))?;

        let block_traces = self.get_block_traces(&block).await?;
        let state_diff = StateDiff::from(block_traces);
        let cache_db = state_diff
            .to_cache_db(&self, Some(block_num.into()))
            .await?;

        let mut fork_factory =
            ForkFactory::new_sandbox_factory(self.arc_provider(), cache_db, fork_block);

        attach_braindance_module(&mut fork_factory);

        let mut evm = EVM::new();
        evm.database(fork_factory.new_sandbox_fork());
        set_block_state(&mut evm, &block.into());
        Ok(evm)
    }
}
