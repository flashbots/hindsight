use crate::config::Config;
use crate::data::arbs::ArbDb;
use crate::hindsight::Hindsight;
use crate::sim::processor::H256Map;
use crate::util::get_ws_client;
use crate::Result;
use ethers::providers::Middleware;
use ethers::types::H256;
use mev_share_sse::EventHistory;
use serde_json::json;

pub async fn run(batch_size: Option<usize>, config: Config, save_to_db: bool) -> Result<()> {
    let hindsight = Hindsight::new(config.to_owned()).await?;
    let juicy_event: EventHistory = serde_json::from_value(json!({
      "block": 17637019,
      "timestamp": 1688673408,
      "hint": {
        "txs": null,
        "hash": "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a",
        "logs": [
          {
            "address": "0x5db3d38bd40c862ba1fdb2286c32a62ab954d36d",
            "topics": [
              "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
              "0x0000000000000000000000000000000000000000000000000000000000000000",
              "0x0000000000000000000000000000000000000000000000000000000000000000"
            ]
          },
          {
            "address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
            "topics": [
              "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
              "0x0000000000000000000000000000000000000000000000000000000000000000",
              "0x0000000000000000000000000000000000000000000000000000000000000000"
            ]
          },
          {
            "address": "0x36bcf57291a291a6e0e0bff7b12b69b556bcd9ed",
            "topics": [
              "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
              "0x0000000000000000000000000000000000000000000000000000000000000000",
              "0x0000000000000000000000000000000000000000000000000000000000000000"
            ]
          }
        ]
      }
    }))?;
    let juicy_tx_hash: H256 =
        "0xf00df02ad86f04a8b32d9f738394ee1b7ff791647f753923c60522363132f84a".parse::<H256>()?;
    let juicy_tx = get_ws_client(None)
        .await?
        .get_transaction(juicy_tx_hash)
        .await?
        .expect("failed to find juicy tx on chain");
    let event_map = vec![juicy_event]
        .iter()
        .map(|event| (event.hint.hash, event.to_owned()))
        .collect::<H256Map<EventHistory>>();
    hindsight
        .process_orderflow(
            vec![juicy_tx].as_ref(),
            batch_size.unwrap_or(1),
            if save_to_db {
                Some(Box::new(ArbDb::new(None).await?))
            } else {
                None
            },
            event_map,
        )
        .await?;
    Ok(())
}
