use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;
use std::env;

type HmacSha256 = Hmac<Sha256>;

pub fn get_psk() -> String {
    env::var("C2_PSK").unwrap_or_else(|_| "educational-c2-psk-key".to_string())
}

pub fn generate_nonce() -> String {
    Uuid::new_v4().to_string()
}

pub fn verify_proof(nonce: &str, signature: &str) -> bool {
    let psk = get_psk();
    let mut mac = HmacSha256::new_from_slice(psk.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(nonce.as_bytes());
    let expected_bytes = mac.finalize().into_bytes();
    let expected_signature = hex::encode(expected_bytes);

    expected_signature == signature
}

pub fn compute_agent_token(agent_id: &str) -> String {
    let psk = get_psk();
    let mut mac = HmacSha256::new_from_slice(psk.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(agent_id.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_agent_token(agent_id: &str, token: &str) -> bool {
    compute_agent_token(agent_id) == token
}
