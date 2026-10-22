use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TTL: u64 = 120;
pub const MIN_DIFFICULTY: u8 = 16;
pub const MAX_DIFFICULTY: u8 = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum InvalidProof {
    Invalid,
    Expired,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    pub version: u8,
    pub id: String,
    pub site: String,
    pub origin: String,
    pub client: String,
    pub difficulty: u8,
    pub issued_at: u64,
    pub expires_at: u64,
}

pub fn signing_key(value: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = BASE64_URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "Invalid signing key.")?;
    if bytes.len() != 32 || BASE64_URL_SAFE_NO_PAD.encode(&bytes) != value {
        return Err("Use a base64url-encoded 32-byte signing key without padding.");
    }
    Ok(bytes)
}

fn mac(key: &[u8], purpose: &str, data: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length.");
    mac.update(purpose.as_bytes());
    mac.update(&[0]);
    mac.update(data);
    mac
}

pub fn fingerprint(key: &[u8], site: &str, ip: &str, minute: u64) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(
        mac(
            key,
            "knock-client-v1",
            format!("{site}\n{ip}\n{minute}").as_bytes(),
        )
        .finalize()
        .into_bytes(),
    )
}

pub fn issue(key: &[u8], challenge: &Challenge) -> String {
    let payload = BASE64_URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(challenge).expect("Challenge serialization is infallible."));
    let signature = BASE64_URL_SAFE_NO_PAD.encode(
        mac(key, "knock-challenge-v1", payload.as_bytes())
            .finalize()
            .into_bytes(),
    );
    format!("{payload}.{signature}")
}

pub fn verify(
    key: &[u8],
    token: &str,
    nonce: &str,
    site: &str,
    origin: &str,
    ip: &str,
    now: u64,
) -> Result<Challenge, InvalidProof> {
    if token.len() > 2048 || nonce.len() > 10 || nonce.is_empty() {
        return Err(InvalidProof::Invalid);
    }
    let number: u32 = nonce.parse().map_err(|_| InvalidProof::Invalid)?;
    if number.to_string() != nonce {
        return Err(InvalidProof::Invalid);
    }
    let (payload, signature) = token.split_once('.').ok_or(InvalidProof::Invalid)?;
    let signature = BASE64_URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| InvalidProof::Invalid)?;
    mac(key, "knock-challenge-v1", payload.as_bytes())
        .verify_slice(&signature)
        .map_err(|_| InvalidProof::Invalid)?;
    let decoded = BASE64_URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| InvalidProof::Invalid)?;
    let challenge: Challenge =
        serde_json::from_slice(&decoded).map_err(|_| InvalidProof::Invalid)?;
    if challenge.version != 1
        || challenge.site != site
        || challenge.origin != origin
        || challenge.id.len() != 32
        || !challenge.id.bytes().all(|b| b.is_ascii_hexdigit())
        || !(MIN_DIFFICULTY..=MAX_DIFFICULTY).contains(&challenge.difficulty)
        || challenge.issued_at > now
        || challenge.issued_at.checked_add(TTL) != Some(challenge.expires_at)
        || challenge.client != fingerprint(key, site, ip, challenge.issued_at / 60)
    {
        return Err(InvalidProof::Invalid);
    }
    if challenge.expires_at <= now {
        return Err(InvalidProof::Expired);
    }
    if !valid_work(token, nonce, challenge.difficulty) {
        return Err(InvalidProof::Invalid);
    }
    Ok(challenge)
}

pub fn valid_work(token: &str, nonce: &str, difficulty: u8) -> bool {
    let hash = Sha256::digest(format!("{token}.{nonce}").as_bytes());
    let mut zeros = 0;
    for byte in hash {
        zeros += byte.leading_zeros();
        if byte != 0 {
            break;
        }
    }
    zeros >= u32::from(difficulty)
}
