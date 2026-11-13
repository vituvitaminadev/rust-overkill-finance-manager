//! A identidade TLS do Anfitrião (ADR 0002): um certificado autoassinado,
//! guardado no banco da residência, que as cópias fixam ao parear.

use std::sync::{Arc, Mutex};

use finance_core::Household;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme};
use sha2::{Digest, Sha256};

const CERT_KEY: &str = "tls_cert";
const KEY_KEY: &str = "tls_key";

pub struct Identity {
    cert: Vec<u8>,
    key: Vec<u8>,
}

impl Identity {
    /// A identidade guardada no banco, criada na primeira vez.
    pub fn load_or_create(household: &Household) -> Result<Identity, IdentityError> {
        let stored = household.local(CERT_KEY)?.zip(household.local(KEY_KEY)?);
        if let Some((cert, key)) = stored
            && let (Some(cert), Some(key)) = (from_hex(&cert), from_hex(&key))
        {
            return Ok(Identity { cert, key });
        }
        let generated = rcgen::generate_simple_self_signed(vec!["finance-manager.local".into()])
            .map_err(|e| IdentityError::Tls(e.to_string()))?;
        let identity = Identity {
            cert: generated.cert.der().to_vec(),
            key: generated.signing_key.serialize_der(),
        };
        household.set_local(CERT_KEY, &to_hex(&identity.cert))?;
        household.set_local(KEY_KEY, &to_hex(&identity.key))?;
        Ok(identity)
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.cert)
    }

    pub fn server_config(&self) -> Result<ServerConfig, IdentityError> {
        ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(tls_error)?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.cert.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
            )
            .map_err(tls_error)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error(transparent)]
    Core(#[from] finance_core::Error),
    #[error("Erro de TLS: {0}")]
    Tls(String),
}

fn tls_error(error: rustls::Error) -> IdentityError {
    IdentityError::Tls(error.to_string())
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A impressão digital SHA-256 de um certificado, em hexadecimal.
pub fn fingerprint(cert: &[u8]) -> String {
    to_hex(&Sha256::digest(cert))
}

/// O começo da impressão digital num formato fácil de comparar em voz alta,
/// como `3FA2-9C01`, exibido nos dois computadores durante o pareamento.
pub fn security_code(fingerprint: &str) -> String {
    let start: String = fingerprint
        .chars()
        .take(8)
        .collect::<String>()
        .to_uppercase();
    match start.len() {
        8 => format!("{}-{}", &start[..4], &start[4..]),
        _ => start,
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Como a cópia decide se o servidor é o Anfitrião que ela conhece.
#[derive(Debug, Clone)]
pub enum Pin {
    /// Só aceita o certificado com esta impressão digital.
    Fingerprint(String),
    /// Primeiro contato: aceita o que vier e informa a impressão digital vista,
    /// para que a pessoa a confira antes de confiar.
    TrustFirst,
}

/// Onde fica, depois da conexão, a impressão digital que o servidor apresentou.
pub type SeenFingerprint = Arc<Mutex<Option<String>>>;

/// A configuração TLS da cópia e, depois da conexão, a impressão digital que
/// o servidor apresentou.
pub fn client_config(pin: Pin) -> Result<(ClientConfig, SeenFingerprint), IdentityError> {
    let seen = Arc::new(Mutex::new(None));
    let verifier = PinnedVerifier {
        pin,
        seen: seen.clone(),
        provider: provider(),
    };
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok((config, seen))
}

/// Não há autoridade certificadora na rede de casa: a confiança vem de o
/// certificado ser exatamente o que foi fixado no pareamento.
#[derive(Debug)]
struct PinnedVerifier {
    pin: Pin,
    seen: SeenFingerprint,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let presented = fingerprint(end_entity.as_ref());
        // Anotada mesmo quando recusada, para que a cópia saiba dizer que
        // falou com alguém que não é o seu Anfitrião.
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(presented.clone());
        if let Pin::Fingerprint(expected) = &self.pin
            && !expected.eq_ignore_ascii_case(&presented)
        {
            return Err(rustls::Error::General(
                "o certificado não é o do Anfitrião pareado".into(),
            ));
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
