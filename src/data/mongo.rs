use super::arbs::{export_arbs_core, ArbDb, ArbFilterParams, WriteEngine};
use crate::interfaces::SimArbResultBatch;
use crate::interfaces::StoredArbsRanges;
use crate::Result;
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::bson::Document;
use mongodb::options::Tls;
use mongodb::options::TlsOptions;
use mongodb::{
    bson::doc,
    options::{FindOneOptions, FindOptions},
    Collection,
};
use mongodb::{options::ClientOptions, Client as DbClient, Database};
use std::path::PathBuf;
use std::sync::Arc;

pub const DB_NAME: &str = "hindsight";
const PROJECT_NAME: &str = "simulator";
const ARB_COLLECTION: &str = "arbs";

#[derive(Debug, Clone)]
pub struct MongoConnect {
    arb_collection: Arc<Collection<SimArbResultBatch>>,
}

#[derive(Clone, Debug)]
pub struct MongoConfig {
    pub url: String,
    pub tls_ca_file_path: Option<PathBuf>,
}

impl Default for MongoConfig {
    fn default() -> Self {
        let config = crate::config::Config::default();
        Self {
            url: config.mongo_url,
            tls_ca_file_path: config.tls_ca_file_mongo,
        }
    }
}

impl From<ArbFilterParams> for Document {
    fn from(val: ArbFilterParams) -> Self {
        let block_start = val.block_start.unwrap_or(1);
        let block_end = val.block_end.unwrap_or(u32::MAX);
        let timestamp_start = val.timestamp_start.unwrap_or(1);
        let timestamp_end = val.timestamp_end.unwrap_or(u32::MAX);
        let min_profit = val.min_profit.unwrap_or(0.into());
        let max_profit = if min_profit > 0.into() {
            doc! {
                "$ne": "0x0",
            }
        } else {
            // basically a noop; matches any doc w/ this field, which is all of them
            doc! {
                "$exists": true
            }
        };

        doc! {
                "event.block": {
                    "$gte": block_start,
                    "$lte": block_end,
                },
                "event.timestamp": {
                    "$gte": timestamp_start,
                    "$lte": timestamp_end,
                },
                "maxProfit": max_profit,
        }
    }
}

/// Talks to the database.
impl MongoConnect {
    /// Creates a new ArbDb instance, which connects to the arb collection.
    pub async fn new(config: MongoConfig) -> Result<Self> {
        let db = MongoConnect::init_db(config).await?;
        let arb_collection = Arc::new(db.collection::<SimArbResultBatch>(ARB_COLLECTION));
        // TODO: use indexes
        Ok(Self { arb_collection })
    }

    /// Connects to Mongo db provided in `config`. If `config.tls_ca_file_path` is None, then TLS is disabled.
    async fn init_db(config: MongoConfig) -> Result<Arc<Database>> {
        let mut options = ClientOptions::parse(config.url).await?;
        options.app_name = Some(PROJECT_NAME.to_owned());
        options.tls = Some(config.tls_ca_file_path.map_or(Tls::Disabled, |ca_path| {
            Tls::Enabled(TlsOptions::builder().ca_file_path(ca_path).build())
        }));
        let db_name = if cfg!(test) {
            // separate test db
            "test_hindsight"
        } else {
            DB_NAME
        };
        let db = Arc::new(DbClient::with_options(options)?.database(db_name));
        Ok(db)
    }

    /// Retrieves the (first, last) arb in the DB (by timestamp).
    async fn get_arb_extrema(
        &self,
    ) -> Result<(Option<SimArbResultBatch>, Option<SimArbResultBatch>)> {
        let first = self
            .arb_collection
            .find_one(
                None,
                FindOneOptions::builder()
                    .sort(doc! { "event.timestamp": 1 })
                    .build(),
            )
            .await?;
        let last = self
            .arb_collection
            .find_one(
                None,
                FindOneOptions::builder()
                    .sort(doc! { "event.timestamp": -1 })
                    .build(),
            )
            .await?;
        Ok((first, last))
    }
}

#[async_trait]
impl ArbDb for MongoConnect {
    /// Write given arbs to the DB.
    async fn write_arbs(&self, arbs: &[SimArbResultBatch]) -> Result<()> {
        self.arb_collection.insert_many(arbs, None).await?;
        Ok(())
    }

    async fn get_num_arbs(&self, filter_params: &ArbFilterParams) -> Result<u64> {
        Ok(self
            .arb_collection
            .count_documents(Some(filter_params.to_owned().into()), None)
            .await?)
    }

    /// Load all arbs from the DB.
    async fn read_arbs(
        &self,
        filter_params: &ArbFilterParams,
        offset: Option<u64>,
        limit: Option<i64>,
    ) -> Result<Vec<SimArbResultBatch>> {
        // small optimization: match non-zero profit if min_profit is set and > 0
        let mut cursor = self
            .arb_collection
            .find(
                Some(filter_params.to_owned().into()),
                Some(FindOptions::builder().skip(offset).limit(limit).build()),
            )
            .await?;

        let mut results = vec![];
        while let Some(res) = cursor.try_next().await? {
            results.push(res);
        }
        // gotta filter profits in memory bc mongo doesn't support bigint comparisons
        let results = results
            .into_iter()
            .filter(|arb| arb.max_profit >= filter_params.min_profit.unwrap_or(0.into()))
            .collect::<Vec<_>>();
        Ok(results)
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
            (1, 1) // TODO: replace tuples w/ option pattern, this is a hack
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

    async fn export_arbs(
        &self,
        write_dest: WriteEngine,
        filter_params: &ArbFilterParams,
    ) -> Result<()> {
        let src = Arc::new(self.clone());
        export_arbs_core(src, write_dest, filter_params).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{config::Config, interfaces::SimArbResultBatch, Result};

    async fn inject_test_arbs(
        connect: &MongoConnect,
        quantity: u64,
    ) -> Result<Vec<SimArbResultBatch>> {
        let mut arbs = vec![];
        (0..quantity).for_each(|i| {
            let mut arb = SimArbResultBatch::test_example();
            arb.event.block = 1 + i;
            arb.event.timestamp = 0x6464beef + (i * 12);
            arbs.push(arb);
        });
        connect.write_arbs(&arbs).await?;
        Ok(arbs)
    }

    async fn connect() -> Result<MongoConnect> {
        let config = Config::default();
        let connect = MongoConnect::new(MongoConfig {
            url: config.mongo_url,
            tls_ca_file_path: config.tls_ca_file_mongo,
        })
        .await?;
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
        let arbs = connect
            .read_arbs(&ArbFilterParams::default(), None, None)
            .await?;
        println!("arbs: {:?}", arbs);
        Ok(())
    }

    // assert read_arbs filters arbs by min_block_number
    #[tokio::test]
    async fn it_filters_arbs() -> Result<()> {
        let connect = connect().await?;
        inject_test_arbs(&connect, 10).await?;
        let (arb_first, arb_last) = connect.get_arb_extrema().await?;
        let (arb_first, arb_last) = (arb_first.unwrap(), arb_last.unwrap());
        let (block_first, _) = (arb_first.event.block, arb_last.event.block);
        let (timestamp_first, _) = (arb_first.event.timestamp, arb_last.event.timestamp);

        // filter by block number
        let arbs = connect
            .read_arbs(
                &ArbFilterParams {
                    block_start: Some(block_first as u32),
                    block_end: Some(block_first as u32 + 5),
                    timestamp_start: None,
                    timestamp_end: None,
                    min_profit: Some(1.into()),
                },
                Some(1),
                Some(3),
            )
            .await?;
        assert!(!arbs.is_empty());
        assert!(arbs.len() <= 5);
        assert!(arbs.iter().all(|arb| arb.event.block >= block_first));

        // filter by timestamp
        let arbs = connect
            .read_arbs(
                &ArbFilterParams {
                    block_start: None,
                    block_end: None,
                    timestamp_start: Some(timestamp_first as u32),
                    timestamp_end: Some(timestamp_first as u32 + 5),
                    min_profit: Some(1.into()),
                },
                None,
                Some(5),
            )
            .await?;
        assert!(!arbs.is_empty());
        assert!(arbs.len() <= 5);
        assert!(arbs
            .iter()
            .all(|arb| arb.event.timestamp >= timestamp_first));

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
                WriteEngine::File(Some("test_arbs.json".to_owned())),
                &ArbFilterParams::default(),
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
