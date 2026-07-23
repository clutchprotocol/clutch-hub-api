//! In-memory cooldown limiter for the testnet faucet.
//! Keyed by client IP (anti-spam) and recipient address (anti-refund drain).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct FaucetRateLimiter {
    // ponytail: one map, "ip:"/"addr:" prefixed keys; pruned to the per-address
    // horizon on each call so it stays bounded — fine for testnet volume. LRU if it grows.
    hits: Mutex<HashMap<String, Instant>>,
    per_ip: Duration,
    per_address: Duration,
}

impl FaucetRateLimiter {
    // ponytail: fixed cooldowns; move to AppConfig if operators need to tune them.
    pub fn new() -> Self {
        Self::with_windows(Duration::from_secs(30), Duration::from_secs(3600))
    }

    pub fn with_windows(per_ip: Duration, per_address: Duration) -> Self {
        Self {
            hits: Mutex::new(HashMap::new()),
            per_ip,
            per_address,
        }
    }

    /// Record a hit for (ip, address). Returns `Err(retry_after_secs)` if either the IP
    /// or the address is still cooling down; on rejection the hit is NOT recorded.
    pub fn check(&self, ip: &str, address: &str) -> Result<(), u64> {
        let now = Instant::now();
        // Recover a poisoned lock rather than wedging the faucet forever.
        let mut hits = self.hits.lock().unwrap_or_else(|p| p.into_inner());

        // Drop entries older than the longest window so the map stays bounded.
        hits.retain(|_, t| now.saturating_duration_since(*t) < self.per_address);

        let ip_key = format!("ip:{}", ip);
        let addr_key = format!("addr:{}", address.to_lowercase());

        if let Some(retry) = Self::remaining(&hits, &ip_key, self.per_ip, now) {
            return Err(retry);
        }
        if let Some(retry) = Self::remaining(&hits, &addr_key, self.per_address, now) {
            return Err(retry);
        }

        hits.insert(ip_key, now);
        hits.insert(addr_key, now);
        Ok(())
    }

    fn remaining(
        hits: &HashMap<String, Instant>,
        key: &str,
        window: Duration,
        now: Instant,
    ) -> Option<u64> {
        let last = hits.get(key)?;
        let elapsed = now.saturating_duration_since(*last);
        if elapsed < window {
            Some((window - elapsed).as_secs() + 1)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn blocks_rapid_repeat_then_allows_after_window() {
        let rl = FaucetRateLimiter::with_windows(
            Duration::from_millis(40),
            Duration::from_millis(40),
        );
        // First hit allowed.
        assert!(rl.check("1.2.3.4", "0xabc").is_ok());
        // Immediate repeat from the same IP is blocked, even for a different address.
        assert!(rl.check("1.2.3.4", "0xother").is_err());
        // Same address from a different IP is blocked by the address window.
        assert!(rl.check("9.9.9.9", "0xabc").is_err());
        // After the window elapses, allowed again.
        sleep(Duration::from_millis(60));
        assert!(rl.check("1.2.3.4", "0xabc").is_ok());
    }

    #[test]
    fn address_key_is_case_insensitive() {
        let rl =
            FaucetRateLimiter::with_windows(Duration::from_secs(10), Duration::from_secs(10));
        assert!(rl.check("1.1.1.1", "0xABC").is_ok());
        // Different IP, same address in different case — still blocked.
        assert!(rl.check("2.2.2.2", "0xabc").is_err());
    }
}
