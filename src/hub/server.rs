use crate::hub::clutch_node_client::{ChainInfo, ClutchNodeClient};
use crate::hub::configuration::AppConfig;
use crate::hub::graphql::build_schema;
use crate::hub::graphql::handler::{graphql_handler, graphql_ws_handler};
use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Result};
use std::sync::Arc;

pub async fn connect_websocket(wss_url: &str) -> Arc<ClutchNodeClient> {
    let url = wss_url.to_string();
    ClutchNodeClient::new(url)
}

/// `"*"` allows any origin (local/dev default); otherwise a comma-separated allowlist.
/// Mirrors clutch-explorer's `app.rs` CORS pattern.
fn build_cors(allowed_origins: &str) -> Cors {
    let cors = if allowed_origins.trim() == "*" {
        Cors::default().allow_any_origin()
    } else {
        allowed_origins
            .split(',')
            .map(str::trim)
            .filter(|o| !o.is_empty())
            .fold(Cors::default(), |cors, origin| cors.allowed_origin(origin))
    };

    cors.allowed_methods(vec!["GET", "POST", "OPTIONS"])
        .allow_any_header()
}

async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "clutch-hub-api",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

pub async fn run_graphql_server(
    ws_addr: &str,
    ws_manager: Arc<ClutchNodeClient>,
    config: AppConfig,
    chain_info: Arc<ChainInfo>,
) -> std::io::Result<()> {
    let schema = build_schema(ws_manager.clone(), config.clone(), chain_info);
    let allowed_origins = config.allowed_origins.clone();
    HttpServer::new(move || {
        App::new()
            .wrap(build_cors(&allowed_origins))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(schema.clone()))
            .app_data(web::Data::new(ws_manager.clone()))
            .service(web::resource("/health").route(web::get().to(health_check)))
            .service(web::resource("/graphql").route(web::post().to(graphql_handler)))
            .service(
                web::resource("/graphql/ws").route(web::get().to(graphql_ws_handler)),
            )
    })
    .bind(ws_addr)?
    .run()
    .await
}
