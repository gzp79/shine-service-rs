use super::{IdEncoder, IdEncoderError};

pub struct PrefixedIdEncoder<E: IdEncoder>(String, E);

impl<E: IdEncoder> PrefixedIdEncoder<E> {
    pub fn new<S: ToString>(prefix: S, encoder: E) -> Self {
        Self(prefix.to_string(), encoder)
    }
}

impl<E: IdEncoder> IdEncoder for PrefixedIdEncoder<E> {
    fn obfuscate(&self, id: u64) -> Result<String, IdEncoderError> {
        Ok(format!("{}{}", self.0, self.1.obfuscate(id)?))
    }

    fn deobfuscate(&self, id: &str) -> Result<u64, IdEncoderError> {
        if let Some(id) = id.strip_prefix(&self.0) {
            self.1.deobfuscate(id)
        } else {
            Err(IdEncoderError::InvalidObfuscatedId(format!("Invalid prefix")))
        }
    }
}
