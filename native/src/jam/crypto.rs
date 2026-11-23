//! The cryptography of a listening session, the same as the page's (nostr-tools), so a native
//! copy and a Tauri copy sit in one session:
//!  - the room: its relay tag and key are made from the session code (src/jam/protocol.ts);
//!  - every message is encrypted with NIP-44 v2 (ChaCha20, HMAC-SHA256, keys by HKDF) under
//!    the room key, and a sealed note to one person under the key two people share (ECDH);
//!  - every event is signed (BIP-340 Schnorr on secp256k1) by this copy's own key, so who sent
//!    a message is known for certain.

use base64::Engine;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// the relay tag and the key of a session, both made from its code
pub fn room_secrets(code: &str) -> (String, [u8; 32]) {
    let tag = hex(&sha256(format!("evplayer-jam-tag:{code}").as_bytes()));
    (tag[..32].to_string(), sha256(format!("evplayer-jam-key:{code}").as_bytes()))
}

// ---- NIP-44 v2 ------------------------------------------------------------------------------

/// the padded length of a message of `len` bytes
pub fn padded_len(len: usize) -> usize {
    if len <= 32 {
        return 32;
    }
    let next = 1usize << (usize::BITS - (len - 1).leading_zeros());
    let chunk = if next <= 256 { 32 } else { next / 8 };
    chunk * ((len - 1) / chunk + 1)
}

fn message_keys(conversation: &[u8; 32], nonce: &[u8; 32]) -> ([u8; 32], [u8; 12], [u8; 32]) {
    let hk = hkdf::Hkdf::<Sha256>::from_prk(conversation).expect("a 32-byte key");
    let mut okm = [0u8; 76];
    hk.expand(nonce, &mut okm).expect("76 bytes");
    let mut key = [0u8; 32];
    let mut iv = [0u8; 12];
    let mut mac = [0u8; 32];
    key.copy_from_slice(&okm[..32]);
    iv.copy_from_slice(&okm[32..44]);
    mac.copy_from_slice(&okm[44..76]);
    (key, iv, mac)
}

/// encrypt `text` under a conversation key, with the given 32-byte nonce
pub fn encrypt_with(text: &str, conversation: &[u8; 32], nonce: &[u8; 32]) -> Result<String, String> {
    let plain = text.as_bytes();
    if plain.is_empty() || plain.len() > 65535 {
        return Err("message too long".into());
    }
    let mut padded = vec![0u8; 2 + padded_len(plain.len())];
    padded[..2].copy_from_slice(&(plain.len() as u16).to_be_bytes());
    padded[2..2 + plain.len()].copy_from_slice(plain);
    let (key, iv, mac_key) = message_keys(conversation, nonce);
    chacha20::ChaCha20::new(&key.into(), &iv.into()).apply_keystream(&mut padded);
    let mut mac = HmacSha256::new_from_slice(&mac_key).expect("any key length");
    mac.update(nonce);
    mac.update(&padded);
    let tag = mac.finalize().into_bytes();
    let mut out = Vec::with_capacity(1 + 32 + padded.len() + 32);
    out.push(2);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&padded);
    out.extend_from_slice(&tag);
    Ok(base64::engine::general_purpose::STANDARD.encode(out))
}

pub fn encrypt(text: &str, conversation: &[u8; 32]) -> Result<String, String> {
    encrypt_with(text, conversation, &rand::random::<[u8; 32]>())
}

pub fn decrypt(payload: &str, conversation: &[u8; 32]) -> Option<String> {
    let data = base64::engine::general_purpose::STANDARD.decode(payload).ok()?;
    if data.len() < 99 || data[0] != 2 {
        return None;
    }
    let nonce: [u8; 32] = data[1..33].try_into().ok()?;
    let (cipher, tag) = data[33..].split_at(data.len() - 33 - 32);
    let (key, iv, mac_key) = message_keys(conversation, &nonce);
    let mut mac = HmacSha256::new_from_slice(&mac_key).ok()?;
    mac.update(&nonce);
    mac.update(cipher);
    mac.verify_slice(tag).ok()?;
    let mut padded = cipher.to_vec();
    chacha20::ChaCha20::new(&key.into(), &iv.into()).apply_keystream(&mut padded);
    let len = u16::from_be_bytes([padded[0], padded[1]]) as usize;
    if len == 0 || 2 + len > padded.len() || padded.len() != 2 + padded_len(len) {
        return None;
    }
    String::from_utf8(padded[2..2 + len].to_vec()).ok()
}

// ---- keys and events ------------------------------------------------------------------------

/// this copy's key in a session (a fresh one each time the program starts a link)
pub struct Keys {
    signing: k256::schnorr::SigningKey,
    secret: k256::SecretKey,
    /// the public key, x only, hex: who this copy is in the session
    pub id: String,
}

impl Keys {
    pub fn generate() -> Self {
        loop {
            let bytes: [u8; 32] = rand::random();
            if let Some(k) = Self::from_bytes(&bytes) {
                return k;
            }
        }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        let signing = k256::schnorr::SigningKey::from_bytes(bytes).ok()?;
        let secret = k256::SecretKey::from_slice(bytes).ok()?;
        let id = hex(&signing.verifying_key().to_bytes());
        Some(Self { signing, secret, id })
    }

    /// the key two people share (NIP-44's conversation key with someone's public key)
    pub fn conversation(&self, other: &str) -> Option<[u8; 32]> {
        let x = unhex(other)?;
        let mut sec1 = vec![2u8];
        sec1.extend_from_slice(&x);
        let public = k256::PublicKey::from_sec1_bytes(&sec1).ok()?;
        let shared = k256::ecdh::diffie_hellman(self.secret.to_nonzero_scalar(), public.as_affine());
        let (prk, _) = hkdf::Hkdf::<Sha256>::extract(Some(b"nip44-v2"), shared.raw_secret_bytes());
        Some(prk.into())
    }

    /// a signed Nostr event
    pub fn event(&self, kind: u32, created_at: u64, tags: serde_json::Value, content: &str) -> serde_json::Value {
        let id = event_id(&self.id, created_at, kind, &tags, content);
        let aux: [u8; 32] = rand::random();
        let sig = self.signing.sign_raw(&id, &aux).map(|s| hex(&s.to_bytes())).unwrap_or_default();
        serde_json::json!({ "id": hex(&id), "pubkey": self.id, "created_at": created_at, "kind": kind, "tags": tags, "content": content, "sig": sig })
    }
}

/// an event's id: the hash of its serialized fields (NIP-01)
pub fn event_id(pubkey: &str, created_at: u64, kind: u32, tags: &serde_json::Value, content: &str) -> [u8; 32] {
    let text = serde_json::to_string(&serde_json::json!([0, pubkey, created_at, kind, tags, content])).unwrap_or_default();
    sha256(text.as_bytes())
}

/// the event is what its id says and was signed by its pubkey
pub fn verify(e: &serde_json::Value) -> bool {
    let (Some(id), Some(pubkey), Some(created), Some(kind), Some(tags), Some(content), Some(sig)) = (e["id"].as_str(), e["pubkey"].as_str(), e["created_at"].as_u64(), e["kind"].as_u64(), e.get("tags"), e["content"].as_str(), e["sig"].as_str()) else {
        return false;
    };
    let hash = event_id(pubkey, created, kind as u32, tags, content);
    if hex(&hash) != id {
        return false;
    }
    let (Some(pk), Some(sig)) = (unhex(pubkey), unhex(sig)) else { return false };
    let (Ok(key), Ok(sig)) = (k256::schnorr::VerifyingKey::from_bytes(&pk), k256::schnorr::Signature::try_from(sig.as_slice())) else { return false };
    key.verify_raw(&hash, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key32(last: u8) -> [u8; 32] {
        let mut k = [0u8; 32];
        k[31] = last;
        k
    }

    #[test]
    fn nip44_matches_the_specification_s_example() {
        // NIP-44's first test vector: sec1 = 1, sec2 = 2, nonce = 1, plaintext "a"
        let a = Keys::from_bytes(&key32(1)).unwrap();
        let b = Keys::from_bytes(&key32(2)).unwrap();
        let conv = a.conversation(&b.id).unwrap();
        assert_eq!(hex(&conv), "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d");
        assert_eq!(b.conversation(&a.id).unwrap(), conv);
        let payload = encrypt_with("a", &conv, &key32(1)).unwrap();
        // version 2, the nonce, then the spec's ciphertext and MAC
        let bytes = base64::engine::general_purpose::STANDARD.decode(&payload).unwrap();
        assert_eq!(bytes[0], 2);
        assert_eq!(&bytes[1..33], &key32(1));
        assert!(payload.ends_with("ee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb"));
        assert_eq!(decrypt(&payload, &conv).as_deref(), Some("a"));
    }

    #[test]
    fn padding_follows_the_specification() {
        for (len, padded) in [(1, 32), (32, 32), (33, 64), (37, 64), (64, 64), (65, 96), (100, 128), (256, 256), (257, 320), (1000, 1024), (65535, 65536)] {
            assert_eq!(padded_len(len), padded, "{len}");
        }
    }

    #[test]
    fn a_room_message_round_trips_and_a_wrong_key_reads_nothing() {
        let (tag, key) = room_secrets("JAM-7K9P-2QXZ");
        assert_eq!(tag.len(), 32);
        let text = r#"{"t":"chat","id":"1","text":"привет 🎧"}"#;
        let sealed = encrypt(text, &key).unwrap();
        assert_eq!(decrypt(&sealed, &key).as_deref(), Some(text));
        assert!(decrypt(&sealed, &room_secrets("JAM-AAAA-BBBB").1).is_none());
    }

    #[test]
    fn events_are_signed_and_checked() {
        let k = Keys::generate();
        let e = k.event(20001, 1_700_000_000, serde_json::json!([["t", "abc"]]), "hello");
        assert!(verify(&e));
        let mut forged = e.clone();
        forged["content"] = "changed".into();
        assert!(!verify(&forged));
    }
}
