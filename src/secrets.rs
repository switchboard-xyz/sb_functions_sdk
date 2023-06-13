use crate::*;
use rsa::{RsaPublicKey, RsaPrivateKey};
use rsa::pkcs8::ToPublicKey;
use rand::rngs::OsRng;
use serde_json::{json};
use std::collections::HashMap;
use rsa::PaddingScheme;
use serde_json;
use serde::Deserialize;

#[allow(dead_code)]
#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Secrets {
    mrEnclaves: Vec<String>,
    permittedAdvisories: Vec<String>,
    keys: HashMap<String, String>,
}

pub async fn fetch_secrets(url: &str) -> Secrets {
    let mut os_rng = OsRng::default();
    let priv_key = RsaPrivateKey::new(&mut os_rng, 2048).unwrap();
    let pub_key = RsaPublicKey::from(&priv_key).to_public_key_der().unwrap();
    let pub_key: &[u8] = pub_key.as_ref();
    let secrets_quote = Sgx::gramine_generate_quote(pub_key).unwrap();
    let client = reqwest::Client::new();
    let res = client.post(url)
        .json(&json!({
            "quote": &secrets_quote,
            "pubkey": pub_key,
        }))
        .send()
        .await
        .unwrap();
    let ciphertext = res.bytes().await.unwrap();
    let padding = PaddingScheme::new_pkcs1v15_encrypt();
    let secrets: Secrets = serde_json::from_slice(&priv_key.decrypt(padding, &ciphertext).unwrap()).unwrap();
    secrets
}
