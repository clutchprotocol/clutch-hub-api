use async_graphql::{SimpleObject, Guard, Context, Result, Error, InputObject};
use serde::{Deserialize, Deserializer, Serialize};

/// Parses a GraphQL `String` amount (CLT base units) to `u64`. GraphQL's `Int` is 32-bit
/// and this project's peg (1 USD = 1,000,000 CLT) overflows it at a ~$2,147 fare, so every
/// fare/amount/balance scalar crosses the wire as a decimal string instead. One helper, used
/// by every mutation/query that accepts or returns such a value.
pub fn parse_clt_amount(s: &str) -> Result<u64> {
    let v: u64 = s
        .trim()
        .parse()
        .map_err(|_| Error::new("amount must be a non-negative integer string (CLT base units)"))?;
    if v > i64::MAX as u64 {
        return Err(Error::new("amount exceeds i64::MAX"));
    }
    Ok(v)
}

/// The node still sends fare/fare_paid as bare JSON numbers (only `total_supply` moved to a
/// decimal string on the node side). This deserializes either shape into the `String` these
/// GraphQL fields now expose, so hub-api's own output stays String-safe past 2^53 without
/// requiring a matching node change to these particular fields.
fn u64_as_string<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumberOrString {
        Number(u64),
        String(String),
    }
    match NumberOrString::deserialize(deserializer)? {
        NumberOrString::Number(n) => Ok(n.to_string()),
        NumberOrString::String(s) => Ok(s),
    }
}

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
    #[serde(deserialize_with = "u64_as_string")]
    pub fare: String,
    pub passenger_address: String,
    #[graphql(name = "referrer")]
    #[serde(default)]
    pub referrer: Option<String>,
}

/// A ride offer made by a driver for a specific ride request.
#[derive(SimpleObject, Serialize, Deserialize)]
pub struct AvailableRideOffer {
    pub tx_hash: String,
    pub ride_request_tx_hash: String,
    #[serde(deserialize_with = "u64_as_string")]
    pub fare: String,
    pub driver_address: String,
    #[graphql(name = "referrer")]
    #[serde(default)]
    pub referrer: Option<String>,
}

/// An active or completed trip listing (same shape: `fare_paid` vs `fare` distinguishes state on the node).
#[derive(SimpleObject, Serialize, Deserialize)]
pub struct AvailableActiveTrip {
    pub tx_hash: String,
    pub ride_offer_tx_hash: String,
    pub ride_request_tx_hash: String,
    pub pickup_location: Coordinates,
    pub dropoff_location: Coordinates,
    #[serde(deserialize_with = "u64_as_string")]
    pub fare: String,
    #[serde(deserialize_with = "u64_as_string")]
    pub fare_paid: String,
    pub driver_address: String,
    pub passenger_address: String,
}

/// Finished trip history: full fare paid (`completed`) or cancelled (`cancelled`).
#[derive(SimpleObject, Serialize, Deserialize)]
pub struct AvailableRecentTrip {
    pub tx_hash: String,
    pub ride_offer_tx_hash: String,
    pub ride_request_tx_hash: String,
    pub pickup_location: Coordinates,
    pub dropoff_location: Coordinates,
    #[serde(deserialize_with = "u64_as_string")]
    pub fare: String,
    #[serde(deserialize_with = "u64_as_string")]
    pub fare_paid: String,
    pub driver_address: String,
    pub passenger_address: String,
    pub trip_status: String,
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

/// Genesis-committed consensus parameters, as exposed to clients. All `u64`s cross as
/// `String` — GraphQL `Int` is 32-bit; `chain_id`/`tx_fee` would individually fit, but one
/// rule for every field here is cheaper for the SDK to remember than a per-field exception.
#[derive(SimpleObject, Clone, Debug)]
pub struct ChainInfo {
    pub chain_id: String,
    pub is_testnet: bool,
    pub tx_fee: String,
    pub total_supply: String,
    pub mint_authority: String,
}

/// Recoverable secp256k1 signature over the `generateToken` auth challenge
/// (`clutch-auth:{chain_id}:{publicKey}:{timestamp}` — see `hub::auth`). `r`/`s` are 32-byte
/// hex (0x optional); `v` is 27 or 28.
#[derive(InputObject)]
pub struct AuthSignatureInput {
    pub r: String,
    pub s: String,
    pub v: i32,
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
