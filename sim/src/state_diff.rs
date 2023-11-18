use std::collections::{btree_map::Entry, BTreeMap};

use ethers::{
    middleware::Middleware,
    providers::ProviderError,
    types::{AccountDiff, Block, BlockId, Diff, TraceType, Transaction, H160},
    utils::keccak256,
};
use futures::{stream::FuturesUnordered, StreamExt};
use hindsight_core::{error::HindsightError, eth_client::WsClient, Result};
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        AccountInfo, Address as rAddress, Bytecode, Bytes as rBytes, FixedBytes, U256 as rU256,
    },
};

type TreeMap = BTreeMap<H160, AccountDiff>;

pub struct StateDiff<'a> {
    pub state: TreeMap,
    pub block: Block<Transaction>,
    client: &'a WsClient,
}

impl<'a> StateDiff<'a> {
    /// Get state diff from block number.
    /// _// TODO: abstract client away_
    ///
    /// **Note:** client must be connected to an archive node.
    pub async fn from_block(client: &'a WsClient, block_num: u64) -> Result<Self> {
        // get txs from block
        let block = client
            .get_block_with_txs(block_num)
            .await?
            .ok_or(HindsightError::BlockNotFound(block_num))?;

        // get state diff from txs by calling trace_call_many on client
        let req = block
            .transactions
            .iter()
            .map(|tx| (tx, vec![TraceType::StateDiff]))
            .collect();
        let block_traces = client.trace_call_many(req, Some(block_num.into())).await?;

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

        Ok(Self {
            state: final_diff,
            client,
            block,
        })
    }

    pub async fn to_cache_db(&self, block_num: Option<BlockId>) -> Result<CacheDB<EmptyDB>> {
        let mut cache_db = CacheDB::new(EmptyDB::new());
        let mut futures = FuturesUnordered::new();

        for (address, diff) in self.state.iter() {
            let nonce_provider = self.client.clone();
            let balance_provider = self.client.clone();
            let code_provider = self.client.clone();

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

        Ok(cache_db)
    }
}
