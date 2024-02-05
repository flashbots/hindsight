use chrono::DateTime;
use csv::ReaderBuilder;
use data::arbs::ArbDatabase;
use ethers::types::H256;
use hindsight_core::eth_client::WsClient;
use hindsight_core::mev_share_sse::{EventClient, EventHistory, EventHistoryParams};
use hindsight_core::util::H256Map;
use hindsight_core::{info, Result};
use scanner::event_history::event_history_url;
use scanner::process_orderflow::ArbClient;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TxEventRaw {
    pub tx_hash: H256,
    pub profit_eth: f64,
    pub event_block: u32,
    pub event_timestamp: String,
}

#[derive(Debug)]
pub struct TxEvent {
    pub tx_hash: H256,
    pub profit_eth: f64,
    pub event_block: u32,
    pub event_timestamp: i64,
}

pub async fn run(
    tx_events: &[TxEvent],
    ws_client: &WsClient,
    mev_share: &EventClient,
    write_db: &ArbDatabase,
) -> Result<()> {
    info!("rescanning {} events", tx_events.len());

    // get event history for each event_block asynchronously
    let mut handles = Vec::new();
    let mut matched_events = Vec::new();
    for event in tx_events {
        let event_block = event.event_block;
        let mev_share = mev_share.clone();
        let event_hash = event.tx_hash;

        let handle = tokio::spawn(async move {
            let event_history = mev_share
                .event_history(
                    &event_history_url(),
                    EventHistoryParams {
                        block_start: Some(event_block.into()),
                        block_end: Some(event_block.into()),
                        timestamp_start: None,
                        timestamp_end: None,
                        limit: Some(250),
                        offset: Some(0),
                    },
                )
                .await?;
            let matching_event = event_history
                .iter()
                .find(|event_match| event_hash.eq(&event_match.hint.hash));
            Result::<Option<EventHistory>>::Ok(matching_event.cloned())
        });

        handles.push(handle);
    }

    for handle in handles {
        let event_match = handle.await?;
        if let Some(event) = event_match? {
            matched_events.push(event);
        }
    }

    info!("matched {} events", matched_events.len());

    // get all txs for the matched events
    let txs = scanner::util::fetch_txs(&ws_client, &matched_events).await?;
    let event_map = matched_events
        .iter()
        .fold(H256Map::new(), |mut map, event| {
            map.insert(event.hint.hash, event.clone());
            map
        });

    // run the arb simulation, save results to DB
    ws_client
        .simulate_arbs(&txs, 32, Some(write_db.clone()), event_map)
        .await?;

    info!("rescan complete! Check your DB for results.");
    Ok(())
}

/// Parse a CSV file into a vector of `TxEvent`s.
///
/// **Row format:**
/// ```csv
/// tx_hash,profit_eth,event_block,event_timestamp
/// ```
/// *Example row:*
/// `0x2b38211e0109bdf3b718f6cc1783fdd47c9e6b13858b0cfb9f528c6130c88ea4,0.003582784174380261,18042665,2023-09-01 15:42:22`
///
/// *Note: CSV file must not have headers.*
pub async fn parse_csv(file_path: &str) -> Result<Vec<TxEvent>> {
    let real_path = std::fs::canonicalize(file_path)?;
    info!("parsing CSV file: {}", real_path.display());

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .from_path(real_path)?;

    let mut tx_events = Vec::new();

    for result in reader.deserialize() {
        let record: TxEventRaw = result?;
        tx_events.push(record);
    }

    Ok(tx_events
        .into_iter()
        .map(|event| {
            // append a timestamp to the event at UTC time if no timestamp is present
            let event_timestamp = if event.event_timestamp.contains("+") {
                event.event_timestamp.to_owned()
            } else {
                format!("{} +0000", &event.event_timestamp)
            };
            // parse the timestamp into a unix timestamp
            let event_timestamp =
                DateTime::parse_from_str(&event_timestamp, "%Y-%m-%d %H:%M:%S %z")
                    .expect(&format!(
                        "rescan: failed to parse date string \"{}\"",
                        event.event_timestamp
                    ))
                    .timestamp();
            TxEvent {
                tx_hash: event.tx_hash,
                profit_eth: event.profit_eth,
                event_block: event.event_block,
                event_timestamp: event_timestamp,
            }
        })
        .collect())
}
