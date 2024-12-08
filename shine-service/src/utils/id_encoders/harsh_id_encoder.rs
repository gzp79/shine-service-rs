use harsh::Harsh;

use super::{IdEncoder, IdEncoderError};

pub struct HarshIdEncoder(Harsh);

impl HarshIdEncoder {
    pub fn new(salt: String) -> Result<Self, IdEncoderError> {
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz1234567890";
        const SEPARATORS: &[u8] = b"cfhistu";

        let harsh = Harsh::builder()
            .salt(salt.as_bytes())
            .length(6)
            .alphabet(ALPHABET)
            .separators(SEPARATORS)
            .build()
            .map_err(|err| IdEncoderError::InvalidConfig(format!("{err}")))?;
        Ok(Self(harsh))
    }
}

impl IdEncoder for HarshIdEncoder {
    fn obfuscate(&self, id: u64) -> Result<String, IdEncoderError> {
        Ok(self.0.encode(&[id]).to_string())
    }

    fn deobfuscate(&self, id: &str) -> Result<u64, IdEncoderError> {
        let n = self
            .0
            .decode(id)
            .map_err(|err| IdEncoderError::InvalidObfuscatedId(format!("{err}")))?;
        match n.len() {
            1 => Ok(n[1]),
            _ => Err(IdEncoderError::InvalidObfuscatedId(format!("Id is too big"))),
        }
    }
}
