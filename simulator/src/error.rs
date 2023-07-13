use anyhow::Error as AnyhowError;
use ethers::types::Address;

type Error = AnyhowError;

pub enum HindsightError {
    /// The specified block number is not in the cache.
    BlockNotCached {
        block_number: u64,
    },
    PoolNotFound(Address),
}

impl Into<anyhow::Error> for HindsightError {
    fn into(self) -> Error {
        match self {
            HindsightError::BlockNotCached { block_number } => {
                anyhow::format_err!("block {} not cached", block_number)
            }
            HindsightError::PoolNotFound(address) => {
                anyhow::format_err!("no other pool found, given pool {}", address)
            }
        }
    }
}
