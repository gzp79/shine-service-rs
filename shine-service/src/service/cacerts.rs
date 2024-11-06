use rustls::RootCertStore;
use rustls_native_certs::{load_native_certs, Error};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
#[error("Failed to load native certs: {0:?}")]
pub struct CertError(Vec<Error>);

pub fn get_root_cert_store() -> Result<RootCertStore, CertError> {
    let mut store = RootCertStore::empty();
    let certs_result = load_native_certs();
    if !certs_result.errors.is_empty() {
        return Err(CertError(certs_result.errors));
    } else {
        store.add_parsable_certificates(certs_result.certs);
        Ok(store)
    }
}
