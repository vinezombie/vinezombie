//! Helpers for creating TLS connections.

use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ClientConfig, RootCertStore,
};
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

/// `ServerCertVerifier` that doesn't care at all about the server cert.
#[derive(Clone, Copy, Debug)]
struct NoVerifier(&'static Arc<rustls::crypto::CryptoProvider>);

impl Default for NoVerifier {
    fn default() -> Self {
        NoVerifier(
            rustls::crypto::CryptoProvider::get_default()
                .expect("no default rustls crypto prodiver"),
        )
    }
}

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // :)
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
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
    for cert in rustls_pemfile::certs(&mut file) {
        let cert = cert?;
        certs
            .add(CertificateDer::from(cert))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    }
    Ok(())
}

fn load_client_cert(
    path: &Path,
) -> std::io::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let mut key = Option::<PrivateKeyDer>::None;
    let mut certs = Vec::<CertificateDer>::new();
    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
    while let Some(item) = rustls_pemfile::read_one(&mut file)? {
        match item {
            rustls_pemfile::Item::X509Certificate(c) => {
                certs.push(CertificateDer::from(c));
            }
            rustls_pemfile::Item::Pkcs8Key(k) => {
                key = Some(PrivateKeyDer::from(k));
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
            if let Some(path) = self.cert.as_ref() { Some(load_client_cert(path)?) } else { None };
        let builder = ClientConfig::builder();
        let config = if matches!(&self.trust, Trust::NoVerify) {
            let builder = builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier::default()));
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
                certs.add_parsable_certificates(natives);
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
