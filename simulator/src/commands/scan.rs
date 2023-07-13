use crate::config::Config;
use crate::hindsight::{Hindsight, HindsightOptions, ScanOptions};

use crate::util::get_ws_client;
use crate::Result;
use mev_share_sse::EventHistoryParams;

/// params.{offset, limit} are ignored; this function automagically fetches all events in your specified range.
pub async fn run(
    params: EventHistoryParams,
    filename: Option<String>,
    config: Config,
) -> Result<()> {
    let ws_client = get_ws_client(None).await?;

    let hindsight = Hindsight::new().init(
        config,
        HindsightOptions::Scan(ScanOptions {
            start_block: params.block_start,
            end_block: params.block_end,
            start_timestamp: params.timestamp_start,
            end_timestamp: params.timestamp_end,
            filename,
        }),
    );

    Ok(())
}
