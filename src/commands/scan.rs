use crate::config::Config;
use crate::data::db::{Db, DbEngine};
use crate::event_history::event_history_url;
use crate::hindsight::Hindsight;
use crate::info;
use crate::sim::processor::H256Map;
use crate::util::{fetch_txs, filter_events_by_topic, get_ws_client};
use crate::Result;
use ethers::types::H256;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};
use std::str::FromStr;
use std::thread::available_parallelism;

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub batch_size: Option<usize>,
    pub block_start: Option<u64>,
    pub block_end: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub db_engine: DbEngine,
}

impl Into<EventHistoryParams> for ScanOptions {
    fn into(self) -> EventHistoryParams {
        EventHistoryParams {
            block_start: self.block_start,
            block_end: self.block_end,
            timestamp_start: self.timestamp_start,
            timestamp_end: self.timestamp_end,
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

pub async fn run(params: ScanOptions, config: Config) -> Result<()> {
    info!(
        "scanning events starting at block={:?} timestamp={:?}",
        params.block_start, params.timestamp_start
    );
    let ws_client = get_ws_client(None).await?;
    let mevshare = EventClient::default();
    let hindsight = Hindsight::new(config.rpc_url_ws).await?;

    let db = Db::new(params.db_engine.to_owned()).await;

    let mut event_params: EventHistoryParams = params.clone().into();
    let batch_size = params.batch_size.unwrap_or(
        // use half the number of cores as default batch size, if available
        // if num cpus cannot be detected, use 4.
        // reason to use half: each event may spawn multiple threads, likely to exceed cpu count.
        available_parallelism()
            .map(|n| usize::from(n) / 2)
            .unwrap_or(4)
            .max(1),
    );

    /* Refine params based on ranges present in DB.
        TODO: ask user if they want to do this.
        Overwriting old results may be desired, but not the default.
        Replace the provided timestamp/block params with the latest respective
        value + 1 in the DB if it's higher than the param.
        We add 1 to prevent duplicates. If an arb is saved in the DB,
        then we know we've scanned & simulated up to that point.
        Timestamp is evaluated by default, falls back to block.
    */
    let db_ranges = db.connect.get_previously_saved_ranges().await?;
    info!("previously saved event ranges: {:?}", db_ranges);
    if let Some(timestamp_start) = params.timestamp_start {
        event_params.timestamp_start = Some(timestamp_start.max(db_ranges.latest_timestamp + 1));
    } else if let Some(block_start) = params.block_start {
        event_params.block_start = Some(block_start.max(db_ranges.latest_block + 1));
    }

    info!(
        "starting at block={:?}, timestamp={:?}",
        event_params.block_start, event_params.timestamp_start
    );

    info!("batch size: {}", batch_size);
    let filter_topics = uniswap_topics();
    /* ========================== event processing ====================================== */
    loop {
        // fetch events (500)
        let events = mevshare
            .event_history(&event_history_url(), event_params.to_owned())
            .await?;

        // update params for next batch of events
        event_params.offset = Some(event_params.offset.unwrap() + events.len() as u64);
        // if the api returns < limit, we've run out of events to process
        // (reached present moment) so we exit
        if events.len() < event_params.limit.unwrap_or(500) as usize {
            break;
        }
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
                .take(batch_size)
                .map(|event| event.to_owned())
                .collect::<Vec<EventHistory>>();
            events_offset += this_batch.len();
            // get txs for relevant events
            txs.append(&mut fetch_txs(&ws_client, &this_batch).await?);
        }

        /* ========================== batch-sized arb processing ========================
           Here, *at least* `batch_size` txs should be passed to `process_orderflow`.
           In `process_orderflow`, *at most* `batch_size` txs are simulated at a time.
           The last iteration will process only (remaining_txs % batch_size) txs, so it's
           most efficient when (txs.len() % batch_size == 0) and/or (txs.len() much greater than batch_size).
        */
        hindsight
            .to_owned()
            .process_orderflow(&txs, batch_size, Some(db.connect.clone()), event_map)
            .await?;
        info!("simulated arbs for {} transactions", txs.len());
        info!("offset: {:?}", event_params.offset);
    }

    Ok(())
}
