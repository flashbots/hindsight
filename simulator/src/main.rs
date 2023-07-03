use simulator::{
    config::Config,
    data::{read_data, write_tx_data},
    sim::sim_bundle,
    util::fetch_txs,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let client = simulator::util::get_ws_client(config.rpc_url_ws.to_owned()).await?;
    let cache_data = read_data(None).await?;
    println!("cache data: {:?}", cache_data.events.len());

    println!(
        "oohh geeez\nauth signer\t{:?}\nrpc url\t\t{:?}",
        config.auth_signer_key, config.rpc_url_ws
    );

    let cached_txs = fetch_txs(client, cache_data.events).await?;
    write_tx_data(None, serde_json::to_string_pretty(&cached_txs)?).await?;
    // let sim_result = sim_bundle(signed_txs, client).await?;

    Ok(())
}
