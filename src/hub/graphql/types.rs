use async_graphql::{SimpleObject, Guard, Context, Result, Error, InputObject};
use serde::{Deserialize, Serialize};

#[derive(SimpleObject, Serialize, Deserialize)]
pub struct RideRequest {
    pub pickup_location: String,
    pub dropoff_location: String,
}

/// Geographic coordinates (latitude, longitude).
#[derive(SimpleObject, Serialize, Deserialize, Clone)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

/// A ride request available for drivers to accept (no acceptance yet).
#[derive(SimpleObject, Serialize, Deserialize)]
pub struct AvailableRideRequest {
    pub tx_hash: String,
    pub pickup_location: Coordinates,
    pub dropoff_location: Coordinates,
    pub fare: u64,
    pub passenger_address: String,
}

/// A ride offer made by a driver for a specific ride request.
#[derive(SimpleObject, Serialize, Deserialize)]
pub struct AvailableRideOffer {
    pub tx_hash: String,
    pub ride_request_tx_hash: String,
    pub fare: u64,
    pub driver_address: String,
}

/// An active or completed trip listing (same shape: `fare_paid` vs `fare` distinguishes state on the node).
#[derive(SimpleObject, Serialize, Deserialize)]
pub struct AvailableActiveTrip {
    pub tx_hash: String,
    pub ride_offer_tx_hash: String,
    pub ride_request_tx_hash: String,
    pub pickup_location: Coordinates,
    pub dropoff_location: Coordinates,
    pub fare: u64,
    pub fare_paid: u64,
    pub driver_address: String,
    pub passenger_address: String,
}
/// Optional map viewport bounds for filtering ride requests by pickup location.
#[derive(InputObject, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MapBoundsInput {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lng: f64,
    pub max_lng: f64,
}

#[derive(SimpleObject)]
pub struct TokenResponse {
    pub token: String,
    pub expires_at: usize,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub public_key: String,
    // Add additional user fields as needed (name, email, etc.)
}

/// Authentication guard for GraphQL operations
pub struct AuthGuard;

impl Guard for AuthGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        // Try to get the authenticated user from the context
        if ctx.data::<AuthUser>().is_ok() {
            // User is authenticated
            Ok(())
        } else {
            // User is not authenticated
            Err(Error::new("Unauthorized: Authentication required"))
        }
    }
}

/// Helper function to get authenticated user from context
pub fn get_auth_user<'a>(ctx: &'a Context<'_>) -> Option<&'a AuthUser> {
    ctx.data::<AuthUser>().ok()
}
