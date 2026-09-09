//! The trust root for every request podbox makes.
//!
//! ⭐ The host bundle first, `webpki-roots` as the fallback. `Cargo.toml`
//! records why both are pinned: a machine whose egress is intercepted has a
//! private CA in its own bundle and nowhere else, and a client that trusts only
//! the compiled-in roots fails there with an error naming a certificate rather
//! than a proxy. A machine with no bundle at all is the opposite case and is
//! why the compiled-in set is not simply dropped.
//!
//! ⭐ **There IS a "skip verification" path, and it is never taken by default.**
//! [`crate::transport::Policy`] decides, from a `--tls-verify=false`, an
//! `--insecure-registry`, `$PODBOX_INSECURE_REGISTRIES` or the config file, and
//! every use of it is announced on stderr.
//! [`TODO/image.md`](../../../TODO/image.md) T-0213.
//!
//! ⚠ **This paragraph used to say the opposite, and it cited a rule that does
//! not exist.** It read: "There is no skip-verification path here, not behind a
//! flag and not behind an environment variable. `docs/security/remote-ops.md`
//! and the environment note in `docs/AGENTS.md` both say never to disable
//! verification". Checked on 2026-09-09: `remote-ops.md` says nothing about TLS
//! at all, and `docs/AGENTS.md`'s note is an instruction to the **agent** about
//! this development container's intercepting proxy, not a rule about what
//! podbox may offer. ⛔ "A switch that exists gets set" is a real argument and
//! is answered rather than dismissed: the switch is off by default, it is
//! per-registry rather than global, and it prints a line naming the registry
//! every time it is used, so a set switch is visible in the transcript of an
//! automated caller rather than inferred from a missing failure.

use std::sync::Arc;

use rustls::ClientConfig;
use rustls_pki_types::CertificateDer;

/// Where a host keeps its CA bundle, in the order they are tried. ⚠ The
/// environment variables come first because that is how an intercepting proxy
/// announces itself; the paths after them are the distributions in the corpus's
/// own `experiments/125-across-distributions.sh` set.
const BUNDLE_ENV: &[&str] = &["SSL_CERT_FILE", "CURL_CA_BUNDLE", "REQUESTS_CA_BUNDLE"];
const BUNDLE_PATHS: &[&str] = &[
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/ssl/ca-bundle.pem",
    "/etc/ssl/cert.pem",
    "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
];

/// Which roots a config was built from, for the diagnostic. ⛔ Reported, never
/// assumed: "TLS failed" is unactionable when the reader cannot tell which
/// trust store was consulted.
pub struct Roots {
    pub source: String,
    pub count: usize,
}

fn load_bundle(path: &str) -> Option<Vec<CertificateDer<'static>>> {
    let data = std::fs::read(path).ok()?;
    let mut rd = std::io::BufReader::new(&data[..]);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut rd)
        .filter_map(|c| c.ok())
        .collect();
    if certs.is_empty() {
        None
    } else {
        Some(certs)
    }
}

fn host_bundle() -> Option<(String, Vec<CertificateDer<'static>>)> {
    for var in BUNDLE_ENV {
        if let Some(p) = std::env::var_os(var) {
            let p = p.to_string_lossy().to_string();
            if let Some(certs) = load_bundle(&p) {
                return Some((format!("${var} ({p})"), certs));
            }
        }
    }
    for p in BUNDLE_PATHS {
        if let Some(certs) = load_bundle(p) {
            return Some(((*p).to_string(), certs));
        }
    }
    None
}

/// A verifier that accepts anything, for a registry the caller named insecure.
///
/// ⛔ Reachable only through [`client_config_unverified`], which
/// [`crate::registry::Client`] calls only for an endpoint
/// [`crate::transport::Policy`] permits. It is not a global mode and there is
/// no environment variable that reaches it without naming a host.
#[derive(Debug)]
struct AcceptAny(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
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
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
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

/// The configuration for an endpoint whose certificate is not checked.
///
/// ⚠ The chain is still built and the handshake still happens; what is skipped
/// is deciding whether the chain reaches a trusted root. A caller who sees
/// `Roots.source` here reads exactly that, so a log cannot be mistaken for a
/// verified connection.
pub fn client_config_unverified() -> (Arc<ClientConfig>, Roots) {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAny(provider)))
        .with_no_client_auth();
    (
        Arc::new(config),
        Roots {
            source: "⚠ NONE: this registry was named insecure, so its \
                     certificate was not verified against any root"
                .to_string(),
            count: 0,
        },
    )
}

/// Build the client configuration and say where its trust came from.
pub fn client_config() -> (Arc<ClientConfig>, Roots) {
    let mut store = rustls::RootCertStore::empty();
    let roots = match host_bundle() {
        Some((source, certs)) => {
            let offered = certs.len();
            let (added, _ignored) = store.add_parsable_certificates(certs);
            // ⚠ The count is what the store ACCEPTED, not what the file held.
            // A bundle whose certificates all fail to parse would otherwise be
            // reported as trust that does not exist.
            if added == 0 {
                store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                Roots {
                    source: format!(
                        "webpki-roots, compiled in: {source} held {offered} PEM block(s) and \
                         none parsed as a certificate"
                    ),
                    count: store.len(),
                }
            } else {
                Roots {
                    source,
                    count: added,
                }
            }
        }
        None => {
            store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Roots {
                source: "webpki-roots, compiled in: this machine has no CA bundle at any \
                         known path"
                    .to_string(),
                count: store.len(),
            }
        }
    };

    // ⛔ The provider is named rather than taken from the process default.
    // `ClientConfig::builder()` resolves a default that depends on which
    // provider features anything else in the graph enabled, so naming `ring`
    // here is what makes the build's crypto the crypto TODO/deps.md T-0905
    // measured.
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("ring supports the default protocol versions")
            .with_root_certificates(store)
            .with_no_client_auth();

    (Arc::new(config), roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_is_built_and_names_where_its_roots_came_from() {
        // The production default of the seam, not an injected double:
        // docs/conventions/code.md requires the shipping branch be tested.
        let (_cfg, roots) = client_config();
        assert!(roots.count > 0, "no roots from {}", roots.source);
        assert!(!roots.source.is_empty());
    }

    #[test]
    fn a_bundle_that_is_not_pem_yields_nothing_rather_than_an_empty_trust_store() {
        let dir = std::env::temp_dir().join(format!("podbox-tls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("not-a-bundle.pem");
        std::fs::write(&p, b"this is not a certificate\n").unwrap();
        assert!(load_bundle(p.to_str().unwrap()).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
