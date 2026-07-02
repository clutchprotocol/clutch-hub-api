use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hub::signature_keys::SignatureKeys;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub pk: String, // public key
    pub exp: usize, // expiration time
}

/// Prefix of the canonical proof-of-key-ownership message signed by clients for `generateToken`.
pub const AUTH_CHALLENGE_PREFIX: &str = "clutch-auth";

/// Maximum allowed clock skew (seconds) between the client challenge timestamp and server time.
pub const AUTH_TIMESTAMP_WINDOW_SECS: i64 = 120;

/// Canonical auth challenge message. Must match clutch-hub-sdk-js `buildAuthChallengeMessage`
/// byte-for-byte: `clutch-auth:{publicKey}:{timestamp}` where `publicKey` is the exact string
/// the client sends as the mutation argument and `timestamp` is decimal unix seconds.
pub fn build_auth_challenge_message(public_key: &str, timestamp: i64) -> String {
    format!("{}:{}:{}", AUTH_CHALLENGE_PREFIX, public_key, timestamp)
}

/// Keccak-256 of the canonical auth message, as 64-char lowercase hex **without** `0x`.
/// Like transaction hashes, the secp256k1 signature is computed over
/// `Keccak256(hash_hex.as_utf8_bytes())` — i.e. over the UTF-8 bytes of this hex string —
/// matching the SDK's `signHash` / the node's verification convention.
pub fn auth_challenge_hash_hex(public_key: &str, timestamp: i64) -> String {
    let message = build_auth_challenge_message(public_key, timestamp);
    let mut hasher = Keccak256::new();
    hasher.update(message.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify proof of key ownership for `generateToken`.
///
/// Rejects when:
/// - `public_key` is not a 40-char address / 130-char uncompressed public key,
/// - `timestamp` deviates more than [`AUTH_TIMESTAMP_WINDOW_SECS`] from `now_secs`, or
/// - the recoverable signature `(r, s, v)` over the challenge does not recover to `public_key`.
pub fn verify_auth_challenge(
    public_key: &str,
    timestamp: i64,
    r: &str,
    s: &str,
    v: i32,
    now_secs: i64,
) -> Result<(), String> {
    SignatureKeys::validate_public_key(public_key)?;

    if (now_secs - timestamp).abs() > AUTH_TIMESTAMP_WINDOW_SECS {
        return Err(format!(
            "challenge timestamp is outside the allowed ±{}s window",
            AUTH_TIMESTAMP_WINDOW_SECS
        ));
    }

    let hash_hex = auth_challenge_hash_hex(public_key, timestamp);
    match SignatureKeys::verify_key_ownership(public_key, hash_hex.as_bytes(), r, s, v) {
        Ok(true) => Ok(()),
        Ok(false) => Err("signature does not match the provided public key".to_string()),
        Err(e) => Err(format!("invalid challenge signature: {}", e)),
    }
}

pub fn generate_jwt_token(
    public_key: &str,
    expiration_hours: u64,
    jwt_secret: &str,
) -> Result<(String, usize), String> {
    SignatureKeys::validate_public_key(public_key)?;

    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
        + (expiration_hours * 3600) as usize;

    let claims = Claims {
        pk: public_key.to_string(),
        exp: expiration,
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| e.to_string())?;

    Ok((token, expiration))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_751_500_000;

    /// Sign the auth challenge exactly like the SDK does (recoverable secp256k1 over
    /// Keccak256 of the challenge-hash hex string's UTF-8 bytes).
    fn sign_challenge(secret_key: &str, public_key: &str, timestamp: i64) -> (String, String, i32) {
        let hash_hex = auth_challenge_hash_hex(public_key, timestamp);
        SignatureKeys::sign(secret_key, hash_hex.as_bytes())
    }

    #[test]
    fn valid_challenge_with_address_passes() {
        let keys = SignatureKeys::generate_new_keypair();
        let (r, s, v) = sign_challenge(&keys.secret_key, &keys.address_key, NOW);
        assert!(verify_auth_challenge(&keys.address_key, NOW, &r, &s, v, NOW).is_ok());
    }

    #[test]
    fn valid_challenge_with_uncompressed_public_key_passes() {
        let keys = SignatureKeys::generate_new_keypair();
        let (r, s, v) = sign_challenge(&keys.secret_key, &keys.public_key, NOW);
        assert!(verify_auth_challenge(&keys.public_key, NOW, &r, &s, v, NOW).is_ok());
    }

    #[test]
    fn valid_challenge_with_prefixed_mixed_case_address_passes() {
        let keys = SignatureKeys::generate_new_keypair();
        // The message must be built from the exact string the client sends, so sign and
        // verify with the same (0x-prefixed, upper-cased) representation.
        let shouty = format!("0x{}", keys.address_key.trim_start_matches("0x").to_uppercase());
        let (r, s, v) = sign_challenge(&keys.secret_key, &shouty, NOW);
        assert!(verify_auth_challenge(&shouty, NOW, &r, &s, v, NOW).is_ok());
    }

    #[test]
    fn timestamp_within_window_passes() {
        let keys = SignatureKeys::generate_new_keypair();
        for skew in [-AUTH_TIMESTAMP_WINDOW_SECS, -60, 0, 60, AUTH_TIMESTAMP_WINDOW_SECS] {
            let ts = NOW + skew;
            let (r, s, v) = sign_challenge(&keys.secret_key, &keys.address_key, ts);
            assert!(
                verify_auth_challenge(&keys.address_key, ts, &r, &s, v, NOW).is_ok(),
                "skew {}s should be accepted",
                skew
            );
        }
    }

    #[test]
    fn expired_or_future_timestamp_fails() {
        let keys = SignatureKeys::generate_new_keypair();
        for skew in [-(AUTH_TIMESTAMP_WINDOW_SECS + 1), AUTH_TIMESTAMP_WINDOW_SECS + 1, -3600, 3600] {
            let ts = NOW + skew;
            let (r, s, v) = sign_challenge(&keys.secret_key, &keys.address_key, ts);
            let err = verify_auth_challenge(&keys.address_key, ts, &r, &s, v, NOW).unwrap_err();
            assert!(err.contains("window"), "skew {}s should be rejected: {}", skew, err);
        }
    }

    #[test]
    fn signature_from_wrong_key_fails() {
        let keys = SignatureKeys::generate_new_keypair();
        let attacker = SignatureKeys::generate_new_keypair();
        // Attacker signs the victim's challenge with their own key.
        let (r, s, v) = sign_challenge(&attacker.secret_key, &keys.address_key, NOW);
        assert!(verify_auth_challenge(&keys.address_key, NOW, &r, &s, v, NOW).is_err());
    }

    #[test]
    fn tampered_message_fails() {
        let keys = SignatureKeys::generate_new_keypair();
        // Signed for NOW, but presented with a different (still in-window) timestamp.
        let (r, s, v) = sign_challenge(&keys.secret_key, &keys.address_key, NOW);
        assert!(verify_auth_challenge(&keys.address_key, NOW + 30, &r, &s, v, NOW).is_err());
        // Signed for one key, presented for another valid key.
        let other = SignatureKeys::generate_new_keypair();
        assert!(verify_auth_challenge(&other.address_key, NOW, &r, &s, v, NOW).is_err());
    }

    #[test]
    fn garbage_signature_fails() {
        let keys = SignatureKeys::generate_new_keypair();
        assert!(verify_auth_challenge(&keys.address_key, NOW, "zz", "zz", 27, NOW).is_err());
        assert!(verify_auth_challenge(&keys.address_key, NOW, "aa", "bb", 27, NOW).is_err());
        let (r, s, _) = sign_challenge(&keys.secret_key, &keys.address_key, NOW);
        assert!(verify_auth_challenge(&keys.address_key, NOW, &r, &s, 99, NOW).is_err());
    }

    #[test]
    fn invalid_public_key_fails() {
        let keys = SignatureKeys::generate_new_keypair();
        let (r, s, v) = sign_challenge(&keys.secret_key, &keys.address_key, NOW);
        assert!(verify_auth_challenge("0x0", NOW, &r, &s, v, NOW).is_err());
        assert!(verify_auth_challenge("not-hex", NOW, &r, &s, v, NOW).is_err());
    }

    /// Fixtures produced by the clutch-hub-sdk-js signing code path
    /// (`signAuthChallenge` in src/sdk.ts) for private key
    /// d2c446110cfcecbdf05b2be528e72483de5b6f7ef9c7856df2f81f48e9f2748f.
    /// Guards byte-for-byte cross-language agreement of the challenge scheme.
    #[test]
    fn sdk_generated_fixture_with_address_verifies() {
        let public_key = "0xdeb4cfb63db134698e1879ea24904df074726cc0";
        let timestamp: i64 = 1_751_500_000;
        assert_eq!(
            auth_challenge_hash_hex(public_key, timestamp),
            "1e5584f163a9b934206f02d125797d34e9489a4b1a02dfd54af00071b372dd76"
        );
        let r = "0x32eac0994b7e1468ed2241f395ef00091b3ec4888648cf1b7e1e29a94554cb3e";
        let s = "0x6330f99b06f292860dd11a9a76d7b8240352694e2cedb39da57ced9a9437c2a2";
        assert!(verify_auth_challenge(public_key, timestamp, r, s, 28, timestamp).is_ok());
        // Same signature must not validate a different timestamp or key.
        assert!(verify_auth_challenge(public_key, timestamp + 1, r, s, 28, timestamp).is_err());
    }

    #[test]
    fn sdk_generated_fixture_with_uncompressed_public_key_verifies() {
        let public_key = "0x04a5ddc16b93f7e744fbab3c025cf99a0ef00113c6727353a3dff406fb4d136a06d73244619adc980818931da1b053462ef5af5e121cb5616be45325edd9b0be15";
        let timestamp: i64 = 1_751_500_000;
        assert_eq!(
            auth_challenge_hash_hex(public_key, timestamp),
            "bbeff99f86081c4a22258485ccf3a2b1a71287a49f23499030e75793983df664"
        );
        let r = "0xeef42109858751b5a25b902c69c44815bf336b466970dad666006054cd170bb6";
        let s = "0x1d5b73dd1581a326c59257c3a9b43739810a8e312d169a15c17727b15afe4471";
        assert!(verify_auth_challenge(public_key, timestamp, r, s, 27, timestamp).is_ok());
    }

    #[test]
    fn challenge_message_format_is_stable() {
        assert_eq!(
            build_auth_challenge_message("0xdeb4cfb63db134698e1879ea24904df074726cc0", 1_751_500_000),
            "clutch-auth:0xdeb4cfb63db134698e1879ea24904df074726cc0:1751500000"
        );
    }
}
