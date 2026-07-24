use config::{Config, ConfigError, Environment, File};
use dotenv::dotenv;
use serde::Deserialize;
use tracing::info;

fn default_faucet_amount() -> u64 {
    1000
}

fn default_allowed_origins() -> String {
    "*".to_string()
}

#[derive(Deserialize, Clone)]
pub struct AppConfig {
    pub log_level: String,
    pub serve_metric_addr: String,
    pub seq_url: String,
    pub seq_api_key: String,
    pub clutch_node_ws_url: String,
    pub ws_addr: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    /// CORS: `"*"` or a comma-separated list of allowed origins (e.g.
    /// `https://app.example.com,https://app-stage.example.com`).
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: String,
    /// When true and `faucet_private_key` is set, POST /faucet is enabled (test networks only).
    #[serde(default)]
    pub faucet_enabled: bool,
    /// Hex-encoded secp256k1 private key (64 hex chars, optional 0x). Must hold CLT on-chain.
    #[serde(default)]
    pub faucet_private_key: String,
    /// Amount of CLT to send per faucet request.
    #[serde(default = "default_faucet_amount")]
    pub faucet_amount_clt: u64,
    /// Default referrer address for RideRequest when the client omits `referrer` (empty = none).
    #[serde(default)]
    pub default_ride_request_referrer: String,
    /// Default referrer address for RideOffer when the client omits `referrer` (empty = none).
    #[serde(default)]
    pub default_ride_offer_referrer: String,
}

// Hand-written so secrets never get dumped into logs/Seq via the startup info! below.
impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("log_level", &self.log_level)
            .field("serve_metric_addr", &self.serve_metric_addr)
            .field("seq_url", &self.seq_url)
            .field("seq_api_key", &"[redacted]")
            .field("clutch_node_ws_url", &self.clutch_node_ws_url)
            .field("ws_addr", &self.ws_addr)
            .field("jwt_secret", &"[redacted]")
            .field("jwt_expiration_hours", &self.jwt_expiration_hours)
            .field("allowed_origins", &self.allowed_origins)
            .field("faucet_enabled", &self.faucet_enabled)
            .field("faucet_private_key", &"[redacted]")
            .field("faucet_amount_clt", &self.faucet_amount_clt)
            .field(
                "default_ride_request_referrer",
                &self.default_ride_request_referrer,
            )
            .field(
                "default_ride_offer_referrer",
                &self.default_ride_offer_referrer,
            )
            .finish()
    }
}

/// Markers of placeholder values shipped in example/dev configs (this repo's own
/// env.example and clutch-deploy's config both ship one) — matched as substrings,
/// case-insensitively, so a copy-pasted-but-unedited placeholder is still caught even
/// if it's padded to pass the length check (e.g. "change-me-to-a-long-random-secret").
const WEAK_JWT_SECRET_MARKERS: &[&str] = &[
    "change-me",
    "changeme",
    "your-secret",
    "your-super-secret",
    "secret-here",
    "placeholder",
];
const WEAK_JWT_SECRETS_EXACT: &[&str] = &["secret", "password", "changeme"];

const MIN_JWT_SECRET_LEN: usize = 32;

fn validate_jwt_secret(secret: &str) -> Result<(), String> {
    if secret.trim().is_empty() {
        return Err("jwt_secret is empty — set APP_JWT_SECRET".to_string());
    }
    let lower = secret.to_lowercase();
    if WEAK_JWT_SECRETS_EXACT.contains(&lower.as_str())
        || WEAK_JWT_SECRET_MARKERS.iter().any(|m| lower.contains(m))
    {
        return Err(
            "jwt_secret is set to a known placeholder value — set a real secret via APP_JWT_SECRET"
                .to_string(),
        );
    }
    if secret.len() < MIN_JWT_SECRET_LEN {
        return Err(format!(
            "jwt_secret is too short ({} chars, need >= {}) to be secure",
            secret.len(),
            MIN_JWT_SECRET_LEN
        ));
    }
    Ok(())
}

impl AppConfig {
    fn from_env(env: &str) -> Result<Self, ConfigError> {
        dotenv().ok();
        let file_path = format!("config/{}.toml", env);
        let builder = Config::builder()
            .add_source(File::with_name(&file_path))
            .add_source(Environment::with_prefix("APP"));

        builder.build()?.try_deserialize::<Self>()
    }

    pub fn load_configuration(env: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = AppConfig::from_env(env)?;
        validate_jwt_secret(&config.jwt_secret)
            .map_err(|e| format!("invalid configuration: {}", e))?;
        info!("Loaded configuration from env {:?}: {:?}", env, config);
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::validate_jwt_secret;

    #[test]
    fn rejects_empty_and_placeholder_secrets() {
        assert!(validate_jwt_secret("").is_err());
        assert!(validate_jwt_secret("change-me-in-production").is_err());
        assert!(validate_jwt_secret("Change-Me-In-Production").is_err());
        assert!(validate_jwt_secret("your-super-secret-jwt-key-here").is_err());
        assert!(validate_jwt_secret("short").is_err());
        // Padded-but-unedited placeholders must not slip through on length alone.
        assert!(validate_jwt_secret("change-me-to-a-long-random-secret").is_err());
    }

    #[test]
    fn accepts_long_random_looking_secret() {
        assert!(validate_jwt_secret("iP8BoK3dJfTQGz5UyXq9NwL7e0vCmAhR6S2YxE1ZpDt4").is_ok());
    }
}
