use crate::data::arbs::{ArbDatabase, ArbFilterParams, WriteEngine};
use crate::foresight::analyze::analyze_pair_data;
use crate::Result;

pub async fn run(
    params: ArbFilterParams,
    read_db: &ArbDatabase,
    write_dest: WriteEngine,
) -> Result<()> {
    println!("analyzing arbs... {:?}", params);
    // read arbs from DB, filtered by token pair (but one is always weth)
    let arbs = read_db.read_arbs(&params, None, None).await?; // TODO: handle offset & limit

    // call foresight::analyze::analyze_arbs
    let result = analyze_pair_data(&arbs).unwrap();
    println!("result: {:?}", result);

    Ok(())
}
