use crate::data::arbs::{ArbFilterParams, WriteEngine};
use crate::data::db::{Db, DbEngine};
use crate::Result;

pub async fn run(
    params: ArbFilterParams,
    read_db_engine: DbEngine,
    write_dest: WriteEngine,
) -> Result<()> {
    let db = Db::new(read_db_engine).await.connect;
    db.export_arbs(write_dest, params).await?;
    Ok(())
}
