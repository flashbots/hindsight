use futures::future::join_all;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::db::DbEngine;
use crate::{
    data::{db::Db, file::save_arbs_to_file},
    debug, info,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use deadqueue::unlimited::Queue;
use ethers::{types::U256, utils::format_ether};

const NUM_ARBS_PER_READ: i64 = 3000;

#[derive(Clone, Debug)]
pub struct ArbFilterParams {
    pub block_start: Option<u64>,
    pub block_end: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub min_profit: Option<U256>,
}

impl Default for ArbFilterParams {
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
pub trait ArbDb: Sync + Send {
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
pub async fn export_arbs_core(
    src: Arc<dyn ArbDb>,
    write_dest: WriteEngine,
    filter_params: &ArbFilterParams,
) -> Result<()> {
    // determine total number of arbs now to prevent running forever
    let total_arbs = src.get_num_arbs(filter_params).await?;
    debug!("total arbs: {}", total_arbs);
    let offset_lock = Arc::new(Mutex::new(0));

    // thread-safe queue
    let arb_queue_handle: Arc<Queue<SimArbResultBatch>> = Arc::new(Queue::new());
    // thread-safe mutex to keep writer thread from quitting before we're done reading
    let process_done = Arc::new(Mutex::new(()));

    // arc clones to give to the reader thread
    let arb_queue = arb_queue_handle.clone();
    let filter_params = filter_params.clone();
    let lock = process_done.clone();

    // spawn reader thread
    let read_handle = tokio::spawn(async move {
        info!("starting reader thread...");
        // lock process_done to keep writer thread from quitting before we're done reading
        let _process_lock = lock.lock().await;
        // read NUM_ARBS_PER_READ arbs at a time
        let mut offset = offset_lock.lock().await;
        while *offset < total_arbs {
            let arbs = src
                .read_arbs(&filter_params, Some(*offset), Some(NUM_ARBS_PER_READ))
                .await
                .expect("failed to read arbs");
            if arbs.len() == 0 {
                break;
            }
            *offset = *offset + NUM_ARBS_PER_READ as u64;
            println!("offset {}", offset);
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

            for arb in arbs {
                println!("im arb: {:?}", arb.event.hint.hash);
                arb_queue.push(arb);
                println!("arb q: len {}", arb_queue.len());
            }
            // arb_lock is dropped here, unlocking the arb_queue mutex
        }
        // _process_lock is dropped here, unlocking the process_done mutex
    });

    // arc clone to give to the writer thread
    let arb_queue = arb_queue_handle.clone();
    // start writer thread
    let write_handle = tokio::spawn(async move {
        info!("starting writer thread...");
        loop {
            println!("[w] arb q {}", arb_queue.len());
            let mut batch_arbs = vec![];
            for _ in 0..arb_queue.len() {
                let arb = arb_queue.pop().await;
                batch_arbs.push(arb);
            }

            info!("finna write {} arbs", batch_arbs.len());
            if batch_arbs.len() > 0 {
                match write_dest.clone() {
                    WriteEngine::File(filename) => {
                        save_arbs_to_file(filename, batch_arbs)
                            .await
                            .expect("failed to write arbs to file");
                    }
                    WriteEngine::Db(db_engine) => {
                        let db = Db::new(db_engine).await.connect;
                        db.write_arbs(&batch_arbs)
                            .await
                            .expect("failed to write arbs to db");
                        info!("wrote {} arbs to db", batch_arbs.len());
                    }
                }
            } else {
                info!("no arbs to write, sleeping...");
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            // if process_done is unlocked, reader thread is done
            if process_done.try_lock().is_ok() && arb_queue.len() == 0 {
                info!("reader thread done, writer thread quitting...");
                break;
            }
        }
    });

    join_all(vec![read_handle, write_handle]).await;

    Ok(())
}

pub type ArbDatabase = Arc<dyn ArbDb>;
