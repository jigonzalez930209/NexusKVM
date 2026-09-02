use std::io;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio_rustls::rustls::{self, Certificate, ClientConfig, PrivateKey, RootCertStore};
use tokio_rustls::TlsConnector;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("No client private key")]
    NoClientKey,
}

fn load_keys(pem: &[u8]) -> Vec<PrivateKey> {
    let mut buf = pem;
    let mut keys = Vec::new();
    while let Ok(Some(item)) = rustls_pemfile::read_one(&mut buf) {
        match item {
            rustls_pemfile::Item::RSAKey(data)
            | rustls_pemfile::Item::PKCS8Key(data)
            | rustls_pemfile::Item::ECKey(data) => keys.push(PrivateKey(data)),
            _ => {}
        }
    }
    keys
}

pub async fn configure(
    certificate: &Path,
    client_certificate: Option<&Path>,
    client_key: Option<&Path>,
) -> Result<TlsConnector, Error> {
    let certificate = fs::read(certificate).await?;
    let certificates = rustls_pemfile::certs(&mut certificate.as_slice())?;

    let mut store = RootCertStore::empty();
    for certificate in certificates {
        store.add(&Certificate(certificate))?;
    }

    let builder = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(store);

    let config = match (client_certificate, client_key) {
        (Some(cert_path), Some(key_path)) => {
            let cert_pem = fs::read(cert_path).await?;
            let key_pem = fs::read(key_path).await?;
            let chain = rustls_pemfile::certs(&mut cert_pem.as_slice())?
                .into_iter()
                .map(Certificate)
                .collect();
            let key = load_keys(&key_pem)
                .into_iter()
                .next()
                .ok_or(Error::NoClientKey)?;
            Arc::new(builder.with_client_auth_cert(chain, key)?)
        }
        _ => Arc::new(builder.with_no_client_auth()),
    };

    Ok(config.into())
}
