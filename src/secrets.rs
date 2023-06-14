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

pub async fn fetch_secrets(url: &str) -> std::result::Result<Secrets, Err> {
    let mut os_rng = OsRng::default();
    let priv_key = RsaPrivateKey::new(&mut os_rng, 2048)
        .map_err(|_| Err::KeygenError)?;
    let pub_key = RsaPublicKey::from(&priv_key).to_public_key_der()
        .map_err(|_| Err::KeyParseError)?;
    let pub_key: &[u8] = pub_key.as_ref();
    let secrets_quote = Sgx::gramine_generate_quote(pub_key)
        .map_err(|_| Err::SgxError)?;
    let client = reqwest::Client::new();
    let res = client.post(url)
        .json(&json!({
            "quote": &secrets_quote,
            "pubkey": pub_key,
        }))
        .send()
        .await
        .unwrap();
    println!("{}", res.status().as_u16());
    let ciphertext = res.bytes().await.map_err(|_| Err::FetchError)?;
    let padding = PaddingScheme::new_pkcs1v15_encrypt();
    let secrets: Secrets = serde_json::from_slice(&priv_key.decrypt(padding, &ciphertext)
        .map_err(|_| Err::DecryptError)?)
        .map_err(|_| Err::DecryptError)?;
    Ok(secrets)
}
