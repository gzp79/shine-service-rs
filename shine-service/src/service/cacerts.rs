use rustls::{Certificate, RootCertStore};
use rustls_native_certs::load_native_certs;
use std::io;

pub fn get_root_cert_store() -> Result<RootCertStore, io::Error> {
    let mut store = RootCertStore::empty();
    let certs = load_native_certs()?;
    for cert in certs {
        store.add(&Certificate(cert.0)).unwrap();
    }
    Ok(store)
}
