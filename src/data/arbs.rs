use std::sync::Arc;

use crate::{
    data::{db::Db, file::save_arbs_to_file},
    info,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use ethers::{types::U256, utils::format_ether};

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
pub async fn export_arbs_core(
    src: &dyn ArbInterface,
    write_dest: WriteEngine,
    filter_params: &ArbFilterParams,
) -> Result<()> {
    // determine total number of arbs now to prevent running forever
    // ...
    let total_arbs = src.get_num_arbs(filter_params).await?;
    println!("total arbs: {}", total_arbs);
    let mut offset = 0;

    let arb_queue: Arc<tokio::sync::Mutex<Vec<SimArbResultBatch>>> =
        Arc::new(tokio::sync::Mutex::new(vec![]));

    // read NUM_ARBS_PER_READ arbs at a time
    while offset < total_arbs {
        let arbs = src
            .read_arbs(filter_params, Some(offset), Some(NUM_ARBS_PER_READ))
            .await?;
        offset += arbs.len() as u64;

        // do the following for every batch until we're out of arbs in the source db
        let start_block = arbs.iter().map(|arb| arb.event.block).min().unwrap_or(0);
        let end_block = arbs.iter().map(|arb| arb.event.block).max().unwrap_or(0);
        let start_timestamp = arbs
            .iter()
            .map(|arb| arb.event.timestamp)
            .min()
            .unwrap_or(0);
        let end_timestamp = arbs
            .iter()
            .map(|arb| arb.event.timestamp)
            .max()
            .unwrap_or(0);
        let sum_profit = arbs
            .iter()
            .fold(0.into(), |acc: U256, arb| acc + arb.max_profit);
        info!("SUM PROFIT: {} Ξ", format_ether(sum_profit));
        info!("(start,end) block: ({}, {})", start_block, end_block);
        info!(
            "time range: {} days",
            (end_timestamp - start_timestamp) as f64 / 86400_f64
        );
        let write_dest = write_dest.clone();
        let db_handle = tokio::spawn(async move {
            match write_dest {
                WriteEngine::File(filename) => {
                    println!("saving arbs to file...");
                    save_arbs_to_file(filename, arbs.to_vec())
                        .expect("failed to save arbs to file");
                }
                WriteEngine::Db(engine) => {
                    let db = Db::new(engine).await;
                    db.connect
                        .write_arbs(&arbs)
                        .await
                        .expect("failed to write arbs to db");
                }
            }
        });
        db_handle.await?;
        // tokio::join!(db_handle);
    }

    Ok(())
}

pub type ArbDatabase = Arc<dyn ArbInterface>;
