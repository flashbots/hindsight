use std::sync::Arc;

use crate::{
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use async_trait::async_trait;
use ethers::types::U256;

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

#[async_trait]
pub trait ArbInterface {
    async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()>;
    async fn read_arbs(&self, filter_params: ArbFilterParams) -> Result<Vec<SimArbResultBatch>>;
    async fn export_arbs(
        &self,
        filename: Option<String>,
        filter_params: ArbFilterParams,
    ) -> Result<()>;
    async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges>;
}

pub type ArbDatabase = Arc<dyn ArbInterface>;
