use crate::data::arbs::{ArbDatabase, ArbFilterParams, WriteEngine};
use crate::foresight::analyze::analyze_arbs;
use crate::Result;

pub async fn run(
    params: ArbFilterParams,
    read_db: &ArbDatabase,
    write_dest: WriteEngine,
) -> Result<()> {
    println!("analyzing arbs... {:?}", params);
    // read arbs from DB, filtered by token pair (but one is always weth)
    let arbs = read_db.read_arbs(&params, None, None).await?; // TODO: handle offset & limit

    let result = analyze_arbs(&arbs).unwrap();
    println!("result: {:?}", result);

    Ok(())
}
