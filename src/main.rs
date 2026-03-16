pub mod hub;

use clap::Parser;
use hub::configuration::AppConfig;
use hub::metric::serve_metrics;
use hub::tracing::setup_tracing;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    env: Option<String>,
    #[clap(index = 1)]
    env_positional: Option<String>,
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
    hub::server::run_graphql_server(&config.ws_addr, ws_manager, config.clone()).await?;

    Ok(())
}
