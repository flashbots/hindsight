use crate::data::arbs::ArbDatabase;
use crate::data::db::DbEngine;
use crate::event_history::event_history_url;
use crate::hindsight::Hindsight;
use crate::info;
use crate::sim::processor::H256Map;
use crate::util::{fetch_txs, filter_events_by_topic, WsClient};
use crate::Result;
use ethers::types::H256;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub batch_size: usize,
    pub block_start: u32,
    pub block_end: Option<u32>,
    pub timestamp_start: u32,
    pub timestamp_end: Option<u32>,
    pub db_engine: DbEngine,
}

impl From<ScanOptions> for EventHistoryParams {
    fn from(val: ScanOptions) -> Self {
        EventHistoryParams {
            block_start: Some(val.block_start.into()),
            block_end: val.block_end.map(|x| x.into()),
            timestamp_start: Some(val.timestamp_start.into()),
            timestamp_end: val.timestamp_end.map(|x| x.into()),
            limit: Some(500),
            offset: Some(0),
        }
    }
}

fn uniswap_topics() -> Vec<H256> {
    vec![
        // univ3
        // Swap(address,address,int256,int256,uint160,uint128,int24)
        H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67")
            .expect("that's some bad hash"),
        // univ2
        // Swap(address,uint256,uint256,uint256,uint256,address)
        H256::from_str("0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822")
            .expect("that's some bad hash"),
    ]
}

pub async fn run(
    params: ScanOptions,
    ws_client: &WsClient,
    mevshare: &EventClient,
    hindsight: &Hindsight,
    write_db: &ArbDatabase,
) -> Result<()> {
    info!(
        "scanning events starting at block={:?} timestamp={:?}",
        params.block_start, params.timestamp_start
    );

    let mut event_params: EventHistoryParams = params.clone().into();

    let filter_topics = uniswap_topics();
    /* ========================== event processing ====================================== */
    loop {
        // fetch events
        let events = mevshare
            .event_history(&event_history_url(), event_params.to_owned())
            .await?;
        // if the api returns 0 results, we've completely run out of events to process
        // so wait, then restart loop
        if events.is_empty() {
            // sleep 12s to allow for new events to be indexed
            std::thread::sleep(std::time::Duration::from_secs(12));
            continue;
        }

        // update params for next batch of events
        event_params.offset = Some(event_params.offset.unwrap() + events.len() as u64);

        info!(
            "fetched {} events. first event timestamp={}",
            events.len(),
            events[0].timestamp
        );
        // filter out irrelevant events
        let events = filter_events_by_topic(&events, &filter_topics);
        info!(
            "filtered for uniswap events. {} events ready to process.",
            events.len()
        );
        // map events by hash for fast lookups
        let event_map = events
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();

        let mut events_offset = 0;
        let mut txs = vec![];

        // Concurrently fetch all landed txs for each event.
        // Only request `batch_size` at a time to avoid overloading the RPC endpoint.
        while events_offset < events.len() {
            let this_batch = events
                .iter()
                .skip(events_offset)
                .take(params.batch_size)
                .map(|event| event.to_owned())
                .collect::<Vec<EventHistory>>();
            events_offset += this_batch.len();
            // get txs for relevant events
            txs.append(&mut fetch_txs(ws_client, &this_batch).await?);
        }

        /* ========================== batch-sized arb processing ========================
           Here, *at least* `batch_size` txs should be passed to `process_orderflow`.
           In `process_orderflow`, *at most* `batch_size` txs are simulated at a time.
           The last iteration will process only (remaining_txs % batch_size) txs, so it's
           most efficient when (txs.len() % batch_size == 0) and/or (txs.len() much greater than batch_size).
        */
        hindsight
            .to_owned()
            .process_orderflow(&txs, params.batch_size, Some(write_db.clone()), event_map)
            .await?;
        info!("simulated arbs for {} transactions", txs.len());
        info!("offset: {:?}", event_params.offset);

        // if the api returns < limit, we're processing the most recent events
        // so we pause to avoid the loop spamming the api
        if events.len() < event_params.limit.unwrap_or(500) as usize {
            if params.block_end.is_some() || params.timestamp_end.is_some() {
                // if we're processing a specific block range, we're done
                break;
            }
            // sleep 12s to allow for new events to be indexed
            std::thread::sleep(std::time::Duration::from_secs(12));
        }
    }
    Ok(())
}
