use crate::hub::clutch_node_client::ClutchNodeClient;
use crate::hub::configuration::AppConfig;
use crate::hub::graphql::build_schema;
use crate::hub::graphql::handler::{graphql_handler, graphql_ws_handler};
use actix_web::{web, App, HttpServer, HttpResponse, Result};
use std::sync::Arc;
use actix_cors::Cors;

pub async fn connect_websocket(wss_url: &str) -> Arc<ClutchNodeClient> {
    let url = wss_url.to_string();
    ClutchNodeClient::new(url)
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
) -> std::io::Result<()> {
    let schema = build_schema(ws_manager.clone(), config.clone());
    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                    .allow_any_header()
            )
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(schema.clone()))
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
