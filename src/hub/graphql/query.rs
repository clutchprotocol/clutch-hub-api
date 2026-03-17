use std::sync::Arc;

use crate::hub::{
    clutch_node_client::ClutchNodeClient,
    graphql::types::{get_auth_user, AuthGuard, AvailableRideRequest, MapBoundsInput, RideRequest},
};
use async_graphql::{Context, Object};
use tracing::error;

#[derive(Default)]
pub struct Query;

#[Object]
impl Query {
    // This query requires authentication
    #[graphql(guard = "AuthGuard")]
    pub async fn user_ride_requests(&self, ctx: &Context<'_>) -> Option<RideRequest> {
        // Get authenticated user from context - safely unwrap because AuthGuard ensures it exists
        let _auth_user = get_auth_user(ctx).expect("User should be authenticated due to AuthGuard");
        
        Some(RideRequest {
            pickup_location: "0".to_string(),
            dropoff_location: "0".to_string(),
        })
    }
    
    // This query doesn't require authentication
    pub async fn ride_request(&self, _ctx: &Context<'_>) -> Option<RideRequest> {
        Some(RideRequest {
            pickup_location: "0".to_string(),
            dropoff_location: "0".to_string(),
        })
    }

    /// Lists available ride requests (not yet accepted). Optionally filter by map bounds.
    pub async fn list_ride_requests(
        &self,
        ctx: &Context<'_>,
        bounds: Option<MapBoundsInput>,
    ) -> async_graphql::Result<Vec<AvailableRideRequest>> {
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("Node client not found"))?
            .clone();

        let params = bounds.map(|b| {
            serde_json::json!({
                "minLat": b.min_lat,
                "maxLat": b.max_lat,
                "minLng": b.min_lng,
                "maxLng": b.max_lng
            })
        });

        let raw_list = client
            .list_ride_requests(params)
            .await
            .map_err(|e| async_graphql::Error::new(e))?;

        let mut result = Vec::with_capacity(raw_list.len());
        for item in raw_list {
            match serde_json::from_value::<AvailableRideRequest>(item) {
                Ok(req) => result.push(req),
                Err(e) => {
                    error!("Failed to parse ride request from node: {}", e);
                }
            }
        }
        Ok(result)
    }

    #[graphql(guard = "AuthGuard")]
    pub async fn account_balance(
        &self,
        ctx: &Context<'_>,
        public_key: Option<String>,
    ) -> async_graphql::Result<u64> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        let address = public_key.unwrap_or_else(|| auth_user.public_key.clone());
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        Ok(client.get_account_balance(&address).await)
    }
}
