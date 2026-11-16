//! Shared, immutable JSON packets. Serialization happens once at the outgoing boundary.
use kinexa_contracts::ServerMessage;
use std::sync::Arc;
#[derive(Clone)]
pub struct Encoded {
    pub sequence: u64,
    pub bytes: bytes::Bytes,
}
impl Encoded {
    pub fn new(message: &ServerMessage, sequence: u64) -> Arc<Self> {
        let bytes = match serde_json::to_vec(message) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!(%error,"Could not encode outgoing message");
                br#"{"type":"notice","level":"error","code":"serialization_failed","message":"A stream message could not be encoded."}"#.to_vec()
            }
        };
        Arc::new(Self {
            sequence,
            bytes: bytes.into(),
        })
    }
}
