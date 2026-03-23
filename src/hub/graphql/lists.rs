//! Shared node list fetch + parse logic for queries and subscriptions.
use std::sync::Arc;

use async_graphql::Error;
use tracing::error;

use crate::hub::clutch_node_client::ClutchNodeClient;
use super::types::{
    AvailableActiveTrip, AvailableRecentTrip, AvailableRideOffer, AvailableRideRequest, MapBoundsInput,
};

pub async fn list_ride_requests_parsed(
    client: &Arc<ClutchNodeClient>,
    bounds: Option<&MapBoundsInput>,
) -> Result<Vec<AvailableRideRequest>, Error> {
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
        .map_err(Error::new)?;

    let mut result = Vec::with_capacity(raw_list.len());
    for item in raw_list {
        match serde_json::from_value::<AvailableRideRequest>(item) {
            Ok(req) => result.push(req),
            Err(e) => error!("Failed to parse ride request from node: {}", e),
        }
    }
    Ok(result)
}

pub async fn list_ride_offers_parsed(
    client: &Arc<ClutchNodeClient>,
    ride_request_tx_hash: &str,
) -> Result<Vec<AvailableRideOffer>, Error> {
    let raw_list = client
        .list_ride_offers(ride_request_tx_hash)
        .await
        .map_err(Error::new)?;

    let mut result = Vec::with_capacity(raw_list.len());
    for item in raw_list {
        match serde_json::from_value::<AvailableRideOffer>(item) {
            Ok(offer) => result.push(offer),
            Err(e) => error!("Failed to parse ride offer from node: {}", e),
        }
    }
    Ok(result)
}

pub async fn list_active_trips_parsed(
    client: &Arc<ClutchNodeClient>,
    driver_address: Option<&str>,
    passenger_address: Option<&str>,
) -> Result<Vec<AvailableActiveTrip>, Error> {
    let params = serde_json::json!({
        "driver_address": driver_address,
        "passenger_address": passenger_address
    });

    let raw_list = client
        .list_active_trips(params)
        .await
        .map_err(Error::new)?;

    let mut result = Vec::with_capacity(raw_list.len());
    for item in raw_list {
        match serde_json::from_value::<AvailableActiveTrip>(item) {
            Ok(trip) => result.push(trip),
            Err(e) => error!("Failed to parse active trip from node: {}", e),
        }
    }
    Ok(result)
}

pub async fn list_completed_trips_parsed(
    client: &Arc<ClutchNodeClient>,
    driver_address: Option<&str>,
    passenger_address: Option<&str>,
) -> Result<Vec<AvailableActiveTrip>, Error> {
    let params = serde_json::json!({
        "driver_address": driver_address,
        "passenger_address": passenger_address
    });

    let raw_list = client
        .list_completed_trips(params)
        .await
        .map_err(Error::new)?;

    let mut result = Vec::with_capacity(raw_list.len());
    for item in raw_list {
        match serde_json::from_value::<AvailableActiveTrip>(item) {
            Ok(trip) => result.push(trip),
            Err(e) => error!("Failed to parse completed trip from node: {}", e),
        }
    }
    Ok(result)
}

pub async fn list_recent_trips_parsed(
    client: &Arc<ClutchNodeClient>,
    driver_address: Option<&str>,
    passenger_address: Option<&str>,
) -> Result<Vec<AvailableRecentTrip>, Error> {
    let params = serde_json::json!({
        "driver_address": driver_address,
        "passenger_address": passenger_address
    });

    let raw_list = client
        .list_recent_trips(params)
        .await
        .map_err(Error::new)?;

    let mut result = Vec::with_capacity(raw_list.len());
    for item in raw_list {
        match serde_json::from_value::<AvailableRecentTrip>(item) {
            Ok(trip) => result.push(trip),
            Err(e) => error!("Failed to parse recent trip from node: {}", e),
        }
    }
    Ok(result)
}
