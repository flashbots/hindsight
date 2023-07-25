use crate::{
    data::DbConnect,
    info,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use ethers::{types::U256, utils::format_ether};
use futures::stream::TryStreamExt;
use mongodb::Collection;
use std::fs::File;
use std::io::{BufWriter, Write};

const ARB_COLLECTION: &'static str = "arbs";
const EXPORT_DIR: &'static str = "./arbData";

/// Arbitrage DB. Used for saving/loading results of simulations for long-term data analysis.
#[derive(Clone, Debug)]
pub struct ArbDb {
    connect: DbConnect,
    arb_collection: Collection<SimArbResultBatch>,
}

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
        Self {
            block_start: None,
            block_end: None,
            timestamp_start: None,
            timestamp_end: None,
            min_profit: None,
        }
    }
}

impl ArbDb {
    /// Creates a new ArbDb instance, which connects to the arb collection.
    pub async fn new(name: Option<String>) -> Result<Self> {
        let connect = DbConnect::new(name).init().await?;
        let arb_collection = connect.db.collection::<SimArbResultBatch>(ARB_COLLECTION);
        Ok(Self {
            connect,
            arb_collection,
        })
    }

    /// Write given arbs to the DB.
    pub async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()> {
        self.arb_collection.insert_many(arbs, None).await?;
        Ok(())
    }

    /// Load all arbs from the DB.
    pub async fn read_arbs(
        &self,
        filter_params: ArbFilterParams,
    ) -> Result<Vec<SimArbResultBatch>> {
        let collection = self
            .connect
            .db
            .collection::<SimArbResultBatch>(ARB_COLLECTION);
        // TODO: add filter params to query instead of filtering post-query
        let mut cursor = collection.find(None, None).await?;
        let mut results = vec![];
        while let Some(res) = cursor.try_next().await? {
            results.push(res);
        }
        let min_block = filter_params.block_start.unwrap_or(1);
        let max_block = filter_params.block_end.unwrap_or(u64::MAX);
        let min_timestamp = filter_params.timestamp_start.unwrap_or(1);
        let max_timestamp = filter_params.timestamp_end.unwrap_or(u64::MAX);
        let min_profit = filter_params.min_profit.unwrap_or(0.into());
        let results = results
            .into_iter()
            .filter(|arb| {
                arb.max_profit >= min_profit
                    && arb.event.block >= min_block
                    && arb.event.block <= max_block
                    && arb.event.timestamp >= min_timestamp
                    && arb.event.timestamp <= max_timestamp
            })
            .collect::<Vec<_>>();
        Ok(results)
    }

    /// Saves arbs in JSON format to given filename. `.json` is appended to the filename if the filename doesn't have it already.
    ///
    /// Save all files in `./arbData/`
    pub async fn export_arbs(
        &self,
        filename: Option<String>,
        filter_params: ArbFilterParams,
    ) -> Result<()> {
        let arbs = self.read_arbs(filter_params).await?;
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
        info!("SUM PROFIT: {}", format_ether(sum_profit));
        info!("(start,end) block: ({}, {})", start_block, end_block);
        info!(
            "time range: {} days",
            (end_timestamp - start_timestamp) as f64 / 86400_f64
        );
        let filename = filename.unwrap_or(format!(
            "arbs_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        ));
        let filename = if filename.ends_with(".json") {
            filename.to_owned()
        } else {
            format!("{}.json", filename)
        };
        // create ./arbData/ if it doesn't exist
        std::fs::create_dir_all(EXPORT_DIR)?;
        let filename = format!("{}/{}", EXPORT_DIR, filename);
        if arbs.len() > 0 {
            info!("exporting {} arbs to file {}...", arbs.len(), filename);
            let file = File::create(filename)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &arbs)?;
            writer.flush()?;
        } else {
            info!("no arbs found to export.");
        }
        Ok(())
    }

    /// Gets the extrema of the blocks and timestamps of the arbs in the DB.
    pub async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges> {
        let mut all_arbs = self.read_arbs(ArbFilterParams::default()).await?;
        // sort arbs by event block number
        all_arbs.sort_by(|a, b| a.event.block.cmp(&b.event.block));
        let earliest_block = all_arbs.first().map(|arb| arb.event.block).unwrap_or(0);
        let latest_block = all_arbs.last().map(|arb| arb.event.block).unwrap_or(0);
        // now sort arbs by event timestamp
        all_arbs.sort_by(|a, b| a.event.timestamp.cmp(&b.event.timestamp));
        let earliest_timestamp = all_arbs.first().map(|arb| arb.event.timestamp).unwrap_or(0);
        let latest_timestamp = all_arbs.last().map(|arb| arb.event.timestamp).unwrap_or(0);
        Ok(StoredArbsRanges {
            earliest_block,
            latest_block,
            earliest_timestamp,
            latest_timestamp,
        })
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{interfaces::SimArbResultBatch, Result};

    const TEST_DB: &'static str = "test_hindsight";

    async fn inject_test_arbs(connect: &ArbDb, quantity: u64) -> Result<Vec<SimArbResultBatch>> {
        let mut arbs = vec![];
        (0..quantity).for_each(|i| {
            let mut arb = SimArbResultBatch::test_example();
            arb.event.block = 1 + i;
            arb.event.timestamp = 0x77777777 + i;
            arbs.push(arb);
        });
        connect.write_arbs(&arbs).await?;
        Ok(arbs)
    }

    async fn connect() -> Result<ArbDb> {
        let connect = ArbDb::new(Some(TEST_DB.to_owned())).await?;
        Ok(connect)
    }

    #[tokio::test]
    async fn it_writes_to_db() -> Result<()> {
        let connect = connect().await?;
        let arbs = vec![SimArbResultBatch::test_example()];
        connect.write_arbs(&arbs).await?;
        Ok(())
    }

    #[tokio::test]
    async fn it_reads_from_db() -> Result<()> {
        let connect = connect().await?;
        let arbs = connect.read_arbs(ArbFilterParams::default()).await?;
        println!("arbs: {:?}", arbs);
        Ok(())
    }

    #[tokio::test]
    async fn it_finds_block_ranges_from_db() -> Result<()> {
        let connect = connect().await?;
        // insert some test data first
        inject_test_arbs(&connect, 2).await?;
        let ranges = connect.get_previously_saved_ranges().await?;
        println!("ranges: {:?}", ranges);
        Ok(())
    }

    #[tokio::test]
    async fn it_exports_arbs() -> Result<()> {
        // inject some test data first
        let connect = connect().await?;
        inject_test_arbs(&connect, 13).await?;
        connect
            .export_arbs(
                Some("test_arbs.json".to_owned()),
                ArbFilterParams::default(),
            )
            .await?;
        Ok(())
    }
}
