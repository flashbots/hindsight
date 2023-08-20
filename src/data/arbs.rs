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

pub enum WriteEngine {
    File(Option<String>),
    Db(DbEngine),
}

#[async_trait]
pub trait ArbInterface: Sync + Send {
    async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()>;
    async fn read_arbs(&self, filter_params: ArbFilterParams) -> Result<Vec<SimArbResultBatch>>;
    async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges>;
    async fn export_arbs(
        &self,
        write_dest: WriteEngine,
        filter_params: ArbFilterParams,
    ) -> Result<()>;
}

/// Saves arbs in JSON format to given filename. `.json` is appended to the filename if the filename doesn't have it already.
///
/// Save all files in `./arbData/`
pub async fn export_arbs_core(
    src: &dyn ArbInterface,
    // arbs: &Vec<SimArbResultBatch>,
    write_dest: WriteEngine,
    filter_params: ArbFilterParams,
) -> Result<()> {
    let arbs = src.read_arbs(filter_params).await?;
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

    match write_dest {
        WriteEngine::File(filename) => {
            save_arbs_to_file(filename, arbs.to_vec())?;
        }
        WriteEngine::Db(engine) => {
            let db = Db::new(engine, None).await;
            db.connect.write_arbs(&arbs).await?;
        }
    }

    Ok(())
}

pub type ArbDatabase = Arc<dyn ArbInterface>;
