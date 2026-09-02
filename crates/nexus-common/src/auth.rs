use chacha20poly1305::{
    aead::{Aead, KeyInit as AeadKeyInit, Payload},
    ChaCha20Poly1305, Key, Nonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub const AEAD_NONCE_LEN: usize = 12;
pub const AEAD_TS_SKEW_SECS: u64 = 90;
const REPLAY_CAP: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeadEnvelope {
    pub n: String,
    pub t: u64,
    pub c: String,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn secret_ok(secret: &str) -> bool {
    !secret.is_empty()
}

pub fn derive_aead_key(secret: &str) -> Result<[u8; 32], String> {
    if secret.is_empty() {
        return Err("empty peer secret".into());
    }
    let hk = Hkdf::<Sha256>::new(Some(b"nexuskvm-peer-v1"), secret.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(b"aead", &mut key)
        .map_err(|_| "hkdf expand".to_string())?;
    Ok(key)
}

fn aead_crypt(
    secret: &str,
    nonce: &[u8; AEAD_NONCE_LEN],
    aad: &[u8],
    msg: &[u8],
    encrypt: bool,
) -> Result<Vec<u8>, String> {
    let key = derive_aead_key(secret)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let n = Nonce::from_slice(nonce);
    let payload = Payload { msg, aad };
    if encrypt {
        cipher
            .encrypt(n, payload)
            .map_err(|_| "aead encrypt".into())
    } else {
        cipher
            .decrypt(n, payload)
            .map_err(|_| "aead decrypt".into())
    }
}

pub fn seal(secret: &str, plaintext: &[u8], ts: u64) -> Result<AeadEnvelope, String> {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let aad = ts.to_le_bytes();
    let ct = aead_crypt(secret, &nonce, &aad, plaintext, true)?;
    Ok(AeadEnvelope {
        n: hex::encode(nonce),
        t: ts,
        c: hex::encode(ct),
    })
}

pub fn open(secret: &str, env: &AeadEnvelope) -> Result<Vec<u8>, String> {
    if env.t.abs_diff(now_unix()) > AEAD_TS_SKEW_SECS {
        return Err("expired peer control message".into());
    }
    let nonce = decode_nonce(&env.n)?;
    let ct = hex::decode(&env.c).map_err(|_| "bad ciphertext".to_string())?;
    let aad = env.t.to_le_bytes();
    aead_crypt(secret, &nonce, &aad, &ct, false)
}

pub fn seal_chunk(secret: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = aead_crypt(secret, &nonce, b"clip", plaintext, true)?;
    let mut out = Vec::with_capacity(AEAD_NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn open_chunk(secret: &str, frame: &[u8]) -> Result<Vec<u8>, String> {
    if frame.len() < AEAD_NONCE_LEN + 16 {
        return Err("clip frame too small".into());
    }
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    nonce.copy_from_slice(&frame[..AEAD_NONCE_LEN]);
    aead_crypt(secret, &nonce, b"clip", &frame[AEAD_NONCE_LEN..], false)
}

fn decode_nonce(hex_n: &str) -> Result<[u8; AEAD_NONCE_LEN], String> {
    let raw = hex::decode(hex_n).map_err(|_| "bad nonce".to_string())?;
    if raw.len() != AEAD_NONCE_LEN {
        return Err("bad nonce length".into());
    }
    let mut n = [0u8; AEAD_NONCE_LEN];
    n.copy_from_slice(&raw);
    Ok(n)
}

#[derive(Debug, Default)]
pub struct ReplayGuard {
    seen: HashSet<[u8; AEAD_NONCE_LEN]>,
    order: VecDeque<[u8; AEAD_NONCE_LEN]>,
}

impl ReplayGuard {
    pub fn check(&mut self, env: &AeadEnvelope) -> Result<(), String> {
        let nonce = decode_nonce(&env.n)?;
        if self.seen.contains(&nonce) {
            return Err("replayed peer control message".into());
        }
        if self.order.len() >= REPLAY_CAP {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        self.seen.insert(nonce);
        self.order.push_back(nonce);
        Ok(())
    }
}

pub fn mac_hex(secret: &str, body: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn mac_verify(secret: &str, body: &str, presented: &str) -> bool {
    let Ok(bytes) = hex::decode(presented) else {
        return false;
    };
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(body.as_bytes());
    mac.verify_slice(&bytes).is_ok()
}

pub fn token_eq(expected: &str, presented: Option<&str>) -> bool {
    let Some(presented) = presented else {
        return false;
    };
    if expected.len() != presented.len() {
        return false;
    }
    expected.as_bytes().ct_eq(presented.as_bytes()).unwrap_u8() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_roundtrip() {
        let m = mac_hex("pw", "switch_local");
        assert!(mac_verify("pw", "switch_local", &m));
        assert!(!mac_verify("pw", "switch_local", "00"));
        assert!(!mac_verify("other", "switch_local", &m));
    }

    #[test]
    fn aead_roundtrip_and_replay() {
        let env = seal("secret", b"hello", now_unix()).unwrap();
        assert_eq!(open("secret", &env).unwrap(), b"hello");
        assert!(open("wrong", &env).is_err());
        let mut g = ReplayGuard::default();
        g.check(&env).unwrap();
        assert!(g.check(&env).is_err());
    }

    #[test]
    fn empty_secret_rejected() {
        assert!(derive_aead_key("").is_err());
        assert!(seal("", b"x", now_unix()).is_err());
    }

    #[test]
    fn token_constant_time() {
        assert!(token_eq("abc", Some("abc")));
        assert!(!token_eq("abc", Some("abd")));
        assert!(!token_eq("abc", None));
        assert!(!token_eq("abc", Some("ab")));
    }
}
