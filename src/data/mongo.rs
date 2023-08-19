use crate::interfaces::StoredArbsRanges;
use crate::Result;
use crate::{config::Config, info, interfaces::SimArbResultBatch};

use super::arbs::{ArbFilterParams, ArbInterface};
use async_trait::async_trait;
use ethers::{types::U256, utils::format_ether};
use futures::stream::TryStreamExt;
use mongodb::{bson::doc, options::FindOptions, Collection};
use mongodb::{options::ClientOptions, Client as DbClient, Database};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Arc;

const DB_NAME: &'static str = "hindsight";
const PROJECT_NAME: &'static str = "simulator";
const ARB_COLLECTION: &'static str = "arbs";
const EXPORT_DIR: &'static str = "./arbData";

#[derive(Debug, Clone)]
pub struct MongoConnect {
    arb_collection: Arc<Collection<SimArbResultBatch>>,
}

/// Talks to the database.
impl MongoConnect {
    /// Creates a new ArbDb instance, which connects to the arb collection.
    pub async fn new(name: Option<&str>) -> Result<Self> {
        let url = Config::default().db_url;
        let db = MongoConnect::init_db(url, name).await?;
        let arb_collection = Arc::new(db.collection::<SimArbResultBatch>(ARB_COLLECTION));
        Ok(Self { arb_collection })
    }

    async fn init_db(url: String, db_name: Option<&str>) -> Result<Arc<Database>> {
        let mut options = ClientOptions::parse(url).await?;
        options.app_name = Some(PROJECT_NAME.to_owned());
        // options.default_database = Some(DB_NAME.to_owned());
        options.credential = Some(
            mongodb::options::Credential::builder()
                .username("root".to_owned())
                .password("example".to_owned())
                .build(),
        );
        let db = Arc::new(DbClient::with_options(options)?.database(db_name.unwrap_or(DB_NAME)));
        Ok(db)
    }

    /// Retrieves the first arb in the DB (by lowest timestamp).
    async fn get_arb_extrema(
        &self,
    ) -> Result<(Option<SimArbResultBatch>, Option<SimArbResultBatch>)> {
        // find start
        let find_options = FindOptions::builder()
            .sort(doc! { "event.timestamp": 1 })
            .build();
        let mut cursor = self.arb_collection.find(None, find_options).await?;
        let arb_start = cursor.try_next().await?;
        // find end
        let find_options = FindOptions::builder()
            .sort(doc! { "event.timestamp": -1 })
            .build();
        let mut cursor = self.arb_collection.find(None, find_options).await?;
        let arb_end = cursor.try_next().await?;
        Ok((arb_start, arb_end))
    }
}

#[async_trait]
impl ArbInterface for MongoConnect {
    /// Write given arbs to the DB.
    async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()> {
        self.arb_collection.insert_many(arbs, None).await?;
        Ok(())
    }

    /// Load all arbs from the DB.
    async fn read_arbs(&self, filter_params: ArbFilterParams) -> Result<Vec<SimArbResultBatch>> {
        // TODO: add filter params to query instead of filtering post-query
        let mut cursor = self.arb_collection.find(None, None).await?;
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
    async fn export_arbs(
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
        info!("SUM PROFIT: {} Ξ", format_ether(sum_profit));
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
    ///
    /// It is assumed that the timestamps and blocks are both monotonically increasing,
    /// so only timestamps need to be checked; checking block number would be redundant and less precise.
    async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges> {
        let (arb_start, arb_end) = self.get_arb_extrema().await?;
        let (earliest_block, earliest_timestamp) = if let Some(arb) = arb_start {
            (arb.event.block, arb.event.timestamp)
        } else {
            (1, 1)
        };
        let (latest_block, latest_timestamp) = if let Some(arb) = arb_end {
            (arb.event.block, arb.event.timestamp)
        } else {
            (2, 2)
        };
        Ok(StoredArbsRanges {
            earliest_block,
            latest_block,
            earliest_timestamp,
            latest_timestamp,
        })
    }
}

// TODO: move these, generalize connect to test both dbs
#[cfg(test)]
mod test {
    use super::*;
    use crate::{interfaces::SimArbResultBatch, Result};

    const TEST_DB: &'static str = "test_hindsight";

    async fn inject_test_arbs(
        connect: &MongoConnect,
        quantity: u64,
    ) -> Result<Vec<SimArbResultBatch>> {
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

    async fn connect() -> Result<MongoConnect> {
        let connect = MongoConnect::new(Some(TEST_DB)).await?;
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
        assert!(ranges.earliest_timestamp < ranges.latest_timestamp);
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

    #[tokio::test]
    async fn it_gets_arb_extrema() -> Result<()> {
        let connect = connect().await?;
        inject_test_arbs(&connect, 13).await?;
        let arb_range = connect.get_arb_extrema().await?;
        println!("first arb: {:?}", arb_range.0);
        println!("last arb: {:?}", arb_range.1);
        assert!(arb_range.0.unwrap().event.timestamp < arb_range.1.unwrap().event.timestamp);
        Ok(())
    }
}
