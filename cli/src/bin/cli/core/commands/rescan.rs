use std::sync::{Arc, Mutex};

use chrono::DateTime;
use csv::ReaderBuilder;
use data::arbs::ArbDatabase;
use ethers::middleware::Middleware;
use ethers::types::H256;
use hindsight_core::eth_client::WsClient;
use hindsight_core::interfaces::SimArbResultBatch;
use hindsight_core::mev_share_sse::{EventClient, EventHistory, EventHistoryParams};
use hindsight_core::util::H256Map;
use hindsight_core::{debug, info, Result};
use scanner::event_history::event_history_url;
use scanner::process_orderflow::ArbClient;
use serde::Deserialize;
use std::io::{self, Write};
use tokio::fs::OpenOptions;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;

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

const LOGFILE: &str = "rescan.log";

fn ask_to_continue() -> Result<()> {
    print!("Continue? (y/n): ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        std::process::exit(0);
    }
    Ok(())
}

/// Loads the log file and returns a vector of tx_hashes that have already
/// been re-scanned.
///
/// This is used to prevent re-scanning the same tx_hashes multiple times before
/// the entire list from the CSV file is processed.
async fn read_logfile() -> Result<Vec<H256>> {
    debug!("reading log file: {}", LOGFILE);
    let mut logreader =
        tokio::io::BufReader::new(OpenOptions::new().read(true).open(LOGFILE).await?);

    debug!("opened logfile");
    let mut logged_tx_hashes = Vec::new();
    let mut line = String::new();
    while let Ok(bytes_read) = logreader.read_line(&mut line).await {
        if bytes_read == 0 {
            break;
        }
        let tx_hash = line.trim();
        logged_tx_hashes.push(
            tx_hash
                .parse()
                .expect(&format!("failed to parse tx_hash {}", tx_hash)),
        );
        line.clear();
    }
    debug!("logged_tx_hashes: {:?}", logged_tx_hashes);
    Ok(logged_tx_hashes)
}

/// Write a new event hash to the log file.
async fn write_logfile(event_hash: H256) -> Result<()> {
    let mut logwriter = tokio::io::BufWriter::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOGFILE)
            .await?,
    );
    logwriter
        .write_all(format!("{:?}\n", event_hash).as_bytes())
        .await?;
    logwriter.flush().await?;
    Ok(())
}

/// Simulate backrun-arbs on `tx_events`, and add their
/// simulation results to the DB.
///
/// _Note: This function is called from the `rescan` command in the CLI._
///
/// **Warning**: _This function will process every given event at the same time.
/// If you have a large number of events (>50), it's very easy to overload your
/// RPC endpoint. The CLI invocation of this will batch the events into
/// manageable chunks, but if you call this directly, you may have to implement
/// your own batching logic._
pub async fn run(
    tx_events: &[TxEvent],
    ws_client: &WsClient,
    mev_share: &EventClient,
    write_db: &ArbDatabase,
) -> Result<()> {
    info!(
        "rescanning {} events: {:#?}",
        tx_events.len(),
        tx_events
            .iter()
            .map(|event| event.tx_hash)
            .collect::<Vec<H256>>()
    );

    // get event history for each event_block asynchronously
    let mut handles = Vec::new();
    let matched_events = Arc::new(Mutex::new(0u32));

    // Open or create the progress log file
    let logged_tx_hashes = read_logfile().await?;

    // define `TEST` in environment to enable regular interrupts
    // to prevent RPC from exploding during testing
    if std::env::var("TEST").is_ok() {
        ask_to_continue()?;
    }

    // before we start scanning, read '.rescan.log'
    // filter tx_events for tx_hashes that are not in the log
    let tx_events = tx_events
        .iter()
        .filter(|event| !logged_tx_hashes.contains(&event.tx_hash));

    for event in tx_events {
        info!("rescanning event: {:?}", event.tx_hash);
        let event_block = event.event_block;
        let mev_share = mev_share.clone();
        let ws_client = ws_client.clone();
        let event_hash = event.tx_hash;
        let write_db = write_db.clone();
        let matched_events = matched_events.clone();

        // spawn a new task (green thread) for each event
        let handle = tokio::task::spawn(async move {
            let event_history = mev_share
                .event_history(
                    &event_history_url(),
                    EventHistoryParams {
                        block_start: Some(event_block.into()),
                        block_end: Some(event_block.into()),
                        timestamp_start: None,
                        timestamp_end: None,
                        limit: None,
                        offset: None,
                    },
                )
                .await?;
            let matching_event = event_history
                .iter()
                .find(|event_match| event_hash.eq(&event_match.hint.hash));
            info!("retrieved event {:?}", event_hash);

            let tx = ws_client.provider.get_transaction(event_hash).await?;
            if let Some(tx) = tx {
                let event_map: H256Map<EventHistory> = matching_event
                    .map(|event| {
                        let mut map = H256Map::new();
                        map.insert(event.hint.hash, event.clone());
                        map
                    })
                    .unwrap_or_default();

                // simulate arb & write to db
                let sim_result = ws_client.backrun_tx(tx, &event_map).await?;
                write_db.write_arbs(&vec![sim_result.to_owned()]).await?;

                *matched_events.lock().unwrap() += 1;

                // when we successfully process an arb, save the event hash in the logfile
                if sim_result.results.len() > 0 {
                    write_logfile(event_hash).await?;
                } else {
                    info!("no arb found for event {:?}", event_hash);
                }
                return Result::<Option<SimArbResultBatch>>::Ok(Some(sim_result));
            }
            // if the transaction is not found, return None
            Result::<Option<SimArbResultBatch>>::Ok(None)
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await?;
        // could do something with the return value here
        // it's an Option<SimArbResultBatch>
        // idea: log failed rescans here by checking for Ok(None) from the handle
    }

    let j = *matched_events.lock().unwrap();
    info!("matched {} events", j);
    info!("rescan complete! Check your DB for results.");
    Ok(())
}

/// Parse a CSV file into a vector of `TxEvent`s.
/// These are the transactions that we want to re-scan.
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
