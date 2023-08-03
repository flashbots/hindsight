use crate::data::arbs::ArbDb;
use crate::data::arbs::ArbFilterParams;
use crate::Result;

pub async fn run(filename: Option<String>, params: ArbFilterParams) -> Result<()> {
    let db = ArbDb::new(None).await?;
    db.export_arbs(filename, params).await?;
    Ok(())
}
