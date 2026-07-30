use super::connection::start_connection_loop;
use super::types::{JSONRPCRequest, JSONRPCResponse};
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use serde::Deserialize;
use serde_json;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::info;
use uuid::Uuid;

/// Node's `get_chain_info` response. `total_supply` is a decimal string on the wire (the
/// one field that can exceed 2^53); every other field is a bare JSON number — parsed here,
/// never coerced, per the node's own `build_chain_info_response` doc comment.
#[derive(Debug, Clone, Deserialize)]
pub struct ChainInfo {
    pub chain_id: u64,
    pub is_testnet: bool,
    pub tx_fee: u64,
    #[serde(deserialize_with = "deserialize_total_supply")]
    pub total_supply: u64,
    pub mint_authority: String,
}

fn deserialize_total_supply<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let s: String = Deserialize::deserialize(d)?;
    s.parse().map_err(serde::de::Error::custom)
}

pub struct ClutchNodeClient {
    ws_sink: Arc<Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>>,
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
}

impl ClutchNodeClient {
    /// Creates a new WebSocketManager and starts the connection task.
    pub fn new(url: String) -> Arc<Self> {
        let ws_sink = Arc::new(Mutex::new(None));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));

        // Start the background connection task
        let ws_sink_clone = ws_sink.clone();
        let pending_requests_clone = pending_requests.clone();
        tokio::spawn(async move {
            start_connection_loop(url, ws_sink_clone, pending_requests_clone).await;
        });

        Arc::new(ClutchNodeClient {
            ws_sink,
            pending_requests,
        })
    }

    /// Sends a request and awaits the response.
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = Uuid::new_v4().to_string();
        
        // Format the request based on the method type
        // For send_raw_transaction, params should be a direct string not an object
        let request = if method == "send_raw_transaction" {
            // For send_raw_transaction, extract the string from the Value
            let tx_string = match &params {
                serde_json::Value::String(s) => s.clone(),
                _ => params.as_str().unwrap_or_default().to_string(),
            };
            
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                params: serde_json::Value::String(tx_string),
                id: id.clone(),
            }
        } else {
            // For other methods, use the params as provided
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                params,
                id: id.clone(),
            }
        };

        let request_json = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        
        // Log the actual request being sent for debugging
        info!("Sending request to node: {}", request_json);

        // Check if the connection is established
        let mut ws_sink_lock = self.ws_sink.lock().await;
        if let Some(ws_sink) = ws_sink_lock.as_mut() {
            let (resp_tx, resp_rx) = oneshot::channel();

            {
                let mut pending = self.pending_requests.lock().await;
                pending.insert(id.clone(), resp_tx);
            }

            // Send the request
            if let Err(e) = ws_sink.send(Message::Text(request_json)).await {
                // Sending failed, remove the pending request
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                return Err(format!("Failed to send request: {}", e));
            }

            // Wait for response with timeout
            let response_result = timeout(Duration::from_secs(10), resp_rx).await;

            match response_result {
                Ok(Ok(response_json)) => {
                    if response_json.is_empty() {
                        // Connection lost, and no response received
                        return Err("Connection lost before receiving response".to_string());
                    }

                    // Log the response for debugging
                    info!("Received response: {}", response_json);

                    // Parse the response
                    let response: JSONRPCResponse =
                        serde_json::from_str(&response_json).map_err(|e| e.to_string())?;
                    if response.id != id {
                        return Err("Mismatched response ID".to_string());
                    }
                    if let Some(error) = response.error {
                        Err(error.message)
                    } else if let Some(result) = response.result {
                        Ok(result)
                    } else {
                        Err("No result or error in response".to_string())
                    }
                }
                Ok(Err(_)) => {
                    // Sender was dropped
                    Err("Failed to receive response".to_string())
                }
                Err(_) => {
                    // Timeout occurred
                    let mut pending = self.pending_requests.lock().await;
                    pending.remove(&id);
                    Err("Request timed out".to_string())
                }
            }
        } else {
            Err("WebSocket connection not established".to_string())
        }
    }

    /// Gets the next nonce value for the given address.
    ///
    /// A down/unreachable node used to fall back to nonce 1, which produces a transaction
    /// that either collides with an already-used nonce or silently skips ahead — either way
    /// a confusing rejection far from the real cause. Propagate the error instead.
    pub async fn get_next_nonce(&self, address: &str) -> Result<u64, String> {
        let result = self
            .send_request("get_next_nonce", json!({ "address": address }))
            .await
            .map_err(|e| format!("Failed to get nonce for address {}: {}", address, e))?;

        match result.get("nonce").and_then(|n| n.as_u64()) {
            Some(nonce) => {
                info!("Retrieved nonce {} for address {}", nonce, address);
                Ok(nonce)
            }
            None => Err(format!(
                "Failed to parse nonce value from node response for address {}: {:?}",
                address, result
            )),
        }
    }

    /// Gets the current balance for the given address.
    ///
    /// A down/unreachable node used to fall back to balance 0, which callers could mistake
    /// for "this account is empty" rather than "we don't actually know". Propagate the error
    /// instead.
    pub async fn get_account_balance(&self, address: &str) -> Result<u64, String> {
        let result = self
            .send_request("get_account_balance", json!({ "address": address }))
            .await
            .map_err(|e| format!("Failed to get balance for address {}: {}", address, e))?;

        match result.get("balance").and_then(|n| n.as_u64()) {
            Some(balance) => {
                info!("Retrieved balance {} for address {}", balance, address);
                Ok(balance)
            }
            None => Err(format!(
                "Failed to parse balance value from node response for address {}: {:?}",
                address, result
            )),
        }
    }

    /// Fetches genesis-committed chain parameters + `total_supply`. This is a genesis
    /// constant (see `ChainInit`), so callers fetch it once at startup rather than polling.
    pub async fn get_chain_info(&self) -> Result<ChainInfo, String> {
        let result = self
            .send_request("get_chain_info", json!({}))
            .await
            .map_err(|e| format!("Failed to get chain info: {}", e))?;

        serde_json::from_value(result.clone())
            .map_err(|e| format!("Failed to parse chain info from node response: {} ({:?})", e, result))
    }

    /// Lists available ride requests from the node, optionally filtered by map bounds.
    /// Pass None for bounds to get all available ride requests.
    pub async fn list_ride_requests(
        &self,
        bounds: Option<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let params = match bounds {
            Some(b) if b.is_object() && !b.as_object().unwrap().is_empty() => b,
            _ => serde_json::Value::Object(serde_json::Map::new()),
        };

        let result = self.send_request("list_ride_requests", params).await?;

        match result {
            serde_json::Value::Array(arr) => Ok(arr),
            _ => Err("Expected array of ride requests in response".to_string()),
        }
    }

    /// Lists ride offers for a specific ride request.
    pub async fn list_ride_offers(
        &self,
        ride_request_tx_hash: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let params = serde_json::json!({
            "ride_request_tx_hash": ride_request_tx_hash
        });

        let result = self.send_request("list_ride_offers", params).await?;

        match result {
            serde_json::Value::Array(arr) => Ok(arr),
            _ => Err("Expected array of ride offers in response".to_string()),
        }
    }

    /// Lists active trips (ride accepted, in progress). Optionally filter by driver_address and/or passenger_address.
    pub async fn list_active_trips(
        &self,
        params: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, String> {
        let params_obj = if params.is_object() {
            params
        } else {
            serde_json::json!({})
        };

        let result = self.send_request("list_active_trips", params_obj).await?;

        match result {
            serde_json::Value::Array(arr) => Ok(arr),
            _ => Err("Expected array of active trips in response".to_string()),
        }
    }

    /// Lists completed trips (full fare paid, not cancelled). Optionally filter by driver/passenger.
    pub async fn list_completed_trips(
        &self,
        params: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, String> {
        let params_obj = if params.is_object() {
            params
        } else {
            serde_json::json!({})
        };

        let result = self.send_request("list_completed_trips", params_obj).await?;

        match result {
            serde_json::Value::Array(arr) => Ok(arr),
            _ => Err("Expected array of completed trips in response".to_string()),
        }
    }

    /// Lists recent finished trips (completed or cancelled). Optionally filter by driver/passenger.
    pub async fn list_recent_trips(
        &self,
        params: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, String> {
        let params_obj = if params.is_object() {
            params
        } else {
            serde_json::json!({})
        };

        let result = self.send_request("list_recent_trips", params_obj).await?;

        match result {
            serde_json::Value::Array(arr) => Ok(arr),
            _ => Err("Expected array of recent trips in response".to_string()),
        }
    }
}
