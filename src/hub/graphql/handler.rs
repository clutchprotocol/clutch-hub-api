use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use async_graphql::Data;
use async_graphql::Schema;
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::hub::configuration::AppConfig;
use crate::hub::graphql::{Mutation, Query, Subscription};
use crate::hub::graphql::types::AuthUser;

// Define JWT claim structure
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    pub pk: String, // public key
    pub exp: usize, // expiration time
}

/// Validate a `Bearer …` header value (or raw JWT) and return `AuthUser` when valid.
pub(crate) fn extract_auth_user_from_bearer(auth_str: &str, config: &AppConfig) -> Option<AuthUser> {
    let token = auth_str
        .strip_prefix("Bearer ")
        .map(str::trim)
        .unwrap_or(auth_str.trim());

    let secret = config.jwt_secret.as_bytes();

    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(token_data) => Some(AuthUser {
            public_key: token_data.claims.pk.clone(),
        }),
        Err(err) => {
            error!("JWT validation failed: {}", err);
            None
        }
    }
}

// Extract JWT token from Authorization header and validate it
fn extract_auth_user(req: &HttpRequest, config: &AppConfig) -> Option<AuthUser> {
    let auth_header = req.headers().get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    extract_auth_user_from_bearer(auth_str, config)
}

pub async fn graphql_handler(
    schema: web::Data<Schema<Query, Mutation, Subscription>>,
    config: web::Data<AppConfig>,
    req: GraphQLRequest,
    http_req: HttpRequest,
) -> GraphQLResponse {
    let auth_user = extract_auth_user(&http_req, &config);

    let mut request = req.into_inner();
    if let Some(user) = auth_user {
        request = request.data(user);
    }

    schema.execute(request).await.into()
}

pub async fn graphql_ws_handler(
    schema: web::Data<Schema<Query, Mutation, Subscription>>,
    config: web::Data<AppConfig>,
    req: HttpRequest,
    payload: web::Payload,
) -> ActixResult<HttpResponse> {
    let config = config.get_ref().clone();

    GraphQLSubscription::new(schema.get_ref().clone())
        .on_connection_init(move |value| {
            let config = config.clone();
            async move {
                let mut data = Data::default();
                if let Some(auth_str) = value
                    .get("Authorization")
                    .or_else(|| value.get("authorization"))
                    .and_then(|v| v.as_str())
                {
                    if let Some(user) = extract_auth_user_from_bearer(auth_str, &config) {
                        data.insert(user);
                    }
                }
                Ok(data)
            }
        })
        .start(&req, payload)
}
