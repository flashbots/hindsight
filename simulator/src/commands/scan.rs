use crate::config::Config;
use crate::data::arbs::ArbDb;
use crate::hindsight::{Hindsight, ScanOptions};
use crate::info;
use crate::scanner::event_history_url;
use crate::sim::processor::H256Map;
use crate::util::{fetch_txs, filter_events_by_topic, get_ws_client};
use crate::Result;
use ethers::types::H256;
use mev_share_sse::{EventClient, EventHistory, EventHistoryParams};
use std::str::FromStr;
use std::thread::available_parallelism;

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
        "scanning events starting at block={}",
        params.block_start.unwrap_or(0)
    );
    let ws_client = get_ws_client(None).await?;
    let mevshare = EventClient::default();
    let hindsight = Hindsight::new().init(config).await?;
    let db = ArbDb::new(None).await?;

    let mut done = false;
    let mut event_params: EventHistoryParams = params.clone().into();
    let batch_size = params.batch_size.unwrap_or(
        // use number of cores as default batch size, if available
        // if num cpus cannot be detected, use 4
        available_parallelism().map(|n| usize::from(n)).unwrap_or(4),
    );

    /* Refine params based on ranges present in DB.
        TODO: ask user if they want to do this.
        Overwriting old results may be desired, but not the default.
        Replace the provided timestamp/block params with the latest respective
        value + 1 in the DB if it's higher than the param.
        We add 1 to prevent duplicates. If an arb is saved in the DB,
        then we know we've scanned & simulated up to that point.
        Timestamp arg takes precedent over block if both are provided.
    */
    let db_ranges = db.get_previously_saved_ranges().await?;
    info!("previously saved event ranges: {:?}", db_ranges);
    if params.timestamp_start.is_some() {
        event_params.timestamp_start = Some(
            params
                .timestamp_start
                .unwrap()
                .max(db_ranges.latest_timestamp + 1),
        );
    } else if params.block_start.is_some() {
        event_params.block_start =
            Some(params.block_start.unwrap().max(db_ranges.latest_block + 1));
    }

    info!(
        "starting at block={:?}, timestamp={:?}",
        event_params.block_start, event_params.timestamp_start
    );

    info!("batch size: {}", batch_size);
    let filter_topics = uniswap_topics();
    while !done {
        // fetch events
        let events = mevshare
            .event_history(&event_history_url(), event_params.to_owned())
            .await?;
        // update params & exit condition
        event_params.offset = Some(event_params.offset.unwrap() + events.len() as u64);
        done = events.len() < event_params.limit.unwrap_or(500) as usize;
        info!(
            "fetched {} events. first event timestamp={}",
            events.len(),
            events[0].timestamp
        );
        // filter out irrelevant events
        let events = filter_events_by_topic(&events, &filter_topics);
        // get txs for relevant events
        let txs = fetch_txs(&ws_client, &events).await?;
        // process arbs
        let event_map = events
            .iter()
            .map(|event| (event.hint.hash, event.to_owned()))
            .collect::<H256Map<EventHistory>>();
        hindsight
            .to_owned()
            .process_orderflow(&txs, batch_size, Some(Box::new(db.to_owned())), event_map)
            .await?;
        info!("simulated arbs for {} transactions", txs.len());

        info!("offset: {:?}", event_params.offset);
        // info!("limit: {}", event_params.limit.unwrap());
        // info!("#events: {}", events.len());
    }

    Ok(())
}
