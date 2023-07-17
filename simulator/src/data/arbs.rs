use crate::{
    data::DbConnect,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use futures::stream::TryStreamExt;
use mongodb::Collection;

const ARB_COLLECTION: &'static str = "arbs";

#[derive(Clone, Debug)]
pub struct ArbDb {
    connect: DbConnect,
    arb_collection: Collection<SimArbResultBatch>,
}

impl ArbDb {
    pub async fn new(name: Option<String>) -> Result<Self> {
        let connect = DbConnect::new(name).init().await?;
        let arb_collection = connect.db.collection::<SimArbResultBatch>(ARB_COLLECTION);
        Ok(Self {
            connect,
            arb_collection,
        })
    }

    pub async fn write_arbs(&self, arbs: Vec<SimArbResultBatch>) -> Result<()> {
        self.arb_collection.insert_many(arbs, None).await?;
        Ok(())
    }

    pub async fn read_arbs(
        &self,
        // limit: Option<i64>,
        // filter: Option<SimArbResultBatch>,
    ) -> Result<Vec<SimArbResultBatch>> {
        // TODO: maybe this later
        // let limit = limit.unwrap_or(100);
        // let mut options;
        // if let Some(filter) = filter {
        //     options = FindOptions::builder().filter(filter).build();
        // }
        // options = options.build();
        let collection = self
            .connect
            .db
            .collection::<SimArbResultBatch>(ARB_COLLECTION);
        let mut cursor = collection.find(None, None).await?;
        let mut results = vec![];
        while let Some(res) = cursor.try_next().await? {
            results.push(res);
        }
        Ok(results)
    }

    pub async fn get_previously_saved_ranges(&self) -> Result<StoredArbsRanges> {
        let mut all_arbs = self.read_arbs().await?;
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

    #[tokio::test]
    async fn it_writes_to_db() -> Result<()> {
        let connect = ArbDb::new(Some(TEST_DB.to_owned())).await?;
        let arbs = vec![SimArbResultBatch::test_example()];
        connect.write_arbs(arbs).await?;
        Ok(())
    }

    #[tokio::test]
    async fn it_reads_from_db() -> Result<()> {
        let connect = ArbDb::new(Some(TEST_DB.to_owned())).await?;
        let arbs = connect.read_arbs().await?;
        println!("arbs: {:?}", arbs);
        Ok(())
    }

    #[tokio::test]
    async fn it_finds_block_ranges_from_db() -> Result<()> {
        let connect = ArbDb::new(Some(TEST_DB.to_owned())).await?;
        // insert some test data first
        let arbs = vec![
            {
                let mut ex1 = SimArbResultBatch::test_example();
                ex1.event.block = 1;
                ex1.event.timestamp = 1;
                ex1
            },
            {
                let mut ex2 = SimArbResultBatch::test_example();
                ex2.event.block = 100000000000;
                ex2.event.timestamp = 184467440737095516;
                ex2
            },
        ];
        connect.write_arbs(arbs).await?;
        let ranges = connect.get_previously_saved_ranges().await?;
        println!("ranges: {:?}", ranges);
        Ok(())
    }
}
