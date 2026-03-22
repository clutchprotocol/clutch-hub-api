use config::{Config, ConfigError, Environment, File};
use dotenv::dotenv;
use serde::Deserialize;
use tracing::info;

fn default_faucet_amount() -> u64 {
    1000
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub log_level: String,
    pub serve_metric_addr: String,
    pub seq_url: String,
    pub seq_api_key: String,
    pub clutch_node_ws_url: String,
    pub ws_addr: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    /// When true and `faucet_private_key` is set, POST /faucet is enabled (test networks only).
    #[serde(default)]
    pub faucet_enabled: bool,
    /// Hex-encoded secp256k1 private key (64 hex chars, optional 0x). Must hold CLT on-chain.
    #[serde(default)]
    pub faucet_private_key: String,
    /// Amount of CLT to send per faucet request.
    #[serde(default = "default_faucet_amount")]
    pub faucet_amount_clt: u64,
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
        info!("Loaded configuration from env {:?}: {:?}", env, config);
        Ok(config)
    }
}
