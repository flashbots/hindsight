use futures::future::join_all;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    data::{db::Db, file::save_arbs_to_file},
    debug, info,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use ethers::{types::U256, utils::format_ether};
use queues::{queue, IsQueue, Queue};

use super::db::DbEngine;

const NUM_ARBS_PER_READ: i64 = 1000;

#[derive(Clone, Debug)]
pub struct ArbFilterParams {
    pub block_start: Option<u64>,
    pub block_end: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub min_profit: Option<U256>,
}

impl Default for ArbFilterParams {
    /// syntactical sugar for ArbFilterParams::none()
    fn default() -> Self {
        Self::none()
    }
}

impl ArbFilterParams {
    pub fn none() -> Self {
        Self {
            block_start: None,
            block_end: None,
            timestamp_start: None,
            timestamp_end: None,
            min_profit: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum WriteEngine {
    File(Option<String>),
    Db(DbEngine),
}

#[async_trait]
pub trait ArbInterface: Sync + Send {
    async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()>;
    async fn read_arbs(
        &self,
        filter_params: &ArbFilterParams,
        offset: Option<u64>,
        limit: Option<i64>,
    ) -> Result<Vec<SimArbResultBatch>>;
    async fn get_num_arbs(&self, filter_params: &ArbFilterParams) -> Result<u64>;
    async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges>;
    async fn export_arbs(
        &self,
        write_dest: WriteEngine,
        filter_params: &ArbFilterParams,
    ) -> Result<()>;
}

/// Saves arbs to given write engine (file or db).
pub async fn export_arbs_core<'k>(
    src: Arc<dyn ArbInterface>,
    write_dest: WriteEngine,
    filter_params: &ArbFilterParams,
) -> Result<()> {
    // determine total number of arbs now to prevent running forever
    let total_arbs = src.get_num_arbs(filter_params).await?;
    debug!("total arbs: {}", total_arbs);
    let mut offset = 0;

    // thread-safe FIFO queue
    let arb_queue_handle: Arc<Mutex<Queue<SimArbResultBatch>>> = Arc::new(Mutex::new(queue![]));
    // thread-safe mutex to keep writer thread from quitting before we're done reading
    let process_done = Arc::new(Mutex::new(()));

    // arc clones to give to the reader thread
    let arb_queue = arb_queue_handle.clone();
    let filter_params = filter_params.clone();
    let lock = process_done.clone();

    // spawn reader thread
    let read_handle = tokio::spawn(async move {
        debug!("starting reader thread...");
        // lock process_done to keep writer thread from quitting before we're done reading
        let _process_lock = lock.lock().await;
        // read NUM_ARBS_PER_READ arbs at a time
        while offset < total_arbs {
            let arbs = src
                .read_arbs(&filter_params, Some(offset), Some(NUM_ARBS_PER_READ))
                .await
                .unwrap_or(vec![]);
            offset += arbs.len() as u64;
            if arbs.len() == 0 {
                break;
            }
            let start_block = arbs.iter().map(|arb| arb.event.block).min().unwrap_or(0);
            let end_block = arbs
                .iter()
                .map(|arb| arb.event.block)
                .max()
                .unwrap_or(u64::MAX);
            let start_timestamp = arbs
                .iter()
                .map(|arb| arb.event.timestamp)
                .min()
                .unwrap_or(0);
            let end_timestamp = arbs
                .iter()
                .map(|arb| arb.event.timestamp)
                .max()
                .unwrap_or(u64::MAX);
            let sum_profit = arbs
                .iter()
                .fold(0.into(), |acc: U256, arb| acc + arb.max_profit);
            info!("SUM PROFIT: {} Ξ", format_ether(sum_profit));
            info!("(start,end) block: ({}, {})", start_block, end_block);
            info!(
                "time range: {} days",
                (end_timestamp - start_timestamp) as f64 / 86400_f64
            );

            let mut arb_lock = arb_queue.lock().await;
            for arb in arbs {
                arb_lock.add(arb).unwrap();
            }
            // arb_lock is dropped here, unlocking the arb_queue mutex
        }
        // _process_lock is dropped here, unlocking the process_done mutex
    });

    // arc clone to give to the writer thread
    let arb_queue = arb_queue_handle.clone();
    // start writer thread
    let write_handle = tokio::spawn(async move {
        debug!("starting writer thread...");
        loop {
            // if process_done is unlocked, reader thread is done
            if process_done.try_lock().is_ok() {
                debug!("reader thread done, writer thread quitting...");
                break;
            }
            let mut arb_lock = arb_queue.lock().await;
            let mut batch_arbs = vec![];
            for _ in 0..arb_lock.size() {
                let arb = arb_lock.remove().ok();
                if let Some(arb) = arb {
                    batch_arbs.push(arb);
                }
            }
            drop(arb_lock);
            if batch_arbs.len() > 0 {
                match write_dest.clone() {
                    WriteEngine::File(filename) => {
                        save_arbs_to_file(filename, batch_arbs).unwrap();
                    }
                    WriteEngine::Db(db_engine) => {
                        let db = Db::new(db_engine).await.connect;
                        db.write_arbs(&batch_arbs).await.unwrap();
                    }
                }
            }
        }
    });

    join_all(vec![read_handle, write_handle]).await;

    Ok(())
}

pub type ArbDatabase = Arc<dyn ArbInterface>;
