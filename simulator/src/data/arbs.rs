use crate::{
    data::DbConnect,
    info,
    interfaces::{SimArbResultBatch, StoredArbsRanges},
    Result,
};
use futures::stream::TryStreamExt;
use mongodb::Collection;
use std::fs::File;
use std::io::{BufWriter, Write};

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

    pub async fn write_arbs(&self, arbs: &Vec<SimArbResultBatch>) -> Result<()> {
        self.arb_collection.insert_many(arbs, None).await?;
        Ok(())
    }

    pub async fn read_arbs(&self) -> Result<Vec<SimArbResultBatch>> {
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

    /// Saves arbs to given filename.
    pub async fn export_arbs(&self, filename: Option<&str>) -> Result<()> {
        let arbs = self.read_arbs().await?;
        let filename = filename.unwrap_or("arbs.json");
        info!("exporting {} arbs to file {}...", arbs.len(), filename);
        let file = File::create(filename)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &arbs)?;
        writer.flush()?;
        Ok(())
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
        let arbs = connect.read_arbs().await?;
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
        connect.export_arbs(Some("test_arbs.json")).await?;
        Ok(())
    }
}
