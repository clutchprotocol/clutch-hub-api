//! Testnet faucet: signs a Transfer from a funded faucet account and submits via the node.
//! Encoding matches clutch-hub-sdk-js `signTransaction` (Keccak-256 over unsigned RLP, then secp256k1).

use crate::hub::clutch_node_client::ClutchNodeClient;
use crate::hub::signature_keys::SignatureKeys;
use rlp::RlpStream;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};
use std::sync::Arc;
use tracing::info;

fn strip_0x(s: &str) -> &str {
    s.trim_start_matches("0x").trim_start_matches("0X")
}

/// Derive Clutch/Ethereum address (0x + 20 bytes hex) from a secp256k1 private key hex string.
pub fn faucet_address_from_private_key(private_key_hex: &str) -> Result<String, String> {
    let secp = Secp256k1::new();
    let bytes = hex::decode(strip_0x(private_key_hex)).map_err(|e| e.to_string())?;
    let sk = SecretKey::from_slice(&bytes).map_err(|e| e.to_string())?;
    let pk = PublicKey::from_secret_key(&secp, &sk);
    let serialized = pk.serialize_uncompressed();
    let mut hasher = Keccak256::new();
    hasher.update(&serialized[1..]);
    let hash = hasher.finalize();
    Ok(format!("0x{}", hex::encode(&hash[12..32])))
}

fn normalize_address(addr: &str) -> Result<String, String> {
    let h = strip_0x(addr);
    if h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(format!("0x{}", h.to_lowercase()));
    }
    // Accept uncompressed secp256k1 public key (130 chars) and derive address
    if h.len() == 130 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        let bytes = hex::decode(h).map_err(|e| e.to_string())?;
        if bytes.len() == 65 && bytes[0] == 0x04 {
            let mut hasher = Keccak256::new();
            hasher.update(&bytes[1..65]);
            let hash = hasher.finalize();
            return Ok(format!("0x{}", hex::encode(&hash[12..32])));
        }
    }
    Err("address must be 20-byte hex or 130-char uncompressed public key (with or without 0x)".to_string())
}

/// RLP-encode FunctionCall::Transfer matching node's `Encodable for FunctionCall` / `Transfer`.
fn encode_transfer_call(to: &str, value: u64) -> Vec<u8> {
    let to_norm = normalize_address(to).unwrap_or_else(|_| to.to_string());
    let mut transfer = RlpStream::new_list(2);
    transfer.append(&to_norm);
    transfer.append(&value);
    let transfer_out = transfer.out();

    let mut fc = RlpStream::new_list(2);
    fc.append(&0u8);
    fc.append_raw(transfer_out.as_ref(), 1);
    fc.out().as_ref().to_vec()
}

/// RLP-encode unsigned tx `[from, nonce, chain_id, data]` with `from` without 0x prefix (SDK
/// behavior). Must match the node's `calculate_hash` preimage exactly — see
/// `clutch-node/src/node/transactions/transaction.rs`.
fn encode_unsigned_transaction(from_with_0x: &str, nonce: u64, chain_id: u64, data_rlp: &[u8]) -> Vec<u8> {
    let from_clean = strip_0x(from_with_0x).to_string();
    let mut stream = RlpStream::new_list(4);
    stream.append(&from_clean);
    stream.append(&nonce);
    stream.append(&chain_id);
    stream.append_raw(data_rlp, 1);
    stream.out().as_ref().to_vec()
}

/// Keccak-256 digest of bytes; return 64-char lowercase hex **without** 0x (for full tx RLP slot).
fn tx_hash_hex(unsigned_rlp: &[u8]) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(unsigned_rlp);
    hex::encode(hasher.finalize())
}

/// Build signed raw transaction hex (with 0x) for Transfer, matching SDK output. Wire format
/// is the node's 8-item list `[from, nonce, chain_id, r, s, v, hash, data]`, `chain_id` at
/// index 2 — mirrors `clutch-node`'s `accepts_faucet_style_transfer_hash` test exactly.
fn sign_transfer_raw_transaction(
    faucet_private_key_hex: &str,
    faucet_from_address: &str,
    nonce: u64,
    chain_id: u64,
    to: &str,
    value: u64,
) -> Result<String, String> {
    let data_rlp = encode_transfer_call(to, value);
    let unsigned_rlp = encode_unsigned_transaction(faucet_from_address, nonce, chain_id, &data_rlp);
    // Must match SDK + node verify: Keccak256(UTF-8 bytes of the 64-char hex string, no 0x).
    let hash_hex = tx_hash_hex(&unsigned_rlp);
    let (r, s, v) = SignatureKeys::sign(faucet_private_key_hex, hash_hex.as_bytes());

    let from_clean = strip_0x(faucet_from_address).to_string();
    let r_clean = strip_0x(&r).to_string();
    let s_clean = strip_0x(&s).to_string();
    let v_u64 = v as u64;

    let mut full = RlpStream::new_list(8);
    full.append(&from_clean);
    full.append(&nonce);
    full.append(&chain_id);
    full.append(&r_clean);
    full.append(&s_clean);
    full.append(&v_u64);
    full.append(&hash_hex);
    full.append_raw(&data_rlp, 1);

    Ok(format!("0x{}", hex::encode(full.out().as_ref())))
}

/// Execute one faucet drip. Returns message from node on success.
pub async fn execute_faucet(
    client: &Arc<ClutchNodeClient>,
    faucet_private_key: &str,
    recipient: &str,
    amount_clt: u64,
    chain_id: u64,
) -> Result<serde_json::Value, String> {
    let recipient = normalize_address(recipient)?;
    let faucet_addr = faucet_address_from_private_key(faucet_private_key)?;

    let balance = client.get_account_balance(&faucet_addr).await?;
    if balance < amount_clt {
        return Err(format!(
            "faucet account {} has insufficient balance (have {}, need {})",
            faucet_addr, balance, amount_clt
        ));
    }

    let nonce = client.get_next_nonce(&faucet_addr).await?;

    let raw = sign_transfer_raw_transaction(
        faucet_private_key,
        &faucet_addr,
        nonce,
        chain_id,
        &recipient,
        amount_clt,
    )?;

    info!(
        "Faucet sending {} CLT from {} to {}",
        amount_clt, faucet_addr, recipient
    );

    let result = client
        .send_request(
            "send_raw_transaction",
            serde_json::Value::String(raw.clone()),
        )
        .await
        .map_err(|e| format!("node rejected faucet tx: {}", e))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_deploy_faucet_address() {
        // Deploy config faucet key; must match clutch-deploy/config/api/default.toml
        let pk = "d2c446110cfcecbdf05b2be528e72483de5b6f7ef9c7856df2f81f48e9f2748f";
        let addr = faucet_address_from_private_key(pk).unwrap();
        // Use for genesis: ensure this address is funded in new_genesis_transactions
        assert!(!addr.is_empty());
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
    }

    #[test]
    fn unsigned_rlp_deterministic() {
        let data = encode_transfer_call("0x1111111111111111111111111111111111111111", 42);
        let u = encode_unsigned_transaction("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1, 2077, &data);
        assert!(!u.is_empty());
        let h = tx_hash_hex(&u);
        assert_eq!(h.len(), 64);
    }

    /// Guards against silently drifting from the node's wire format: decodes the faucet's own
    /// raw transaction bytes with the `rlp` crate (independent of this file's own encoder) and
    /// asserts exactly 8 items with `chain_id` at index 2, matching
    /// `clutch-node`'s `Encodable`/`Decodable for Transaction` in `rlp_encoding.rs`.
    #[test]
    fn faucet_tx_matches_node_wire_format() {
        let faucet_private_key = "d2c446110cfcecbdf05b2be528e72483de5b6f7ef9c7856df2f81f48e9f2748f";
        let faucet_addr = faucet_address_from_private_key(faucet_private_key).unwrap();
        let chain_id: u64 = 2077;
        let nonce: u64 = 1;
        let to = "0x1111111111111111111111111111111111111111";
        let value: u64 = 100;

        let raw = sign_transfer_raw_transaction(
            faucet_private_key,
            &faucet_addr,
            nonce,
            chain_id,
            to,
            value,
        )
        .unwrap();
        let raw_bytes = hex::decode(strip_0x(&raw)).unwrap();

        let rlp = rlp::Rlp::new(&raw_bytes);
        assert!(rlp.is_list(), "raw transaction must be an RLP list");
        assert_eq!(rlp.item_count().unwrap(), 8, "wire format must be the 8-item list");

        let decoded_chain_id: u64 = rlp.val_at(2).unwrap();
        assert_eq!(decoded_chain_id, chain_id, "chain_id must sit at index 2");

        let decoded_from: String = rlp.val_at(0).unwrap();
        assert_eq!(decoded_from, strip_0x(&faucet_addr));
        let decoded_nonce: u64 = rlp.val_at(1).unwrap();
        assert_eq!(decoded_nonce, nonce);
    }
}
