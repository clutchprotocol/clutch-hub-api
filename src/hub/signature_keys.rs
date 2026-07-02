use hex::FromHex;
use rand::rngs::OsRng;
use secp256k1::{
    ecdsa::RecoverableSignature, ecdsa::RecoveryId, Message, PublicKey, Secp256k1, SecretKey,
};
use sha3::{Digest, Keccak256};

#[derive(Debug)]
#[allow(dead_code)]
pub struct SignatureKeys {
    pub secret_key: String,
    pub public_key: String,
    pub address_key: String,
}

impl SignatureKeys {
    #[allow(dead_code)]
    pub fn generate_new_keypair() -> Self {
        let secp = Secp256k1::new();
        let mut rng = OsRng::default();
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        let address_key = Self::derive_address(&public_key);

        SignatureKeys {
            secret_key: hex::encode(secret_key.as_ref()),
            public_key: hex::encode(public_key.serialize_uncompressed()),
            address_key: address_key,
        }
    }

    fn derive_address(public_key: &PublicKey) -> String {
        let serialized_pubkey = public_key.serialize_uncompressed();
        let mut hasher = Keccak256::new();
        hasher.update(&serialized_pubkey[1..]);
        let hash = hasher.finalize();

        let address_key = format!("0x{}", hex::encode(&hash[12..32]));
        address_key
    }

    /// Strip 0x/0X prefix for hex parsing (Rust hex crate does not accept it)
    fn strip_hex_prefix(s: &str) -> &str {
        s.trim_start_matches("0x").trim_start_matches("0X")
    }

    pub fn sign(secret_key: &str, data: &[u8]) -> (String, String, i32) {
        let secp = Secp256k1::new();

        let secret_key_bytes = hex::decode(Self::strip_hex_prefix(secret_key)).unwrap();
        let secret_key = SecretKey::from_slice(&secret_key_bytes).unwrap();

        // Create a message hash (Keccak-256 of the data)
        let mut hasher = Keccak256::new();
        hasher.update(data);
        let message_hash = hasher.finalize();

        // Create a message object for secp256k1
        let message = Message::from_digest_slice(&message_hash)
            .expect("Message could not be created from hash");

        // Sign the message
        let recoverable_sig = secp.sign_ecdsa_recoverable(&message, &secret_key);

        // Serialize the signature to compact format
        let (recid, sig) = recoverable_sig.serialize_compact();

        // Convert signature and recovery ID to appropriate formats
        let r = hex::encode(&sig[0..32]); // r component
        let s = hex::encode(&sig[32..64]); // s component
        let v = recid.to_i32() + 27; // recovery ID, adjusted for Ethereum (v = 27 or 28)

        (r, s, v)
    }

    /// Recover the signer's public key from a recoverable signature over `Keccak256(data)`.
    pub fn recover_public_key(data: &[u8], r: &str, s: &str, v: i32) -> Result<PublicKey, String> {
        let secp = Secp256k1::new();
        let mut hasher = Keccak256::new();
        hasher.update(data);
        let message_hash = hasher.finalize();
        let message = Message::from_digest_slice(&message_hash)
            .map_err(|_| "Message could not be created from hash".to_string())?;

        let sig_r = Vec::from_hex(Self::strip_hex_prefix(r)).map_err(|_| "Invalid hex in r".to_string())?;
        let sig_s = Vec::from_hex(Self::strip_hex_prefix(s)).map_err(|_| "Invalid hex in s".to_string())?;
        if sig_r.len() != 32 || sig_s.len() != 32 {
            return Err("r and s must each be 32 bytes of hex".to_string());
        }
        let signature_data = [&sig_r[..], &sig_s[..]].concat();
        let recovery_id =
            RecoveryId::from_i32(v - 27).map_err(|_| "Invalid recovery ID".to_string())?;
        let recoverable_sig = RecoverableSignature::from_compact(&signature_data, recovery_id)
            .map_err(|_| "Valid signature could not be created".to_string())?;

        secp.recover_ecdsa(&message, &recoverable_sig)
            .map_err(|_| "Public key could not be recovered".to_string())
    }

    pub fn verify(
        derive_address: &str,
        data: &[u8],
        r: &str,
        s: &str,
        v: i32,
    ) -> Result<bool, String> {
        let recovered_public_key = Self::recover_public_key(data, r, s, v)?;
        let derived_address = Self::derive_address(&recovered_public_key);
        Ok(derived_address == derive_address)
    }

    /// Verify that the recoverable signature `(r, s, v)` over `Keccak256(data)` was produced by
    /// the holder of `claimed_key`. `claimed_key` may be a 40-char address or a 130-char
    /// uncompressed public key, with or without `0x`, any case (same shapes as
    /// `validate_public_key`).
    pub fn verify_key_ownership(
        claimed_key: &str,
        data: &[u8],
        r: &str,
        s: &str,
        v: i32,
    ) -> Result<bool, String> {
        let recovered_public_key = Self::recover_public_key(data, r, s, v)?;
        let cleaned_key = Self::strip_hex_prefix(claimed_key).to_lowercase();
        match cleaned_key.len() {
            130 => Ok(hex::encode(recovered_public_key.serialize_uncompressed()) == cleaned_key),
            40 => {
                let derived_address = Self::derive_address(&recovered_public_key);
                Ok(Self::strip_hex_prefix(&derived_address) == cleaned_key)
            }
            _ => Err(
                "public key must be a 40-char address or 130-char uncompressed public key"
                    .to_string(),
            ),
        }
    }

    pub fn validate_public_key(public_key: &str) -> Result<(), String> {
        let cleaned_key = Self::strip_hex_prefix(public_key);
        
        // Ethereum addresses are 40 characters (20 bytes)
        // Uncompressed secp256k1 public keys are 130 characters (65 bytes)
        if cleaned_key.len() != 40 && cleaned_key.len() != 130 {
            return Err(format!("Invalid public key length. Expected 40 or 130 characters, got {} for key: {}", cleaned_key.len(), public_key));
        }

        // Validate hex format
        Vec::from_hex(cleaned_key)
            .map_err(|_| format!("Invalid hex format for public key: {}", public_key))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tracing::{error, info};

    use super::*;

    #[test]
    fn test_generate_new_keypair() {
        let keys = SignatureKeys::generate_new_keypair();
        info!(
            "{:?},{:?},{:?}",
            keys.address_key, keys.secret_key, keys.public_key
        )
    }

    #[test]
    fn test_sign_and_verify() {
        let keys = SignatureKeys::generate_new_keypair();
        let data = b"Blockchain technology";
        info!("Public key: {:?}", keys.public_key);
        info!("Address: {:?}", keys.address_key);
        info!("Secret key: {:?}", keys.secret_key);

        // Test signing
        let (r, s, v) = SignatureKeys::sign(&keys.secret_key, data);
        info!("Signature: r={:?}, s={:?}, v={:?}", r, s, v);

        match SignatureKeys::verify(&keys.address_key, data, &r, &s, v) {
            Ok(is_verified) => assert!(is_verified, "Signature verification should succeed"),
            Err(e) => error!("Signature verification failed with error: {}", e),
        }
    }

    #[test]
    fn test_sign_and_verify_failure_on_modified_data() {
        let keys = SignatureKeys::generate_new_keypair();
        let original_data = b"Blockchain technology";
        let modified_data = b"Altered data";

        // Test signing with the original data
        let (r, s, v) = SignatureKeys::sign(&keys.secret_key, original_data);

        // Attempt to verify signature against modified data
        match SignatureKeys::verify(&keys.address_key, modified_data, &r, &s, v) {
            Ok(is_verified) => assert!(
                !is_verified,
                "Signature verification should fail on modified data"
            ),
            Err(_) => assert!(true, "Expected verification failure on modified data"),
        }
    }

    #[test]
    fn test_sign_and_verify_failure_on_wrong_key() {
        let keys = SignatureKeys::generate_new_keypair();
        let other_keys = SignatureKeys::generate_new_keypair(); // Generate a different key pair
        let data = b"Blockchain technology";

        // Test signing with the first key
        let (r, s, v) = SignatureKeys::sign(&keys.secret_key, data);

        // Attempt to verify signature with a different public key
        match SignatureKeys::verify(&other_keys.address_key, data, &r, &s, v) {
            Ok(is_verified) => assert!(
                !is_verified,
                "Signature verification should fail with a different public key"
            ),
            Err(_) => assert!(
                true,
                "Expected verification failure with a different public key"
            ),
        }
    }

    #[test]
    fn test_validate_public_key() {
        // Test with Ethereum address format (40 chars)
        let eth_address = "deb4cfb63db134698e1879ea24904df074726cc0";
        let result = SignatureKeys::validate_public_key(eth_address);
        assert!(result.is_ok(), "Valid Ethereum address should be accepted");

        // Test with 0x prefixed Ethereum address
        let prefixed_eth_address = "0xdeb4cfb63db134698e1879ea24904df074726cc0";
        let result = SignatureKeys::validate_public_key(prefixed_eth_address);
        assert!(result.is_ok(), "0x prefixed Ethereum address should be accepted");

        // Generate a valid secp256k1 key pair to test with real public key
        let keys = SignatureKeys::generate_new_keypair();
        let result = SignatureKeys::validate_public_key(&keys.public_key);
        assert!(result.is_ok(), "Valid secp256k1 public key should be accepted");

        // Test with invalid hex string
        let invalid_hex = "not_a_valid_hex_string";
        let result = SignatureKeys::validate_public_key(invalid_hex);
        assert!(result.is_err(), "Invalid hex string should be rejected");

        // Test with wrong length
        let wrong_length = "abc123";
        let result = SignatureKeys::validate_public_key(wrong_length);
        assert!(result.is_err(), "Wrong length key should be rejected");
    }
}
