use super::arbs::{export_arbs_core, ArbFilterParams, ArbInterface, WriteEngine};
use crate::interfaces::SimArbResultBatch;
use crate::interfaces::StoredArbsRanges;
use crate::Result;
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::bson::Document;
use mongodb::options::Tls;
use mongodb::options::TlsOptions;
use mongodb::{bson::doc, options::FindOptions, Collection};
use mongodb::{options::ClientOptions, Client as DbClient, Database};
use std::path::PathBuf;
use std::sync::Arc;

pub const DB_NAME: &'static str = "hindsight";
const PROJECT_NAME: &'static str = "simulator";
const ARB_COLLECTION: &'static str = "arbs";

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

impl Into<Document> for ArbFilterParams {
    fn into(self) -> Document {
        let block_start = self.block_start.unwrap_or(1);
        let block_end = self.block_end.unwrap_or(f64::MAX as u64);
        let timestamp_start = self.timestamp_start.unwrap_or(1);
        let timestamp_end = self.timestamp_end.unwrap_or(f64::MAX as u64);
        let min_profit = self.min_profit.unwrap_or(0.into());
        let max_profit = if min_profit > 0.into() {
            doc! {
                "$ne": "0x0",
            }
        } else {
            //noop
            doc! {
                "$exists": true
            }
        };

        doc! {
                "event.block": {
                    "$gte": block_start as f64,
                    "$lte": block_end as f64,
                },
                "event.timestamp": {
                    "$gte": timestamp_start as f64,
                    "$lte": timestamp_end as f64,
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
        // create index if dne
        // let index_names = vec!["event.block", "event.timestamp", "max_profit"];
        // let names = arb_collection.list_index_names_with_session().await?;
        // for name in index_names {
        // if !names.contains(&name.to_owned()) {
        // let index = doc! { "event.block": 1, "event.timestamp": 1, "max_profit": -1 };
        // let options = None;
        // arb_collection
        //     .create_index(IndexModel::builder().keys(index).build(), options)
        //     .await?;
        // break;
        // }
        // }
        // self.arb_collection.create_index(index, options)
        Ok(Self { arb_collection })
    }

    /// if tls_ca_file_path is None, then TLS is disabled
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

    async fn get_num_arbs(&self, filter_params: &ArbFilterParams) -> Result<u64> {
        let filter: Document = filter_params.to_owned().into();
        Ok(self
            .arb_collection
            .count_documents(Some(filter), None)
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
                Some(
                    FindOptions::builder()
                        .sort(doc! { "event.timestamp": 1 })
                        .skip(offset)
                        .limit(limit)
                        .build(),
                ),
            )
            .await?;

        let mut results = vec![];
        while let Some(res) = cursor.try_next().await? {
            results.push(res);
        }
        // gotta filter in memory bc mongo doesn't support bigint comparisons
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
        // TODO: find a more idiomatic way of implementing this for every ArbInterface impl
        export_arbs_core(self, write_dest, filter_params).await?;
        Ok(())
    }
}

// TODO: move these, generalize connect to test both dbs
#[cfg(test)]
mod test {
    use super::*;
    use crate::{config::Config, interfaces::SimArbResultBatch, Result};

    async fn inject_test_arbs(
        connect: &dyn ArbInterface,
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
        let block_first = connect.get_arb_extrema().await?.0.unwrap().event.block;
        let arbs = connect
            .read_arbs(
                &ArbFilterParams {
                    block_start: Some(block_first + 5),
                    block_end: Some(block_first + 9),
                    timestamp_start: Some(0x6464beef),
                    timestamp_end: Some(0x6464deaf),
                    min_profit: Some(1.into()),
                },
                Some(1),
                Some(3),
            )
            .await?;
        println!(
            "arbs: {:?}",
            arbs.iter().map(|arb| arb.event.block).collect::<Vec<_>>()
        );
        assert!(arbs.len() > 0);
        assert!(arbs.len() <= 3);
        assert!(arbs.iter().all(|arb| arb.event.block >= block_first + 5));

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
