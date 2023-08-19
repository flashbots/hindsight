use crate::data::arbs::ArbFilterParams;
use crate::data::db::Db;
use crate::Result;

pub async fn run(filename: Option<String>, params: ArbFilterParams) -> Result<()> {
    // TODO: PARAMETERIZE ENGINE W/ COMMAND FLAGS
    let db = Db::new(crate::data::db::DbEngine::Mongo, None)
        .await
        .connect;
    db.export_arbs(filename, params).await?;
    Ok(())
}
