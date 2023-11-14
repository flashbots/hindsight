use crate::Error;
use ethers::types::{Address, H256};

#[derive(Clone, Debug)]
pub enum HindsightError {
    /// The specified block number could not be fetched.
    BlockNotFound(u64),
    /// The specified transaction hash could not be found in the event map.
    EventNotCached(H256),
    /// Could not find another trading pool with same tokens as the given pool.
    PoolNotFound(Address),
    /// Could not find transaction onchain.
    TxNotLanded(H256),
    /// Failed to call smart contract.
    CallError(String),
    /// Failed to perform math operation.
    MathError(String),
    /// Failed to parse data into revm core types.
    EvmParseError(String),
}

impl From<HindsightError> for Error {
    fn from(val: HindsightError) -> Self {
        match val {
            HindsightError::BlockNotFound(block_number) => {
                anyhow::format_err!("block not found (number={})", block_number)
            }
            HindsightError::EventNotCached(tx_hash) => {
                anyhow::format_err!("event not cached (hash={})", tx_hash)
            }
            HindsightError::PoolNotFound(address) => {
                anyhow::format_err!("no other pool found, (pool={})", address)
            }
            HindsightError::TxNotLanded(tx_hash) => {
                anyhow::format_err!("tx not landed (hash={})", tx_hash)
            }
            HindsightError::CallError(msg) => anyhow::format_err!("call error: {}", msg),
            HindsightError::MathError(msg) => {
                anyhow::format_err!("math error: {}", msg,)
            }
            HindsightError::EvmParseError(msg) => {
                anyhow::format_err!("evm parse error: {}", msg,)
            }
        }
    }
}

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {
        Err(anyhow::anyhow!(format!($($arg)*)))
    };
}
