//! The trust root for every request podbox makes.
//!
//! ⭐ The host bundle first, `webpki-roots` as the fallback. `Cargo.toml`
//! records why both are pinned: a machine whose egress is intercepted has a
//! private CA in its own bundle and nowhere else, and a client that trusts only
//! the compiled-in roots fails there with an error naming a certificate rather
//! than a proxy. A machine with no bundle at all is the opposite case and is
//! why the compiled-in set is not simply dropped.
//!
//! ⛔ There is no "skip verification" path here, not behind a flag and not
//! behind an environment variable. `docs/security/remote-ops.md` and the
//! environment note in `docs/AGENTS.md` both say never to disable verification,
//! and a switch that exists gets set.

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
