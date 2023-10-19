use crate::data::arbs::{ArbDatabase, ArbFilterParams, WriteEngine};
use crate::Result;

pub async fn run(
    params: ArbFilterParams,
    read_db: &ArbDatabase,
    write_dest: WriteEngine,
) -> Result<()> {
    println!("analyzing arbs... {:?}", params);
    // read arbs from DB, filtered by pair address
    // TODO: add pair filter logic to each db engine
    // call foresight::analyze::analyze_arbs
    Ok(())
}
