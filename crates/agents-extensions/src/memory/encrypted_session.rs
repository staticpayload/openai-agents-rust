use std::time::{SystemTime, UNIX_EPOCH};

use agents_core::{AgentsError, InputItem, Result, Session, SessionSettings};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Envelope stored in the underlying session for encrypted items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub nonce: u64,
    pub ciphertext_hex: String,
    pub created_at_ms: u64,
}

/// Transparent encrypted wrapper for any session implementation.
#[derive(Clone, Debug)]
pub struct EncryptedSession<S> {
    pub inner: S,
    pub encryption_key: String,
    pub ttl_seconds: Option<u64>,
}

impl<S> EncryptedSession<S> {
    pub fn new(inner: S, encryption_key: impl Into<String>) -> Self {
        Self {
            inner,
            encryption_key: encryption_key.into(),
            ttl_seconds: None,
        }
    }

    pub fn with_ttl_seconds(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }
}

#[async_trait]
impl<S> Session for EncryptedSession<S>
where
    S: Session + Send + Sync,
{
    fn session_id(&self) -> &str {
        self.inner.session_id()
    }

    fn session_settings(&self) -> Option<&SessionSettings> {
        self.inner.session_settings()
    }

    async fn get_items_with_limit(&self, limit: Option<usize>) -> Result<Vec<InputItem>> {
        let items = self.inner.get_items_with_limit(limit).await?;
        let mut decrypted = Vec::new();
        for item in items {
            if let Some(value) = self.try_decrypt_item(item)? {
                decrypted.push(value);
            }
        }
        Ok(decrypted)
    }

    async fn add_items(&self, items: Vec<InputItem>) -> Result<()> {
        let encrypted = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| self.encrypt_item(item, index as u64))
            .collect::<Result<Vec<_>>>()?;
        self.inner.add_items(encrypted).await
    }

    async fn pop_item(&self) -> Result<Option<InputItem>> {
        loop {
            let Some(item) = self.inner.pop_item().await? else {
                return Ok(None);
            };
            if let Some(value) = self.try_decrypt_item(item)? {
                return Ok(Some(value));
            }
        }
    }

    async fn clear_session(&self) -> Result<()> {
        self.inner.clear_session().await
    }
}

impl<S> EncryptedSession<S>
where
    S: Session + Send + Sync,
{
    fn encrypt_item(&self, item: InputItem, nonce: u64) -> Result<InputItem> {
        let plaintext =
            serde_json::to_vec(&item).map_err(|error| AgentsError::message(error.to_string()))?;
        let keystream = derive_keystream(
            &self.encryption_key,
            self.session_id(),
            nonce,
            plaintext.len(),
        );
        let ciphertext = plaintext
            .iter()
            .zip(keystream.iter())
            .map(|(lhs, rhs)| lhs ^ rhs)
            .collect::<Vec<_>>();
        let envelope = EncryptedEnvelope {
            nonce,
            ciphertext_hex: encode_hex(&ciphertext),
            created_at_ms: now_ms(),
        };
        Ok(InputItem::Json {
            value: serde_json::to_value(envelope)
                .map_err(|error| AgentsError::message(error.to_string()))?,
        })
    }

    fn try_decrypt_item(&self, item: InputItem) -> Result<Option<InputItem>> {
        let InputItem::Json { value } = item else {
            return Ok(Some(item));
        };
        let envelope: EncryptedEnvelope = match serde_json::from_value(value.clone()) {
            Ok(envelope) => envelope,
            Err(_) => {
                return Ok(Some(InputItem::Json { value }));
            }
        };
        if let Some(ttl_seconds) = self.ttl_seconds {
            let age_ms = now_ms().saturating_sub(envelope.created_at_ms);
            if age_ms > ttl_seconds.saturating_mul(1_000) {
                return Ok(None);
            }
        }

        let ciphertext = decode_hex(&envelope.ciphertext_hex)?;
        let keystream = derive_keystream(
            &self.encryption_key,
            self.session_id(),
            envelope.nonce,
            ciphertext.len(),
        );
        let plaintext = ciphertext
            .iter()
            .zip(keystream.iter())
            .map(|(lhs, rhs)| lhs ^ rhs)
            .collect::<Vec<_>>();
        let item = serde_json::from_slice::<InputItem>(&plaintext)
            .map_err(|error| AgentsError::message(error.to_string()))?;
        Ok(Some(item))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn derive_keystream(secret: &str, session_id: &str, nonce: u64, len: usize) -> Vec<u8> {
    let mut stream = Vec::with_capacity(len);
    let mut counter = 0u64;
    while stream.len() < len {
        let block =
            fnv1a64(format!("{secret}:{session_id}:{nonce}:{counter}").as_bytes()).to_le_bytes();
        stream.extend(block);
        counter = counter.wrapping_add(1);
    }
    stream.truncate(len);
    stream
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(AgentsError::message("invalid hex payload"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars = value.as_bytes();
    for index in (0..chars.len()).step_by(2) {
        let high = decode_hex_nibble(chars[index])?;
        let low = decode_hex_nibble(chars[index + 1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn decode_hex_nibble(value: u8) -> Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(AgentsError::message("invalid hex digit")),
    }
}

#[cfg(test)]
mod tests {
    use agents_core::MemorySession;

    use super::*;

    #[tokio::test]
    async fn round_trips_items_through_envelope() {
        let session = EncryptedSession::new(MemorySession::new("session"), "secret");
        session
            .add_items(vec![InputItem::from("hello")])
            .await
            .expect("encrypted item should save");

        let items = session.get_items().await.expect("items should load");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].as_text(), Some("hello"));
    }
}
