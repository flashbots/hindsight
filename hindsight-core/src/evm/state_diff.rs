use crate::{eth_client::WsClient, Result};
use async_trait::async_trait;
use ethers::{
    middleware::Middleware,
    providers::ProviderError,
    types::{AccountDiff, BlockId, BlockTrace, Diff, H160},
    utils::keccak256,
};
use futures::{stream::FuturesUnordered, StreamExt};
use revm::{
    db::{CacheDB, DatabaseRef, EmptyDB, EmptyDBTyped},
    primitives::{
        AccountInfo, Address as rAddress, Bytecode, Bytes as rBytes, FixedBytes, U256 as rU256,
    },
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    convert::Infallible,
};

type TreeMap = BTreeMap<H160, AccountDiff>;

pub struct StateDiff {
    pub state: TreeMap,
}

#[async_trait]
pub trait ToCacheDb<B: DatabaseRef> {
    async fn to_cache_db(
        &self,
        client: &WsClient,
        block_num: Option<BlockId>,
    ) -> Result<CacheDB<B>>;
}

#[async_trait]
impl<DbRef> ToCacheDb<DbRef> for StateDiff
where
    DbRef: DatabaseRef + Default + std::fmt::Debug,
    CacheDB<DbRef>: From<CacheDB<EmptyDBTyped<Infallible>>>,
{
    async fn to_cache_db(
        &self,
        client: &WsClient,
        block_num: Option<BlockId>,
    ) -> Result<CacheDB<DbRef>> {
        let client = client.to_owned();
        let mut cache_db = CacheDB::new(EmptyDB::default());
        let mut futures = FuturesUnordered::new();

        for (address, diff) in self.state.iter() {
            let nonce_provider = client.get_provider();
            let balance_provider = client.get_provider();
            let code_provider = client.get_provider();

            let addr = *address;

            let future = async move {
                let nonce = nonce_provider
                    .get_transaction_count(addr, block_num)
                    .await?;
                let balance = balance_provider.get_balance(addr, block_num).await?;
                let code = code_provider.get_code(addr, block_num).await?;
                let code_hash = keccak256(code.to_owned());
                let mut rbalance: [u8; 32] = [0; 32];
                balance.to_big_endian(&mut rbalance);
                Ok::<(AccountDiff, rAddress, u64, rU256, rBytes, FixedBytes<32>), ProviderError>((
                    diff.clone(),
                    addr.0.into(),
                    nonce.as_u64(),
                    rU256::from_be_slice(&rbalance),
                    code.0.into(),
                    code_hash.into(),
                ))
            };
            futures.push(future);
        }

        while let Some(result) = futures.next().await {
            let (acct_diff, address, nonce, balance, code, code_hash) = result?;
            let bytecode = Bytecode::new_raw(code);
            let acct_info = AccountInfo::new(balance, nonce, code_hash, bytecode);

            cache_db.insert_account_info(address, acct_info);

            acct_diff.storage.iter().for_each(|(slot, storage_diff)| {
                let slot_value: [u8; 32] = match storage_diff.to_owned() {
                    Diff::Changed(v) => v.from.0,
                    Diff::Died(v) => v.0,
                    _ => {
                        return;
                    }
                };
                cache_db
                    .insert_account_storage(
                        address.0.into(),
                        rU256::from_be_slice(&slot.0),
                        rU256::from_be_slice(&slot_value),
                    )
                    .expect("failed to insert account storage");
            });
        }

        Ok(cache_db.into())
    }
}

impl StateDiff {
    /// Get state diff from block number.
    /// _// TODO: abstract client away_
    ///
    /// **Note:** client must be connected to an archive node.
    pub async fn from_block_traces(block_traces: Vec<BlockTrace>) -> Result<Self> {
        let mut final_diff = BTreeMap::new();
        block_traces
            .into_iter()
            .flat_map(|trace| trace.state_diff.map(|diff| diff.0.into_iter()))
            .flatten()
            .for_each(|(address, diff)| {
                match final_diff.entry(address.into()) {
                    Entry::Vacant(entry) => {
                        entry.insert(diff.into());
                    }
                    Entry::Occupied(_) => {
                        // do nothing if key already exists
                        // thanks for the code [@mouseless](https://github.com/mouseless-eth/rusty-sando)
                    }
                }
            });

        Ok(Self { state: final_diff })
    }
}
