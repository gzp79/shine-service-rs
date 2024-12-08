use crate::utils::Optimus;

use super::{IdEncoder, IdEncoderError};

pub struct OptimusIdEncoder(Optimus);

impl OptimusIdEncoder {
    pub fn new(prime: u64, random: u64) -> Self {
        Self(Optimus::new(prime, random))
    }
}

impl IdEncoder for OptimusIdEncoder {
    fn obfuscate(&self, id: u64) -> Result<String, IdEncoderError> {
        Ok(self.0.encode(id).to_string())
    }

    fn deobfuscate(&self, id: &str) -> Result<u64, IdEncoderError> {
        let n = id
            .parse::<u64>()
            .map_err(|err| IdEncoderError::InvalidObfuscatedId(format!("{}", err)))?;
        Ok(self.0.decode(n))
    }
}
