use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::hub::clutch_node_client::ClutchNodeClient;
use super::lists;
use super::types::{
    AvailableActiveTrip, AvailableRecentTrip, AvailableRideOffer, AvailableRideRequest, MapBoundsInput,
};
use async_graphql::{Context, Error, Result};
use futures_util::stream::{self, Stream};

/// Default delay between node polls for subscription snapshots (after the first immediate tick).
const SNAPSHOT_INTERVAL_MS: u64 = 1000;
/// Slightly faster polling for ride-offer lists (many UX-critical updates).
const OFFERS_SNAPSHOT_INTERVAL_MS: u64 = 500;

type RideRequestStream = Pin<Box<dyn Stream<Item = Result<Vec<AvailableRideRequest>>> + Send>>;
type RideOfferStream = Pin<Box<dyn Stream<Item = Result<Vec<AvailableRideOffer>>> + Send>>;
type ActiveTripStream = Pin<Box<dyn Stream<Item = Result<Vec<AvailableActiveTrip>>> + Send>>;
type RecentTripStream = Pin<Box<dyn Stream<Item = Result<Vec<AvailableRecentTrip>>> + Send>>;
type AccountBalanceStream = Pin<Box<dyn Stream<Item = Result<u64>> + Send>>;

fn require_node_client(ctx: &Context<'_>) -> Result<Arc<ClutchNodeClient>> {
    ctx.data::<Arc<ClutchNodeClient>>()
        .map(|c| c.clone())
        .map_err(|_| Error::new("Node client not found"))
}

#[derive(Default)]
pub struct Subscription;

#[async_graphql::Subscription]
impl Subscription {
    /// Periodically mirrors `listRideRequests` while the subscription is active.
    async fn ride_requests_updated(
        &self,
        ctx: &Context<'_>,
        bounds: Option<MapBoundsInput>,
    ) -> RideRequestStream {
        let client = match require_node_client(ctx) {
            Ok(c) => c,
            Err(e) => {
                let s: RideRequestStream = Box::pin(stream::once(async move { Err(e) }));
                return s;
            }
        };

        let bounds = bounds.clone();
        let s: RideRequestStream = Box::pin(stream::unfold(0u32, move |n| {
            let client = client.clone();
            let bounds = bounds.clone();
            async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS)).await;
                }
                let res = lists::list_ride_requests_parsed(&client, bounds.as_ref()).await;
                Some((res, n + 1))
            }
        }));
        s
    }

    /// Periodically mirrors `listRideOffers` for one ride request.
    async fn ride_offers_updated(
        &self,
        ctx: &Context<'_>,
        ride_request_tx_hash: String,
    ) -> RideOfferStream {
        let client = match require_node_client(ctx) {
            Ok(c) => c,
            Err(e) => {
                let s: RideOfferStream = Box::pin(stream::once(async move { Err(e) }));
                return s;
            }
        };

        let s: RideOfferStream = Box::pin(stream::unfold(0u32, move |n| {
            let client = client.clone();
            let hash = ride_request_tx_hash.clone();
            async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(OFFERS_SNAPSHOT_INTERVAL_MS)).await;
                }
                let res = lists::list_ride_offers_parsed(&client, &hash).await;
                Some((res, n + 1))
            }
        }));
        s
    }

    /// Periodically mirrors `listActiveTrips`.
    async fn active_trips_updated(
        &self,
        ctx: &Context<'_>,
        driver_address: Option<String>,
        passenger_address: Option<String>,
    ) -> ActiveTripStream {
        let client = match require_node_client(ctx) {
            Ok(c) => c,
            Err(e) => {
                let s: ActiveTripStream = Box::pin(stream::once(async move { Err(e) }));
                return s;
            }
        };

        let s: ActiveTripStream = Box::pin(stream::unfold(0u32, move |n| {
            let client = client.clone();
            let driver = driver_address.clone();
            let passenger = passenger_address.clone();
            async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS)).await;
                }
                let res = lists::list_active_trips_parsed(
                    &client,
                    driver.as_deref(),
                    passenger.as_deref(),
                )
                .await;
                Some((res, n + 1))
            }
        }));
        s
    }

    /// Periodically mirrors `listCompletedTrips`.
    async fn completed_trips_updated(
        &self,
        ctx: &Context<'_>,
        driver_address: Option<String>,
        passenger_address: Option<String>,
    ) -> ActiveTripStream {
        let client = match require_node_client(ctx) {
            Ok(c) => c,
            Err(e) => {
                let s: ActiveTripStream = Box::pin(stream::once(async move { Err(e) }));
                return s;
            }
        };

        let s: ActiveTripStream = Box::pin(stream::unfold(0u32, move |n| {
            let client = client.clone();
            let driver = driver_address.clone();
            let passenger = passenger_address.clone();
            async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS)).await;
                }
                let res = lists::list_completed_trips_parsed(
                    &client,
                    driver.as_deref(),
                    passenger.as_deref(),
                )
                .await;
                Some((res, n + 1))
            }
        }));
        s
    }

    /// Periodically mirrors `listRecentTrips` (completed + cancelled).
    async fn recent_trips_updated(
        &self,
        ctx: &Context<'_>,
        driver_address: Option<String>,
        passenger_address: Option<String>,
    ) -> RecentTripStream {
        let client = match require_node_client(ctx) {
            Ok(c) => c,
            Err(e) => {
                let s: RecentTripStream = Box::pin(stream::once(async move { Err(e) }));
                return s;
            }
        };

        let s: RecentTripStream = Box::pin(stream::unfold(0u32, move |n| {
            let client = client.clone();
            let driver = driver_address.clone();
            let passenger = passenger_address.clone();
            async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS)).await;
                }
                let res = lists::list_recent_trips_parsed(
                    &client,
                    driver.as_deref(),
                    passenger.as_deref(),
                )
                .await;
                Some((res, n + 1))
            }
        }));
        s
    }

    /// Periodically mirrors `accountBalance(publicKey)` for the given public key.
    async fn account_balance_updated(
        &self,
        ctx: &Context<'_>,
        public_key: String,
    ) -> AccountBalanceStream {
        let client = match require_node_client(ctx) {
            Ok(c) => c,
            Err(e) => {
                let s: AccountBalanceStream = Box::pin(stream::once(async move { Err(e) }));
                return s;
            }
        };

        let s: AccountBalanceStream = Box::pin(stream::unfold(0u32, move |n| {
            let client = client.clone();
            let pk = public_key.clone();
            async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS)).await;
                }
                let bal = client.get_account_balance(&pk).await;
                Some((Ok(bal), n + 1))
            }
        }));

        s
    }
}
