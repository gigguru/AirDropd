//! Persistent self-signed TLS certificate for AirDrop HTTPS.

use anyhow::{Context, Result};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
use std::path::PathBuf;
use tokio_rustls::rustls::{Certificate as RustlsCert, PrivateKey as RustlsKey, ServerConfig};
use std::sync::Arc;

pub fn tls_dir() -> PathBuf {
    crate::config::config_path()
        .parent()
        .map(|p| p.join("tls"))
        .unwrap_or_else(|| PathBuf::from("tls"))
}

pub fn load_or_create_server_config(computer_name: &str) -> Result<Arc<ServerConfig>> {
    let dir = tls_dir();
    std::fs::create_dir_all(&dir)?;
    let cert_path = dir.join("cert.der");
    let key_path = dir.join("key.der");

    let (cert_der, key_der) = if cert_path.exists() && key_path.exists() {
        (
            std::fs::read(&cert_path).context("read cert.der")?,
            std::fs::read(&key_path).context("read key.der")?,
        )
    } else {
        let (cert_der, key_der) = generate_cert(computer_name)?;
        std::fs::write(&cert_path, &cert_der)?;
        std::fs::write(&key_path, &key_der)?;
        (cert_der, key_der)
    };

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![RustlsCert(cert_der)], RustlsKey(key_der))?;

    Ok(Arc::new(config))
}

fn generate_cert(computer_name: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut params = CertificateParams::new(vec![
        "AirDrop".to_string(),
        computer_name.to_string(),
    ]);
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, computer_name);
    dn.push(DnType::OrganizationName, "AirDropd");
    params.distinguished_name = dn;

    let cert = Certificate::from_params(params)?;
    Ok((cert.serialize_der()?, cert.serialize_private_key_der()))
}
