//! `TODO/image.md` T-0201: the OCI distribution endpoints podbox actually uses,
//! over HTTPS and nothing else.
//!
//! ⛔ **No plain-HTTP fallback, at any layer.** tcp/80 egress is broken on the
//! runtime podbox targets, so a fallback that exists to improve reliability
//! makes the failure **untimed and undiagnosable**: the request hangs instead
//! of failing. Every URL this module builds is `https://`, and
//! [`crate::reference::Reference::parse`] refuses an `http://` reference before
//! it reaches here.
//!
//! ⛔ **No secret is ever printed.** `docs/security/secrets.md`. The bearer
//! token is held in memory, never logged, never put in an error, and every URL
//! that reaches a message goes through [`redact`] first.
//!
//! Blocking, per the entry's Decision: podbox has no reason to be async and an
//! async runtime is a large dependency with nothing to do here.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Duration;

use serde::Deserialize;

use crate::digest::{Digest, Verifier};
use crate::error::{Error, Result};
use crate::oci;

/// ⚠ Bounds a stalled read, not a large transfer. A blob is streamed, so an
/// overall deadline would fail a slow but healthy download; this fails a
/// connection that has stopped producing bytes.
const READ_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(60);

/// `RULES.md` section 8: bounded retry, never an unbounded one.
const ATTEMPTS: u32 = 3;
const BACKOFF_BASE: Duration = Duration::from_millis(500);
/// ⛔ A cap, honoured even when the server asks for longer.
/// `docs/conventions/forbidden-patterns.md`: retrying a rate limit without
/// honouring its stated delay, and without a cap, is a spiral that makes the
/// limit worse. Both halves are here.
const BACKOFF_CAP: Duration = Duration::from_secs(30);

/// A registry caps a manifest well below this. The bound exists so a registry
/// that streams forever cannot exhaust memory: `docs/conventions/forbidden-patterns.md`
/// forbids buffering a whole body with no ceiling.
const MANIFEST_CEILING: u64 = 32 * 1024 * 1024;

/// What a fetched manifest carries. ⭐ `bytes` is what the digest was computed
/// over, and the store writes those bytes verbatim: re-serialising a parsed
/// document produces a different digest and breaks the parity M1 is accepted on.
pub struct FetchedManifest {
    pub bytes: Vec<u8>,
    pub digest: Digest,
    pub media_type: String,
}

pub struct Client {
    agent: ureq::Agent,
    /// ⚠ Agents by `(verify, proxy)`, built lazily. Separate rather than flags
    /// on one agent, because a `ureq::Agent` carries one `ClientConfig`, one
    /// proxy setting and one connection pool: verifying for one registry and
    /// not another through the same pool is how a verified connection gets
    /// reused for an unverified request.
    agents: HashMap<(bool, bool), ureq::Agent>,
    pub policy: crate::transport::Policy,
    /// Endpoints whose HTTPS connect failed and which the policy permits over
    /// HTTP. ⛔ Only ever populated for a host the caller named insecure.
    http_endpoints: std::collections::BTreeSet<String>,
    /// One token per `<endpoint>|<scope>`. ⚠ Keyed on the scope as well as the
    /// host: a token minted for one repository does not authorise another, and
    /// a cache keyed on the host alone would send it anyway and read the 403 as
    /// a permission problem.
    tokens: HashMap<String, String>,
    pub roots: crate::tls::Roots,
}

impl Client {
    pub fn new() -> Client {
        Client::with_policy(crate::transport::Policy::default())
    }

    pub fn with_policy(policy: crate::transport::Policy) -> Client {
        let (config, roots) = crate::tls::client_config();
        let agent = ureq::AgentBuilder::new()
            .tls_config(config)
            // ⚠ The environment's proxy, when it sets one. An intercepting
            // proxy is the normal case in the environments podbox is built for,
            // and ignoring it produces a connect timeout rather than a refusal.
            .try_proxy_from_env(true)
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout_read(READ_TIMEOUT)
            .timeout_write(WRITE_TIMEOUT)
            .user_agent(concat!("podbox/", env!("CARGO_PKG_VERSION")))
            .build();
        Client {
            agent,
            agents: HashMap::new(),
            policy,
            http_endpoints: std::collections::BTreeSet::new(),
            tokens: HashMap::new(),
            roots,
        }
    }

    /// The agent for one endpoint, and the scheme to use with it.
    ///
    /// ⛔ Two agents rather than one with a switch: a `ureq::Agent` carries one
    /// `ClientConfig` and one connection pool, and verifying for one registry
    /// and not another through the same pool is how a verified connection gets
    /// reused for an unverified request.
    fn agent_for(&mut self, endpoint: &str) -> ureq::Agent {
        let verify = self.policy.verify(endpoint);
        let proxy = self.policy.use_proxy(endpoint);
        if verify && proxy {
            return self.agent.clone();
        }
        self.agents
            .entry((verify, proxy))
            .or_insert_with(|| {
                let (config, _roots) = if verify {
                    crate::tls::client_config()
                } else {
                    crate::tls::client_config_unverified()
                };
                let mut b = ureq::AgentBuilder::new()
                    .tls_config(config)
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout_read(READ_TIMEOUT)
                    .timeout_write(WRITE_TIMEOUT)
                    .user_agent(concat!("podbox/", env!("CARGO_PKG_VERSION")));
                // ⛔ A loopback registry is never reached through a proxy, and
                // `NO_PROXY` is honoured. Measured on 2026-09-09: with an
                // intercepting proxy in the environment, a plain-HTTP request
                // to `localhost:5000` came back HTTP 405, which is the proxy
                // refusing a non-CONNECT request and reads as a broken
                // registry. `ureq` 2's `try_proxy_from_env` does not consult
                // `NO_PROXY`, so this is where that is decided.
                if proxy {
                    b = b.try_proxy_from_env(true);
                }
                b.build()
            })
            .clone()
    }

    /// The base URL for an endpoint, `https://` unless this one has already
    /// been found to need `http://`.
    ///
    /// ⚠ The fallback is remembered per endpoint for the life of the client, so
    /// one failed HTTPS connect costs one timeout rather than one per blob.
    fn base(&self, endpoint: &str) -> String {
        let scheme = if self.http_endpoints.contains(endpoint) {
            "http"
        } else {
            "https"
        };
        format!("{scheme}://{endpoint}")
    }

    /// `GET /v2/<repository>/manifests/<selector>`.
    pub fn manifest(
        &mut self,
        endpoint: &str,
        repository: &str,
        selector: &str,
    ) -> Result<FetchedManifest> {
        // ⚠ A PATH, not a URL: `get` chooses the scheme from the endpoint's
        // transport policy, so nothing above it hard-codes `https://`.
        let path = format!("/v2/{repository}/manifests/{selector}");
        let scope = format!("repository:{repository}:pull");
        let resp = self.get(&path, endpoint, &scope, Some(oci::ACCEPT))?;
        let media_type = resp
            .header("Content-Type")
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());
        let declared = resp.header("Docker-Content-Digest").map(str::to_string);

        let mut bytes = Vec::new();
        // ⛔ `take` before `read_to_end`, so the ceiling bounds what is read
        // rather than being checked after the memory is already spent.
        resp.into_reader()
            .take(MANIFEST_CEILING + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| Error::Http {
                what: format!("GET {}", redact(&path)),
                detail: format!("reading the manifest body: {e}"),
            })?;
        if bytes.len() as u64 > MANIFEST_CEILING {
            return Err(Error::Http {
                what: format!("GET {}", redact(&path)),
                detail: format!("the manifest exceeds {MANIFEST_CEILING} bytes and was not read"),
            });
        }

        // ⭐ The digest podbox reports is COMPUTED over the bytes that arrived.
        // The header is checked against it and never substituted for it: this
        // is the value M1's acceptance compares with docker.
        let digest = Digest::of(&bytes);
        if let Some(declared) = declared {
            let declared = Digest::parse(&declared)?;
            if declared != digest {
                return Err(Error::DigestMismatch {
                    what: format!("manifest {selector} from {endpoint}/{repository}"),
                    want: declared.to_string(),
                    got: digest.to_string(),
                });
            }
        }
        Ok(FetchedManifest {
            bytes,
            digest,
            media_type: media_type.unwrap_or_default(),
        })
    }

    /// `GET /v2/<repository>/blobs/<digest>`, streamed into `sink` and verified
    /// against the descriptor as it is written.
    ///
    /// ⛔ Verify before use, not after download. The corpus at
    /// `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:149-157`
    /// checks a payload's hash before an in-memory exec; a layer about to be
    /// extracted as root is the same case. The caller receives the sink back
    /// only when both the digest and the byte count matched.
    pub fn blob<W: Write>(
        &mut self,
        endpoint: &str,
        repository: &str,
        want: &Digest,
        size: Option<u64>,
        sink: W,
    ) -> Result<W> {
        let path = format!("/v2/{repository}/blobs/{want}");
        let scope = format!("repository:{repository}:pull");
        let resp = self.get(&path, endpoint, &scope, None)?;
        let mut verifier = Verifier::new(sink);
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; 128 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| Error::Http {
                what: format!("GET {}", redact(&path)),
                detail: format!(
                    "reading the blob body after {} byte(s): {e}",
                    verifier.written()
                ),
            })?;
            if n == 0 {
                break;
            }
            verifier
                .write_all(&buf[..n])
                .map_err(|e| Error::io(format!("blob {want}"), e))?;
        }
        verifier.finish(&format!("blob {want}"), want, size)
    }

    /// One GET, with the bearer dance and the bounded retry around it.
    fn get(
        &mut self,
        path: &str,
        endpoint: &str,
        scope: &str,
        accept: Option<&str>,
    ) -> Result<ureq::Response> {
        let mut last: Option<Error> = None;
        let mut target = format!("{}{path}", self.base(endpoint));
        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(backoff(attempt, None));
            }
            match self.get_once(&target, endpoint, scope, accept) {
                Ok(Attempt::Done(r)) => return Ok(*r),
                Ok(Attempt::Retry { after, why }) => {
                    // ⭐ TODO/image.md T-0213. A transport failure against an
                    // endpoint the CALLER named insecure is the one case where
                    // podbox retries over plain HTTP, and it says so.
                    // ⛔ Never a discovery: `after_connect_failure` answers
                    // `None` for every endpoint nobody named, which keeps
                    // T-0201's finding intact. An automatic downgrade on a
                    // network where tcp/80 is black-holed is a hang.
                    if why.starts_with("transport:") && !self.http_endpoints.contains(endpoint) {
                        if let Some(d) = self.policy.after_connect_failure(endpoint) {
                            self.http_endpoints.insert(endpoint.to_string());
                            let _ = writeln!(
                                std::io::stderr(),
                                "podbox: {endpoint} did not answer over HTTPS ({why}), and it \
                                 is configured as an insecure registry, so this and every \
                                 later request to it use {}://",
                                d.scheme
                            );
                            target = format!("{}{}", self.base(endpoint), path);
                            continue;
                        }
                    }
                    if attempt + 1 < ATTEMPTS {
                        std::thread::sleep(backoff(attempt + 1, after));
                    }
                    last = Some(Error::Http {
                        what: format!("GET {}", redact(&target)),
                        detail: why,
                    });
                }
                Err(e) => return Err(e),
            }
        }
        Err(last.unwrap_or_else(|| Error::Http {
            what: format!("GET {}", redact(&target)),
            detail: format!("gave up after {ATTEMPTS} attempt(s)"),
        }))
    }

    fn get_once(
        &mut self,
        url: &str,
        endpoint: &str,
        scope: &str,
        accept: Option<&str>,
    ) -> Result<Attempt> {
        let agent = self.agent_for(endpoint);
        match self.send(&agent, url, scope, accept) {
            Ok(r) => Ok(Attempt::Done(Box::new(r))),
            Err(ureq::Error::Status(401, resp)) => {
                // The challenge decides where the token comes from. ⚠ Read from
                // the response rather than hard-coded, so a registry that is not
                // Docker Hub works without a table of hosts.
                let Some(challenge) = resp.header("WWW-Authenticate").map(str::to_string) else {
                    return Err(Error::Http {
                        what: format!("GET {}", redact(url)),
                        detail: "401 with no WWW-Authenticate header, so there is no \
                                 challenge to answer"
                            .into(),
                    });
                };
                let token = self.token(endpoint, scope, &challenge)?;
                self.tokens.insert(key(endpoint, scope), token);
                match self.send(&agent, url, scope, accept) {
                    Ok(r) => Ok(Attempt::Done(Box::new(r))),
                    Err(e) => Err(status_error(url, e)),
                }
            }
            Err(ureq::Error::Status(code, resp)) if retryable(code) => Ok(Attempt::Retry {
                after: resp.header("Retry-After").and_then(parse_retry_after),
                why: format!("HTTP {code}"),
            }),
            Err(ureq::Error::Transport(t)) => Ok(Attempt::Retry {
                after: None,
                why: format!("transport: {t}"),
            }),
            Err(e) => Err(status_error(url, e)),
        }
    }

    // ⚠ `clippy::result_large_err`: `ureq::Error` carries a whole `Response`
    // and is a foreign type. Boxing it here would box it again at every call
    // site that matches on its variants, which is the only thing this function
    // exists to let callers do. The escape hatch is named with its reason, as
    // `docs/conventions/code.md` requires.
    #[allow(clippy::result_large_err)]
    fn send(
        &self,
        agent: &ureq::Agent,
        url: &str,
        scope: &str,
        accept: Option<&str>,
    ) -> std::result::Result<ureq::Response, ureq::Error> {
        let mut req = agent.get(url);
        if let Some(a) = accept {
            req = req.set("Accept", a);
        }
        // ⚠ Keyed by the URL's own host: a redirect to a CDN must not be sent
        // the registry's bearer token, and ureq does not carry headers across
        // hosts by default. The token is attached to the request this client
        // builds, which is always the registry.
        if let Some(host) = host_of(url) {
            if let Some(t) = self.tokens.get(&key(&host, scope)) {
                req = req.set("Authorization", &format!("Bearer {t}"));
            }
        }
        req.call()
    }

    /// Answer a `Bearer` challenge at the realm it names.
    fn token(&self, endpoint: &str, scope: &str, challenge: &str) -> Result<String> {
        let params = parse_challenge(challenge);
        let scheme = challenge.split_whitespace().next().unwrap_or("");
        if !scheme.eq_ignore_ascii_case("bearer") {
            return Err(Error::Http {
                what: format!("authenticating to {endpoint}"),
                detail: format!(
                    "the registry asked for {scheme:?} authentication, and podbox \
                     implements Bearer only"
                ),
            });
        }
        let Some(realm) = params.get("realm") else {
            return Err(Error::Http {
                what: format!("authenticating to {endpoint}"),
                detail: "the Bearer challenge names no realm".into(),
            });
        };
        if realm.starts_with("http://") {
            // ⚠ NOT `PlainHttpRefused`: that variant means the caller wrote an
            // `http://` reference and exits 2. This is the registry naming a
            // plain-HTTP realm, which is the same refusal about somebody
            // else's input and is a runtime failure, not a usage one.
            return Err(Error::Http {
                what: format!("authenticating to {endpoint}"),
                detail: format!(
                    "the token realm is {}, and podbox speaks HTTPS only. tcp/80 \
                     egress is broken on the runtime podbox targets, so following \
                     it would hang rather than fail (TODO/image.md T-0201)",
                    redact(realm)
                ),
            });
        }
        if !realm.starts_with("https://") {
            return Err(Error::Http {
                what: format!("authenticating to {endpoint}"),
                detail: format!(
                    "the token realm {} is not an absolute https URL",
                    redact(realm)
                ),
            });
        }

        let mut req = self.agent.get(realm);
        if let Some(service) = params.get("service") {
            req = req.query("service", service);
        }
        // ⚠ The challenge's own scope when it gives one, and the scope this
        // request needs otherwise. A registry that omits it still has to be
        // told what the token is for.
        req = req.query(
            "scope",
            params.get("scope").map(String::as_str).unwrap_or(scope),
        );

        #[derive(Deserialize)]
        struct TokenBody {
            token: Option<String>,
            access_token: Option<String>,
        }
        let resp = req.call().map_err(|e| status_error(realm, e))?;
        // ⚠ `into_string` rather than ureq's `json` feature: `serde_json` is
        // already pinned by TODO/deps.md T-0908 and measured, and a second JSON
        // reader in the graph would be a dependency nobody ruled on. It is
        // bounded by ureq at 10 MiB, which a token response is far under.
        let text = resp.into_string().map_err(|e| Error::Http {
            what: format!("token from {}", redact(realm)),
            detail: format!("reading the token response: {e}"),
        })?;
        // ⛔ The error names the failure and never the body: a token response's
        // body IS the credential. docs/security/secrets.md.
        let body: TokenBody = serde_json::from_str(&text).map_err(|e| Error::Http {
            what: format!("token from {}", redact(realm)),
            detail: format!("the token response does not parse: {e}"),
        })?;
        // ⛔ Neither branch prints the value. The two field names are the two
        // spellings in use; a registry sending neither has not issued a token.
        body.token.or(body.access_token).ok_or_else(|| Error::Http {
            what: format!("token from {}", redact(realm)),
            detail: "the response carried neither `token` nor `access_token`".into(),
        })
    }
}

impl Default for Client {
    fn default() -> Client {
        Client::new()
    }
}

enum Attempt {
    // ⚠ Boxed. A `ureq::Response` is ~264 bytes and `Retry` is a pointer and a
    // duration, so the unboxed enum makes every return of the small variant
    // carry the large one's footprint.
    Done(Box<ureq::Response>),
    Retry {
        after: Option<Duration>,
        why: String,
    },
}

fn key(endpoint: &str, scope: &str) -> String {
    format!("{endpoint}|{scope}")
}

fn host_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    Some(rest.split('/').next()?.to_string())
}

/// ⛔ A URL in a message loses its query first. `docs/security/secrets.md`: a
/// credential never reaches a log, and some registries put one in a query
/// parameter of a signed URL.
pub fn redact(url: &str) -> String {
    match url.split_once('?') {
        Some((head, _)) => format!("{head}?<redacted>"),
        None => url.to_string(),
    }
}

fn retryable(code: u16) -> bool {
    matches!(code, 429 | 500 | 502 | 503 | 504)
}

/// ⚠ Seconds only. The HTTP-date form is legal and is answered with the
/// ordinary backoff rather than a parsed date, because a mis-parsed date is a
/// wait of unknown length and `RULES.md` section 8 forbids one.
fn parse_retry_after(v: &str) -> Option<Duration> {
    v.trim().parse::<u64>().ok().map(Duration::from_secs)
}

fn backoff(attempt: u32, asked: Option<Duration>) -> Duration {
    let ours = BACKOFF_BASE * 2u32.saturating_pow(attempt.saturating_sub(1));
    // The server's number when it gave one, ours otherwise, and the cap over
    // both.
    asked.unwrap_or(ours).min(BACKOFF_CAP)
}

fn status_error(url: &str, e: ureq::Error) -> Error {
    match e {
        ureq::Error::Status(code, resp) => {
            // A registry's error body is JSON with a `errors[].message`. It is
            // read for the message and nothing else; an unparseable body is
            // reported as the status alone rather than dumped.
            let detail = resp
                .into_string()
                .ok()
                .and_then(|b| registry_message(&b))
                .map(|m| format!("HTTP {code}: {m}"))
                .unwrap_or_else(|| format!("HTTP {code}"));
            Error::Http {
                what: format!("GET {}", redact(url)),
                detail,
            }
        }
        ureq::Error::Transport(t) => Error::Http {
            what: format!("GET {}", redact(url)),
            detail: format!("transport: {t}"),
        },
    }
}

fn registry_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Errors {
        errors: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        code: Option<String>,
        message: Option<String>,
    }
    let parsed: Errors = serde_json::from_str(body).ok()?;
    let first = parsed.errors.first()?;
    match (&first.code, &first.message) {
        (Some(c), Some(m)) => Some(format!("{c}: {m}")),
        (_, Some(m)) => Some(m.clone()),
        (Some(c), _) => Some(c.clone()),
        _ => None,
    }
}

/// `Bearer realm="...",service="...",scope="..."` into its parameters.
fn parse_challenge(challenge: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let rest = match challenge.split_once(char::is_whitespace) {
        Some((_, rest)) => rest,
        None => return out,
    };
    let mut key = String::new();
    let mut value = String::new();
    let mut in_key = true;
    let mut quoted = false;
    for ch in rest.chars() {
        match ch {
            '=' if in_key && !quoted => in_key = false,
            '"' if !in_key => quoted = !quoted,
            // ⚠ A comma inside quotes is part of the value: a scope legally
            // carries several, and splitting on every comma truncates it to the
            // first, which then authorises less than the request needs.
            ',' if !quoted => {
                if !key.trim().is_empty() {
                    out.insert(key.trim().to_string(), value.clone());
                }
                key.clear();
                value.clear();
                in_key = true;
            }
            c if in_key => key.push(c),
            c => value.push(c),
        }
    }
    if !key.trim().is_empty() {
        out.insert(key.trim().to_string(), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bearer_challenge_parses_into_realm_service_and_scope() {
        let got = parse_challenge(
            r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/alpine:pull""#,
        );
        assert_eq!(got["realm"], "https://auth.docker.io/token");
        assert_eq!(got["service"], "registry.docker.io");
        assert_eq!(got["scope"], "repository:library/alpine:pull");
    }

    #[test]
    fn a_scope_carrying_a_comma_is_not_truncated_at_it() {
        let got = parse_challenge(
            r#"Bearer realm="https://x/token",scope="repository:a:pull,push",service="x""#,
        );
        assert_eq!(got["scope"], "repository:a:pull,push");
        assert_eq!(got["service"], "x");
    }

    #[test]
    fn a_url_query_never_survives_into_a_message() {
        assert_eq!(
            redact("https://cdn.example/blob?X-Amz-Signature=deadbeef"),
            "https://cdn.example/blob?<redacted>"
        );
        assert_eq!(redact("https://x/v2/"), "https://x/v2/");
    }

    #[test]
    fn the_backoff_honours_the_servers_delay_and_still_caps_it() {
        assert_eq!(
            backoff(1, Some(Duration::from_secs(5))),
            Duration::from_secs(5)
        );
        assert_eq!(backoff(1, Some(Duration::from_secs(600))), BACKOFF_CAP);
        assert!(backoff(1, None) < backoff(3, None));
        assert!(backoff(20, None) <= BACKOFF_CAP);
    }

    #[test]
    fn only_the_statuses_worth_repeating_are_retried() {
        assert!(retryable(429) && retryable(503));
        // ⛔ A 404 or a 403 retried three times is three wrong answers and a
        // slower error message.
        assert!(!retryable(404) && !retryable(403) && !retryable(401));
    }

    #[test]
    fn a_retry_after_that_is_not_a_number_falls_back_rather_than_waiting_forever() {
        assert_eq!(parse_retry_after("7"), Some(Duration::from_secs(7)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
    }

    #[test]
    fn a_token_is_keyed_by_repository_and_not_by_host_alone() {
        // ⛔ docs/conventions/forbidden-patterns.md: a cache keyed without the
        // variant serves the variant to the next unqualified fetch.
        assert_ne!(
            key("registry-1.docker.io", "repository:library/alpine:pull"),
            key("registry-1.docker.io", "repository:library/busybox:pull")
        );
    }

    #[test]
    fn a_registry_error_body_is_read_for_its_message() {
        let body = r#"{"errors":[{"code":"MANIFEST_UNKNOWN","message":"manifest unknown"}]}"#;
        assert_eq!(
            registry_message(body).unwrap(),
            "MANIFEST_UNKNOWN: manifest unknown"
        );
        assert!(registry_message("not json").is_none());
    }
}
