//! Helpers for creating TLS connections.

use rustls::{Certificate, ClientConfig, RootCertStore};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// A representation of what trust anchors to use for server certificate verification.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
#[non_exhaustive]
pub enum Trust {
    /// Only use the provided root certificates.
    Only(Vec<PathBuf>),
    /// Use system root certificates.
    #[default]
    Default,
    /// Use these root certificates in addition to system root certificates.
    Also(Vec<PathBuf>),
    /// Disables server identity verification.
    ///
    /// This is usually a bad idea, but may be necessary to connect to some servers.
    NoVerify,
}

/// `ServerCertVerifier` that verifies literally everything.
#[derive(Clone, Copy, Debug, Default)]
struct NoVerifier;

impl rustls::client::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _: &rustls::Certificate,
        _: &[rustls::Certificate],
        _: &rustls::ServerName,
        _: &mut dyn Iterator<Item = &[u8]>,
        _: &[u8],
        _: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        // :)
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

/// `rustls` client configuration wrapped in an [`Arc`].
pub type TlsConfig = Arc<ClientConfig>;

/// Basic options for creating a [`TlsConfig`].
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct TlsConfigOptions {
    /// Options for validating the server's identity.
    pub trust: Trust,
    /// An optional path to a PEM-encoded file containing
    /// one `PKCS#8` private key and client certificate chain.
    ///
    /// Used for networks that support CertFP.
    pub cert: Option<PathBuf>,
}

fn load_pem(path: &Path, certs: &mut RootCertStore) -> std::io::Result<()> {
    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
    for cert in rustls_pemfile::certs(&mut file)? {
        certs
            .add(&rustls::Certificate(cert))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    }
    Ok(())
}

fn load_client_cert(path: &Path) -> std::io::Result<(Vec<Certificate>, rustls::PrivateKey)> {
    let mut key = Option::<rustls::PrivateKey>::None;
    let mut certs = Vec::<Certificate>::new();
    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
    while let Some(item) = rustls_pemfile::read_one(&mut file)? {
        match item {
            rustls_pemfile::Item::X509Certificate(c) => {
                certs.push(Certificate(c));
            }
            rustls_pemfile::Item::PKCS8Key(k) => {
                key = Some(rustls::PrivateKey(k));
            }
            _ => (),
        }
    }
    key.map(|k| (certs, k))
        .ok_or(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing PKCS#8 private key"))
}

impl TlsConfigOptions {
    /// Builds a [`TlsConfig`] from `self`.
    ///
    /// This is an expensive operation. It should ideally be done only once per network.
    pub fn build(&self) -> std::io::Result<TlsConfig> {
        let cli_auth =
            if let Some(path) = &self.cert { Some(load_client_cert(path)?) } else { None };
        let builder = ClientConfig::builder().with_safe_defaults();
        let config = if matches!(&self.trust, Trust::NoVerify) {
            let builder = builder.with_custom_certificate_verifier(Arc::new(NoVerifier));
            if let Some((certs, key)) = cli_auth {
                builder
                    .with_client_auth_cert(certs, key)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
            } else {
                builder.with_no_client_auth()
            }
        } else {
            let mut certs = RootCertStore { roots: Vec::new() };
            if matches!(&self.trust, Trust::Default | Trust::Also(_)) {
                let natives = rustls_native_certs::load_native_certs()?;
                certs.add_parsable_certificates(&natives);
            }
            if let Trust::Only(paths) | Trust::Also(paths) = &self.trust {
                certs.roots.reserve_exact(paths.len());
                for path in paths {
                    load_pem(path.as_ref(), &mut certs)?;
                }
            }
            let builder = builder.with_root_certificates(certs);
            if let Some((certs, key)) = cli_auth {
                builder
                    .with_client_auth_cert(certs, key)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
            } else {
                builder.with_no_client_auth()
            }
        };
        Ok(Arc::new(config))
    }
}
