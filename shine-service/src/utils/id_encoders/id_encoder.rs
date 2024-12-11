use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum IdEncoderError {
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Invalid obfuscated id: {0}")]
    InvalidObfuscatedId(String),
}

/// Sequence number obfuscation
pub trait IdEncoder: 'static + Send + Sync {
    fn obfuscate(&self, id: u64) -> Result<String, IdEncoderError>;
    fn deobfuscate(&self, id: &str) -> Result<u64, IdEncoderError>;
}

impl IdEncoder for Box<dyn IdEncoder> {
    fn obfuscate(&self, id: u64) -> Result<String, IdEncoderError> {
        self.as_ref().obfuscate(id)
    }

    fn deobfuscate(&self, id: &str) -> Result<u64, IdEncoderError> {
        self.as_ref().deobfuscate(id)
    }
}

impl IdEncoder for Arc<dyn IdEncoder> {
    fn obfuscate(&self, id: u64) -> Result<String, IdEncoderError> {
        self.as_ref().obfuscate(id)
    }

    fn deobfuscate(&self, id: &str) -> Result<u64, IdEncoderError> {
        self.as_ref().deobfuscate(id)
    }
}
