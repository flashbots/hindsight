/* credit to mouseless: [rusty-sando](https://raw.githubusercontent.com/mouseless-eth/rusty-sando/8bed4dbc27e8dac5c1f38cff595bdc082f1f892b/bot/src/forked_db/fork_db.rs) */

use std::sync::mpsc::channel as oneshot_channel;

use super::global_backend::BackendFetchRequest;
use crate::Result;
use futures::channel::mpsc::Sender;
use revm::{
    db::{CacheDB, DatabaseRef, EmptyDB},
    primitives::{
        Account, AccountInfo, Address as rAddress, Bytecode as rBytecode, HashMap as rHashMap,
        B256, KECCAK_EMPTY, U256 as rU256,
    },
    Database, DatabaseCommit,
};

#[derive(Clone, Debug)]
pub struct ForkDB {
    // used to make calls for missing data
    backend: Sender<BackendFetchRequest>,
    db: CacheDB<EmptyDB>,
}

impl ForkDB {
    pub fn new(backend: Sender<BackendFetchRequest>, db: CacheDB<EmptyDB>) -> Self {
        Self { backend, db }
    }

    fn do_get_basic(&self, address: rAddress) -> Result<Option<AccountInfo>> {
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot_channel();
            let req = BackendFetchRequest::Basic(address, sender);
            self.backend.clone().try_send(req)?;
            rx.recv()?.map(Some)
        })
    }

    fn do_get_storage(&self, address: rAddress, index: rU256) -> Result<rU256> {
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot_channel();
            let req = BackendFetchRequest::Storage(address, index, sender);
            self.backend.clone().try_send(req)?;
            rx.recv()?
        })
    }

    fn do_get_block_hash(&self, number: rU256) -> Result<B256> {
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot_channel();
            let req = BackendFetchRequest::BlockHash(number, sender);
            self.backend.clone().try_send(req)?;
            rx.recv()?
        })
    }
}

impl Database for ForkDB {
    type Error = crate::Error;

    fn basic(&mut self, address: rAddress) -> Result<Option<AccountInfo>, Self::Error> {
        // found locally, return it
        match self.db.accounts.get(&address) {
            // basic info is already in db
            Some(account) => Ok(Some(account.info.clone())),
            None => {
                // basic info is not in db, make rpc call to fetch it
                let info = match self.do_get_basic(address) {
                    Ok(i) => i,
                    Err(e) => return Err(e),
                };

                // keep record of fetched acc basic info
                if info.is_some() {
                    self.db.insert_account_info(
                        address,
                        info.clone().expect("failed to insert account info"),
                    );
                }

                Ok(info)
            }
        }
    }

    fn storage(&mut self, address: rAddress, index: rU256) -> Result<rU256, Self::Error> {
        // found locally, return it
        if let Some(account) = self.db.accounts.get(&address) {
            if let Some(entry) = account.storage.get(&index) {
                // account storage exists at slot
                return Ok(*entry);
            }
        }

        // get account info
        let acc_info = match self.do_get_basic(address) {
            Ok(a) => a,
            Err(e) => return Err(e),
        };

        if let Some(a) = acc_info {
            self.db.insert_account_info(address, a);
        }

        // make rpc call to fetch storage
        let storage_val = match self.do_get_storage(address, index) {
            Ok(i) => i,
            Err(e) => return Err(e),
        };

        // keep record of fetched storage (can unwrap safely as cacheDB always returns true)
        self.db
            .insert_account_storage(address, index, storage_val)
            .expect("failed to insert account storage");

        Ok(storage_val)
    }

    fn block_hash(&mut self, number: rU256) -> Result<B256, Self::Error> {
        match self.db.block_hashes.get(&number) {
            // found locally, return it
            Some(hash) => Ok(*hash),
            None => {
                // rpc call to fetch block hash
                let block_hash = match self.do_get_block_hash(number) {
                    Ok(i) => i,
                    Err(e) => return Err(e),
                };

                // insert fetched block hash into db
                self.db.block_hashes.insert(number, block_hash);

                Ok(block_hash)
            }
        }
    }

    /// Get account code by its hash
    fn code_by_hash(&mut self, code_hash: B256) -> Result<rBytecode, Self::Error> {
        match self.db.code_by_hash(code_hash) {
            Ok(code) => Ok(code),
            Err(e) => {
                // should alr be loaded
                Err(Self::Error::new(e))
            }
        }
    }
}

impl DatabaseRef for ForkDB {
    type Error = crate::Error;

    fn basic(&self, address: rAddress) -> Result<Option<AccountInfo>, Self::Error> {
        match self.db.accounts.get(&address) {
            Some(account) => Ok(Some(account.info.clone())),
            None => {
                // state doesnt exist so fetch it
                self.do_get_basic(address)
            }
        }
    }

    fn storage(&self, address: rAddress, index: rU256) -> Result<rU256, Self::Error> {
        match self.db.accounts.get(&address) {
            Some(account) => match account.storage.get(&index) {
                Some(entry) => Ok(*entry),
                None => {
                    // state doesnt exist so fetch it
                    match self.do_get_storage(address, index) {
                        Ok(storage) => Ok(storage),
                        Err(e) => Err(e),
                    }
                }
            },
            None => {
                // state doesnt exist so fetch it
                match self.do_get_storage(address, index) {
                    Ok(storage) => Ok(storage),
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn block_hash(&self, number: rU256) -> Result<B256, Self::Error> {
        if number > rU256::from(u64::MAX) {
            return Ok(KECCAK_EMPTY);
        }
        self.do_get_block_hash(number)
    }

    /// Get account code by its hash
    fn code_by_hash(&self, code_hash: B256) -> Result<revm::primitives::Bytecode, Self::Error> {
        match self.db.code_by_hash(code_hash) {
            Ok(code) => Ok(code),
            Err(e) => {
                // should alr be loaded
                Err(Self::Error::msg(format!(
                    "MissingCode (code_hash={}): {}",
                    code_hash, e
                )))
            }
        }
    }
}

impl DatabaseCommit for ForkDB {
    fn commit(&mut self, changes: rHashMap<rAddress, Account>) {
        self.db.commit(changes)
    }
}
