use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hub::{
    auth,
    clutch_node_client::ClutchNodeClient,
    configuration::AppConfig,
    graphql::types::{get_auth_user, AuthGuard, AuthSignatureInput, TokenResponse},
    referrer::configured_referrer,
};
use async_graphql::{Context, Json, Object};
use serde_json::json;
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum MutationError {
    #[error("Authentication failed: {0}")]
    AuthError(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

#[derive(Default)]
pub struct Mutation;

#[Object]
impl Mutation {
    /// Issue a JWT after verifying proof of key ownership: the caller must sign the canonical
    /// challenge `clutch-auth:{publicKey}:{timestamp}` (see `hub::auth`) with the private key
    /// belonging to `public_key`. `timestamp` is unix seconds and must be within ±120s of
    /// server time.
    pub async fn generate_token(
        &self,
        ctx: &Context<'_>,
        public_key: String,
        timestamp: i64,
        signature: AuthSignatureInput,
    ) -> async_graphql::Result<TokenResponse> {
        let config = ctx
            .data::<AppConfig>()
            .map_err(|_| async_graphql::Error::new("Failed to get app config"))?;

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| async_graphql::Error::new("System time error"))?
            .as_secs() as i64;

        auth::verify_auth_challenge(
            &public_key,
            timestamp,
            &signature.r,
            &signature.s,
            signature.v,
            now_secs,
        )
        .map_err(|e| {
            error!("generateToken proof-of-key-ownership failed for {}: {}", public_key, e);
            async_graphql::Error::new(format!("Proof of key ownership failed: {}", e))
        })?;

        let (token, expires_at) = auth::generate_jwt_token(
            &public_key,
            config.jwt_expiration_hours,
            config.jwt_secret.as_str(),
        )
        .map_err(|e| async_graphql::Error::new(format!("Failed to generate token: {}", e)))?;

        Ok(TokenResponse { token, expires_at })
    }

    #[graphql(guard = "AuthGuard")]
    pub async fn create_unsigned_ride_request(
        &self,
        ctx: &Context<'_>,
        pickup_latitude: f64,
        pickup_longitude: f64,
        dropoff_latitude: f64,
        dropoff_longitude: f64,
        fare: i32,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        // Get authenticated user from context
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        let config = ctx
            .data::<AppConfig>()
            .map_err(|_| async_graphql::Error::new("Failed to get app config"))?;

        let resolved_referrer =
            configured_referrer(&config.default_ride_request_referrer);

        info!(
            "Processing ride request for user {} referrer={:?}",
            auth_user.public_key, resolved_referrer
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        // Get the next nonce for this user using the client method
        let nonce = client.get_next_nonce(&auth_user.public_key).await;

        // Create request parameters
        let params = json!({
            "from": auth_user.public_key,
            "nonce": nonce,
            "data": {
                "function_call_type": "RideRequest",
                "arguments": {
                    "fare": fare,
                    "pickup_location": {
                        "latitude": pickup_latitude,
                        "longitude": pickup_longitude
                    },
                    "dropoff_location": {
                        "latitude": dropoff_latitude,
                        "longitude": dropoff_longitude
                    },
                    "referrer": resolved_referrer
                }
            }
        });

        Ok(Json(params))
    }

    #[graphql(guard = "AuthGuard")]
    pub async fn create_unsigned_ride_offer(
        &self,
        ctx: &Context<'_>,
        ride_request_transaction_hash: String,
        fare: i32,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        let config = ctx
            .data::<AppConfig>()
            .map_err(|_| async_graphql::Error::new("Failed to get app config"))?;

        let resolved_referrer = configured_referrer(&config.default_ride_offer_referrer);

        info!(
            "Processing ride offer for user {} on request {} referrer={:?}",
            auth_user.public_key, ride_request_transaction_hash, resolved_referrer
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        let nonce = client.get_next_nonce(&auth_user.public_key).await;

        let params = json!({
            "from": auth_user.public_key,
            "nonce": nonce,
            "data": {
                "function_call_type": "RideOffer",
                "arguments": {
                    "ride_request_transaction_hash": ride_request_transaction_hash,
                    "fare": fare,
                    "referrer": resolved_referrer
                }
            }
        });

        Ok(Json(params))
    }

    #[graphql(guard = "AuthGuard")]
    pub async fn create_unsigned_ride_acceptance(
        &self,
        ctx: &Context<'_>,
        ride_offer_transaction_hash: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        info!(
            "Processing ride acceptance for passenger {} on offer {}",
            auth_user.public_key, ride_offer_transaction_hash
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        let nonce = client.get_next_nonce(&auth_user.public_key).await;

        let params = json!({
            "from": auth_user.public_key,
            "nonce": nonce,
            "data": {
                "function_call_type": "RideAcceptance",
                "arguments": {
                    "ride_offer_transaction_hash": ride_offer_transaction_hash
                }
            }
        });

        Ok(Json(params))
    }

    /// Passenger pays the driver in one or more portions (RidePay). `fare` is this payment amount (CLT).
    #[graphql(guard = "AuthGuard")]
    pub async fn create_unsigned_ride_pay(
        &self,
        ctx: &Context<'_>,
        ride_acceptance_transaction_hash: String,
        fare: i32,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        if fare <= 0 {
            return Err(async_graphql::Error::new("fare must be positive"));
        }

        info!(
            "Processing ride pay for passenger {} on acceptance {}",
            auth_user.public_key, ride_acceptance_transaction_hash
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        let nonce = client.get_next_nonce(&auth_user.public_key).await;

        let params = json!({
            "from": auth_user.public_key,
            "nonce": nonce,
            "data": {
                "function_call_type": "RidePay",
                "arguments": {
                    "ride_acceptance_transaction_hash": ride_acceptance_transaction_hash,
                    "fare": fare
                }
            }
        });

        Ok(Json(params))
    }

    /// Cancel an active ride. Either passenger or driver may cancel. Refunds unpaid fare to passenger.
    /// Cannot cancel if full fare has already been paid.
    #[graphql(guard = "AuthGuard")]
    pub async fn create_unsigned_ride_cancel(
        &self,
        ctx: &Context<'_>,
        ride_acceptance_transaction_hash: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        info!(
            "Processing ride cancel for user {} on acceptance {}",
            auth_user.public_key, ride_acceptance_transaction_hash
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        let nonce = client.get_next_nonce(&auth_user.public_key).await;

        let params = json!({
            "from": auth_user.public_key,
            "nonce": nonce,
            "data": {
                "function_call_type": "RideCancel",
                "arguments": {
                    "ride_acceptance_transaction_hash": ride_acceptance_transaction_hash
                }
            }
        });

        Ok(Json(params))
    }

    /// Cancel a pending ride request (before a driver accepts). Only the passenger who created the request can cancel.
    #[graphql(guard = "AuthGuard")]
    pub async fn create_unsigned_ride_request_cancel(
        &self,
        ctx: &Context<'_>,
        ride_request_transaction_hash: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        info!(
            "Processing ride request cancel for user {} on request {}",
            auth_user.public_key, ride_request_transaction_hash
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        let nonce = client.get_next_nonce(&auth_user.public_key).await;

        let params = json!({
            "from": auth_user.public_key,
            "nonce": nonce,
            "data": {
                "function_call_type": "RideRequestCancel",
                "arguments": {
                    "ride_request_transaction_hash": ride_request_transaction_hash
                }
            }
        });

        Ok(Json(params))
    }

    #[graphql(guard = "AuthGuard")]
    pub async fn send_raw_transaction(
        &self,
        ctx: &Context<'_>,
        raw_transaction: String,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        info!(
            "Submitting transaction for user with public key: {}",
            auth_user.public_key
        );

        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        // Ensure the raw transaction is properly formatted (has 0x prefix)
        let formatted_tx = if !raw_transaction.starts_with("0x") {
            format!("0x{}", raw_transaction)
        } else {
            raw_transaction
        };

        // Send the transaction to the node
        // The client.send_request method will handle formatting the request properly
        let result = client
            .send_request("send_raw_transaction", serde_json::Value::String(formatted_tx))
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to send transaction: {}", e)))?;

        // Return the result as JSON
        Ok(Json(result))
    }
}
