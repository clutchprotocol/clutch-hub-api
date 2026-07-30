use std::sync::Arc;

use crate::hub::{
    clutch_node_client::{ChainInfo as NodeChainInfo, ClutchNodeClient},
    graphql::lists,
    graphql::types::{
        get_auth_user, AuthGuard, AvailableActiveTrip, AvailableRecentTrip, AvailableRideOffer,
        AvailableRideRequest, ChainInfo, MapBoundsInput, RideRequest,
    },
};
use async_graphql::{Context, Object};

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

        lists::list_ride_requests_parsed(&client, bounds.as_ref()).await
    }

    /// Lists ride offers for a specific ride request.
    pub async fn list_ride_offers(
        &self,
        ctx: &Context<'_>,
        ride_request_tx_hash: String,
    ) -> async_graphql::Result<Vec<AvailableRideOffer>> {
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("Node client not found"))?
            .clone();

        lists::list_ride_offers_parsed(&client, &ride_request_tx_hash).await
    }

    /// Lists active trips (ride accepted, in progress). Optionally filter by driver or passenger address.
    pub async fn list_active_trips(
        &self,
        ctx: &Context<'_>,
        driver_address: Option<String>,
        passenger_address: Option<String>,
    ) -> async_graphql::Result<Vec<AvailableActiveTrip>> {
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("Node client not found"))?
            .clone();

        lists::list_active_trips_parsed(
            &client,
            driver_address.as_deref(),
            passenger_address.as_deref(),
        )
        .await
    }

    /// Lists completed trips (ride accepted, full fare paid, not cancelled). Optional driver/passenger filter.
    pub async fn list_completed_trips(
        &self,
        ctx: &Context<'_>,
        driver_address: Option<String>,
        passenger_address: Option<String>,
    ) -> async_graphql::Result<Vec<AvailableActiveTrip>> {
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("Node client not found"))?
            .clone();

        lists::list_completed_trips_parsed(
            &client,
            driver_address.as_deref(),
            passenger_address.as_deref(),
        )
        .await
    }

    /// Lists recent finished trips (full fare paid or cancelled). Optional driver/passenger filter.
    pub async fn list_recent_trips(
        &self,
        ctx: &Context<'_>,
        driver_address: Option<String>,
        passenger_address: Option<String>,
    ) -> async_graphql::Result<Vec<AvailableRecentTrip>> {
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("Node client not found"))?
            .clone();

        lists::list_recent_trips_parsed(
            &client,
            driver_address.as_deref(),
            passenger_address.as_deref(),
        )
        .await
    }

    #[graphql(guard = "AuthGuard")]
    pub async fn account_balance(
        &self,
        ctx: &Context<'_>,
        public_key: Option<String>,
    ) -> async_graphql::Result<String> {
        let auth_user = get_auth_user(ctx)
            .ok_or_else(|| async_graphql::Error::new("User not authenticated"))?;

        let address = public_key.unwrap_or_else(|| auth_user.public_key.clone());
        let client = ctx
            .data::<Arc<ClutchNodeClient>>()
            .map_err(|_| async_graphql::Error::new("WebSocket manager not found"))?
            .clone();

        client
            .get_account_balance(&address)
            .await
            .map(|b| b.to_string())
            .map_err(|e| async_graphql::Error::new(format!("Failed to get balance: {}", e)))
    }

    /// Genesis-committed chain parameters, fetched once at startup (see `main.rs`) — a chain
    /// swap requires an API restart to pick up new values, which is the intended trade-off
    /// for a genesis constant.
    pub async fn chain_info(&self, ctx: &Context<'_>) -> async_graphql::Result<ChainInfo> {
        let info = ctx
            .data::<Arc<NodeChainInfo>>()
            .map_err(|_| async_graphql::Error::new("Chain info not found"))?;

        Ok(ChainInfo {
            chain_id: info.chain_id.to_string(),
            is_testnet: info.is_testnet,
            tx_fee: info.tx_fee.to_string(),
            total_supply: info.total_supply.to_string(),
            mint_authority: info.mint_authority.clone(),
        })
    }
}
