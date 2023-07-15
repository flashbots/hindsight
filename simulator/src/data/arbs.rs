use crate::{data::DbConnect, interfaces::SimArbResultBatch, Result};
use futures::stream::TryStreamExt;

const ARB_COLLECTION: &'static str = "arbs";

#[derive(Clone, Debug)]
pub struct ArbDb {
    connect: DbConnect,
}

impl ArbDb {
    pub async fn new(name: Option<String>) -> Result<Self> {
        let connect = DbConnect::new(name).init().await?;
        Ok(Self { connect })
    }

    pub async fn write_arbs(self, arbs: Vec<SimArbResultBatch>) -> Result<()> {
        let collection = self
            .connect
            .db
            .collection::<SimArbResultBatch>(ARB_COLLECTION);
        collection.insert_many(arbs, None).await?;
        Ok(())
    }

    pub async fn read_arbs(
        self,
        // limit: Option<i64>,
        // filter: Option<SimArbResultBatch>,
    ) -> Result<Vec<SimArbResultBatch>> {
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
}

// TODO: pub async fn get_previously_saved_range() ->

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
}
