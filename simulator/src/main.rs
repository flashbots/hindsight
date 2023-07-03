// use rusty_sando::forked_db::GlobalBackend;
use simulator::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    // let mut backend = GlobalBackend::new();

    println!(
        "oohh geeez\nauth signer\t{:?}\nrpc url\t\t{:?}",
        config.auth_signer_key, config.rpc_url_ws
    );
    Ok(())
}
