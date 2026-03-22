use crate::hub::clutch_node_client::ClutchNodeClient;
use crate::hub::configuration::AppConfig;
use crate::hub::faucet::execute_faucet;
use crate::hub::graphql::build_schema;
use crate::hub::graphql::handler::{graphql_handler, graphql_ws_handler};
use crate::hub::signature_keys::SignatureKeys;
use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Result};
use serde::Deserialize;
use std::sync::Arc;

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

#[derive(Deserialize)]
pub struct FaucetRequestBody {
    /// Recipient address (0x + 40 hex).
    pub address: String,
}

async fn post_faucet(
    body: web::Json<FaucetRequestBody>,
    client: web::Data<Arc<ClutchNodeClient>>,
    config: web::Data<AppConfig>,
) -> HttpResponse {
    if !config.faucet_enabled {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Faucet is disabled (set faucet_enabled = true in config for test networks)"
        }));
    }
    if config.faucet_private_key.trim().is_empty() {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Faucet is not configured (set faucet_private_key to a funded account private key)"
        }));
    }
    if let Err(e) = SignatureKeys::validate_public_key(&body.address) {
        return HttpResponse::BadRequest().json(serde_json::json!({ "error": e }));
    }
    match execute_faucet(
        client.get_ref(),
        config.faucet_private_key.trim(),
        body.address.trim(),
        config.faucet_amount_clt,
    )
    .await
    {
        Ok(node_result) => HttpResponse::Ok().json(serde_json::json!({
            "ok": true,
            "amount_clt": config.faucet_amount_clt,
            "node": node_result
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e })),
    }
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
            .app_data(web::Data::new(ws_manager.clone()))
            .service(web::resource("/health").route(web::get().to(health_check)))
            .service(web::resource("/faucet").route(web::post().to(post_faucet)))
            .service(web::resource("/graphql").route(web::post().to(graphql_handler)))
            .service(
                web::resource("/graphql/ws").route(web::get().to(graphql_ws_handler)),
            )
    })
    .bind(ws_addr)?
    .run()
    .await
}
