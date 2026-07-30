pub mod hub;

use clap::Parser;
use hub::clutch_node_client::{ChainInfo, ClutchNodeClient};
use hub::configuration::AppConfig;
use hub::metric::serve_metrics;
use hub::tracing::setup_tracing;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    env: Option<String>,
    #[clap(index = 1)]
    env_positional: Option<String>,
}

/// Node startup can lag this API's own startup (plain `depends_on` in the compose stack only
/// waits for the *container* to start, not for the node's WS RPC server to be ready) — so a
/// bare single-shot `get_chain_info()` would flakily panic on ordinary boot races, not just
/// genuine misconfiguration. Retries on the same 5s cadence as the WS client's own reconnect
/// loop (`connection.rs`) for up to a minute before giving up for real.
async fn get_chain_info_with_retry(client: &ClutchNodeClient) -> ChainInfo {
    const MAX_ATTEMPTS: u32 = 12;
    const RETRY_DELAY_SECS: u64 = 5;
    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match client.get_chain_info().await {
            Ok(info) => return info,
            Err(e) => {
                last_err = e;
                warn!(
                    "get_chain_info attempt {}/{} failed: {}",
                    attempt, MAX_ATTEMPTS, last_err
                );
                tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
            }
        }
    }
    panic!(
        "failed to fetch chain info from node at startup after {} attempts: {}",
        MAX_ATTEMPTS, last_err
    );
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let env_owned = args
        .env
        .or(args.env_positional)
        .unwrap_or_else(|| "default".to_string());
    let config = AppConfig::load_configuration(&env_owned)?;

    setup_tracing(&config.log_level, &config.seq_url, &config.seq_api_key)?;
    serve_metrics(&config.serve_metric_addr);
    let ws_manager = hub::server::connect_websocket(&config.clutch_node_ws_url).await;

    // chain_id is a genesis constant (committed by the node's genesis ChainInit tx), so it's
    // fetched once here rather than polled/refreshed. ponytail: a node swap onto a different
    // chain requires an API restart to pick up the new value — acceptable for a value that
    // never changes on a running chain.
    let chain_info = get_chain_info_with_retry(&ws_manager).await;
    info!(
        "Loaded chain info: chain_id={} is_testnet={} tx_fee={}",
        chain_info.chain_id, chain_info.is_testnet, chain_info.tx_fee
    );

    // A faucet that signs and submits real transfers must never survive onto a non-testnet
    // chain — that would let anyone drain real value. This is boot-time fatal
    // misconfiguration, the one place this codebase panics rather than returning Result.
    if config.faucet_enabled && !chain_info.is_testnet {
        panic!("faucet enabled on a non-testnet chain (chain_id={}); refusing to start", chain_info.chain_id);
    }

    let chain_info = Arc::new(chain_info);
    hub::server::run_graphql_server(&config.ws_addr, ws_manager, config.clone(), chain_info).await?;

    Ok(())
}
