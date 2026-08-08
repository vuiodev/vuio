//! TLS connection to Cast devices.
//!
//! Cast devices use self-signed TLS certificates on port 8009.
//! Certificate verification is **disabled by default** because Cast
//! devices do not use CA-signed certificates. This means the connection
//! is encrypted but not authenticated — a LAN attacker could MITM.
//!
//! This is the same trade-off made by pychromecast, go-chromecast,
//! rust_cast, and node-castv2. The Cast protocol has a separate device
//! authentication channel (`urn:x-cast:com.google.cast.tp.deviceauth`)
//! which is not yet implemented in oxicast.
//!
//! Set `verify_tls(true)` on the builder if your device has a CA-signed
//! certificate (uncommon).

use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::error::{Error, Result};

/// Establish a TLS connection to a Cast device.
///
/// Returns the connected TLS stream which can be split into read/write halves.
pub async fn connect(host: &str, port: u16, verify_tls: bool) -> Result<TlsStream<TcpStream>> {
    let addr = format!("{host}:{port}");
    tracing::debug!("connecting to cast device at {addr}");

    let tcp = TcpStream::connect(&addr).await.map_err(Error::Connect)?;

    let config = if verify_tls {
        let mut root_store = rustls::RootCertStore::empty();
        let certs_result = rustls_native_certs::load_native_certs();
        for cert in certs_result.certs {
            let _ = root_store.add(cert);
        }
        ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth()
    } else {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertVerification))
            .with_no_client_auth()
    };

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(host.to_string())
        .or_else(|_| {
            let ip: std::net::IpAddr = host.parse().map_err(|e| {
                Error::Connect(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
            })?;
            Ok(match ip {
                std::net::IpAddr::V4(v4) => {
                    ServerName::IpAddress(rustls::pki_types::IpAddr::V4(v4.into()))
                }
                std::net::IpAddr::V6(v6) => {
                    ServerName::IpAddress(rustls::pki_types::IpAddr::V6(v6.into()))
                }
            })
        })
        .map_err(|e: Error| Error::Tls(format!("invalid host: {e}")))?;

    let tls_stream =
        connector.connect(server_name, tcp).await.map_err(|e| Error::Tls(format!("{e}")))?;

    tracing::debug!("TLS connection established to {addr}");
    Ok(tls_stream)
}

/// TLS certificate verifier that accepts all certificates (insecure).
///
/// Required for Cast devices which use self-signed certificates.
/// The connection is still encrypted — only authentication is skipped.
#[derive(Debug)]
struct NoCertVerification;

impl rustls::client::danger::ServerCertVerifier for NoCertVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
