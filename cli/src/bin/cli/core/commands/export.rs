use data::arbs::{ArbDatabase, ArbFilterParams, WriteEngine};
use hindsight_core::Result;

pub async fn run(
    params: ArbFilterParams,
    read_db: &ArbDatabase,
    write_dest: WriteEngine,
) -> Result<()> {
    println!("exporting arbs... {:?}", params);
    read_db.export_arbs(write_dest, &params).await?;
    Ok(())
}
