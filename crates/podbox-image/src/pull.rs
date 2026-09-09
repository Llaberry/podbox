//! `podbox pull`: resolve, precheck, fetch, verify, record.
//!
//! The order is the whole design and every step of it is an entry:
//!
//! 1. the reference is normalised, and `http://` is refused ([`crate::reference`], T-0201);
//! 2. the probe answer is taken from `$store/probe.json` where its key still
//!    holds ([`crate::probe_cache`], T-0111), because `pull` is one of the hot
//!    paths `TOOL.md` section 6.1 asks not to re-probe;
//! 3. the manifest is fetched and its digest **computed over the served bytes**
//!    (T-0202);
//! 4. blocks **and inodes** are checked at the destination before a single
//!    layer is fetched, and the refusal names the destination, the free amount,
//!    the required amount and the unit ([`crate::space`], T-0203);
//! 5. every blob is verified as it is written and is renamed into `blobs/` only
//!    on success (T-0202).
//!
//! ⛔ No extraction. `TODO/milestones.md` T-1102: M1 acquires, M2 extracts.

use std::io::Write;

use crate::digest::Digest;
use crate::error::{Error, Result};
use crate::oci::{self, Fetched};
use crate::platform::Platform;
use crate::reference::Reference;
use crate::registry::Client;
use crate::space;
use crate::store::{self, Record, Store};
use crate::transport::Policy;

/// What the store needs beyond the payload: the index, a staging copy of the
/// largest blob and the lock files.
///
/// ⚠ It is **not** an estimate of the extracted size. M2 checks that against
/// the layer contents it is about to write, and inventing a multiplier here
/// would be a fabricated number (`docs/AGENTS.md` absolute 3).
pub const STORE_HEADROOM_BYTES: u64 = 16 * 1024 * 1024;

pub struct Pulled {
    pub record: Record,
    /// Blobs that were already in the store, by digest.
    pub reused: Vec<String>,
    pub fetched: Vec<String>,
    pub probe_source: crate::probe_cache::Source,
}

impl Pulled {
    /// True when nothing had to be fetched, which is docker's "up to date".
    pub fn up_to_date(&self) -> bool {
        self.fetched.is_empty()
    }
}

/// Run a pull, writing docker's transcript to `out`.
///
/// ⚠ The transcript is the **terminal state of each layer**, printed once it is
/// known. There is no synthetic progress bar:
/// `docs/conventions/forbidden-patterns.md` forbids a hardcoded or synthetic
/// progress display, and a percentage podbox cannot measure is exactly one.
/// ⭐ `platform` is what the caller asked for, resolved by
/// [`Platform::wanted`] before it gets here so the flag, the environment and
/// the host are settled in one place rather than three.
pub fn pull(
    store: &Store,
    want: &str,
    platform: &Platform,
    policy: &Policy,
    out: &mut dyn Write,
) -> Result<Pulled> {
    let reference = Reference::parse(want)?;

    // ⭐ TODO/image.md T-0213. The refusal is HERE and not in `Reference::parse`,
    // because only here is the transport policy in scope, and a refusal that
    // cannot name the flag that would permit it is a refusal a caller cannot
    // act on.
    if reference.plain_http && !policy.permits_explicit_http(reference.endpoint()) {
        return Err(Error::PlainHttpRefused(format!(
            "{want:?} names http://, and {0} is not configured as an insecure \
             registry. podbox does not downgrade a connection on its own: \
             tcp/80 is black-holed on the runtimes podbox targets, so an \
             automatic fallback hangs rather than failing (T-0201). Permit it \
             deliberately with `--insecure-registry {0}`, with \
             $PODBOX_INSECURE_REGISTRIES, or with a line in the registries \
             config file",
            reference.endpoint()
        )));
    }
    // ⛔ Announced once, before anything is fetched. An agent cannot notice a
    // downgraded transport the way a person might.
    if let Some(said) = policy.disclosure(reference.endpoint()) {
        let _ = writeln!(std::io::stderr(), "{said}");
    }
    let probe = crate::probe_cache::resolve(store);

    let mut client = Client::with_policy(policy.clone());
    let endpoint = reference.endpoint().to_string();
    let repository = reference.repository.clone();

    let _ = writeln!(
        out,
        "{}: Pulling from {repository}",
        reference
            .tag
            .clone()
            .unwrap_or_else(|| reference.manifest_selector())
    );

    // ---------------------------------------------------------- the manifest
    let top = client.manifest(&endpoint, &repository, &reference.manifest_selector())?;
    let parsed = Fetched::parse(&top.bytes, Some(&top.media_type))?;

    // ⭐ `resolved` is what the REFERENCE resolved to, and it is the value
    // `docker image inspect` reports in RepoDigests. For a multi-platform tag
    // that is the index digest, not the per-platform manifest digest, and
    // recording the wrong one is exactly the parity M1 is accepted on.
    let resolved = top.digest.clone();
    let (manifest_digest, manifest_bytes, manifest) = match parsed {
        Fetched::Manifest(m) => (resolved.clone(), top.bytes.clone(), m),
        Fetched::Index(index) => {
            let picked = oci::select_platform(&index, platform)?;
            let d = picked.parsed_digest()?;
            let inner = client.manifest(&endpoint, &repository, &d.to_string())?;
            match Fetched::parse(&inner.bytes, Some(&inner.media_type))? {
                Fetched::Manifest(m) => (inner.digest, inner.bytes, m),
                Fetched::Index(_) => {
                    return Err(Error::Oci(format!(
                        "{endpoint}/{repository} served an index where the \
                         {platform} manifest should be. podbox does not follow \
                         an index into another index: an image is one level deep"
                    )))
                }
            }
        }
    };

    // ------------------------------------------------------------- the space
    //
    // ⛔ Before a single layer is fetched. Streaming until ENOSPC leaves a
    // partial store to clean up on a filesystem that is already full, which is
    // the state in which cleanup is least likely to work (T-0203).
    let need_bytes: u64 = manifest
        .layers
        .iter()
        .chain(std::iter::once(&manifest.config))
        .filter(|d| {
            d.parsed_digest()
                .map(|x| !store.has_blob(&x))
                .unwrap_or(true)
        })
        .map(|d| d.size)
        .sum::<u64>()
        + top.bytes.len() as u64
        + manifest_bytes.len() as u64;
    // One inode per blob, plus the index, the lock, the cache and the staging
    // file that exists while each blob is in flight.
    let need_inodes = manifest.layers.len() as u64 + 8;
    space::require(
        &store.root().to_string_lossy(),
        &reference.to_string(),
        need_bytes,
        need_inodes,
        STORE_HEADROOM_BYTES,
    )?;

    // ------------------------------------------------------------- the blobs
    let mut reused = Vec::new();
    let mut fetched = Vec::new();

    for (bytes, d, what) in [
        (&top.bytes, &resolved, "manifest"),
        (&manifest_bytes, &manifest_digest, "manifest"),
    ] {
        if store.has_blob(d) {
            continue;
        }
        // ⛔ The bytes as served. Re-serialising the parsed document produces a
        // different digest and breaks the parity this milestone is accepted on.
        store.put_bytes(bytes, d, what)?;
    }

    // ⚠ The config is fetched with the layers and is NOT announced with them.
    // Driving a real pull on 2026-09-08 showed the config's short digest
    // printed as a third `Pull complete` beside two layers, which reads as an
    // image with three layers. docker's transcript names layers only, and a
    // transcript that names something else is a display that lies.
    let announced = manifest.layers.len();
    for (n, descriptor) in manifest
        .layers
        .iter()
        .chain(std::iter::once(&manifest.config))
        .enumerate()
    {
        let is_layer = n < announced;
        let d = descriptor.parsed_digest()?;
        if store.has_blob(&d) {
            if is_layer {
                let _ = writeln!(out, "{}: Already exists", d.short());
            }
            reused.push(d.to_string());
            continue;
        }
        let (staged, file) = store.stage(d.short())?;
        // ⚠ A staged file left behind by a failed fetch is removed here rather
        // than swept later: it is named by this process's pid and nothing else
        // will ever claim it.
        // ⚠ No `BufWriter`. T-0214 restarts this sink on a body that was cut
        // short, and a `BufWriter` has no way to discard what it is holding; the
        // fetch already writes one 128 KiB chunk at a time, so the buffer was
        // adding a copy rather than a saving.
        let result = client
            .blob(&endpoint, &repository, &d, Some(descriptor.size), file, out)
            .and_then(|mut w| {
                w.flush()
                    .map_err(|e| Error::io(staged.display().to_string(), e))?;
                w.sync_all()
                    .map_err(|e| Error::io(staged.display().to_string(), e))
            })
            .and_then(|()| store.commit(&staged, &d));
        if let Err(e) = result {
            let _ = std::fs::remove_file(&staged);
            return Err(e);
        }
        if is_layer {
            let _ = writeln!(out, "{}: Pull complete", d.short());
        }
        fetched.push(d.to_string());
    }

    // ------------------------------------------------------------ the record
    let config_digest = manifest.config.parsed_digest()?;
    let config: oci::Config = serde_json::from_slice(&store.read_blob(&config_digest)?)
        .map_err(|e| Error::Oci(format!("the image config does not parse: {e}")))?;
    if !config.architecture.is_empty() && !config.os.is_empty() {
        // ⛔ The variant is checked against what was asked for, not assumed
        // from the index entry. `docs/conventions/forbidden-patterns.md`: a
        // cache holding a variant it was not keyed by serves it to the next
        // unqualified fetch, and `Exec format error` is how that surfaces.
        // ⛔ Checked against what was ASKED FOR, not against the host. A
        // deliberate `--platform linux/arm64` on an amd64 machine must not be
        // refused here; a registry serving amd64 bytes under an arm64
        // descriptor must be.
        if !platform.matches(&config.os, &config.architecture, None) {
            return Err(Error::Oci(format!(
                "the config of {reference} declares {}/{}, and podbox asked for \
                 {platform}. The store records the platform of what it holds, \
                 so a mismatch here is refused rather than recorded",
                config.os, config.architecture,
            )));
        }
    }

    let record = store::record_of(
        &reference,
        &resolved,
        &top.media_type,
        &manifest_digest,
        &manifest,
        &config,
        &platform.to_string(),
    )?;
    store.put_record(record.clone())?;

    let _ = writeln!(out, "Digest: {resolved}");
    let _ = writeln!(
        out,
        "Status: {} for {reference}",
        if fetched.is_empty() {
            "Image is up to date"
        } else {
            "Downloaded newer image"
        }
    );
    Ok(Pulled {
        record,
        reused,
        fetched,
        probe_source: probe.source,
    })
}

/// The digest a store already holds for a reference, for the "already present"
/// path that never touches the network.
pub fn local(store: &Store, want: &str) -> Result<Option<Digest>> {
    match store.find(want)?.first() {
        Some(r) => Ok(Some(Digest::parse(&r.digest)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ The refusal T-0201 won, kept, and now able to say what would permit
    /// it. A message a caller cannot act on is a message that costs a session.
    #[test]
    fn an_http_reference_is_refused_and_the_refusal_names_the_flag() {
        let store = Store::open(
            std::env::temp_dir().join(format!("podbox-pull-http-{}", std::process::id())),
        )
        .unwrap();
        let mut out = Vec::new();
        let e = pull(
            &store,
            "http://localhost:5000/x:latest",
            &Platform::host(),
            &Policy::default(),
            &mut out,
        );
        let Err(e) = e else {
            panic!("a pull succeeded with nothing listening")
        };
        let text = format!("{e}");
        assert!(
            text.contains("--insecure-registry localhost:5000"),
            "{text}"
        );
        assert!(text.contains("T-0201"), "{text}");
        // ⛔ And nothing was fetched: the refusal is before the network.
        assert!(out.is_empty(), "it printed a transcript before refusing");
        let _ = std::fs::remove_dir_all(store.root());
    }

    /// ⭐ The same reference, with the registry named insecure, gets past the
    /// policy. ⚠ It then fails to CONNECT, because nothing is listening on
    /// localhost:5000 in a test, and that is the right place to stop: this
    /// asserts the policy decision, and `experiments/280-insecure-registry.sh`
    /// drives the whole path against a registry that is really there.
    #[test]
    fn naming_the_registry_insecure_gets_past_the_policy() {
        let store = Store::open(
            std::env::temp_dir().join(format!("podbox-pull-ok-{}", std::process::id())),
        )
        .unwrap();
        let mut out = Vec::new();
        let e = pull(
            &store,
            "http://localhost:5000/x:latest",
            &Platform::host(),
            &Policy::with_insecure(&["localhost:5000"]),
            &mut out,
        );
        let Err(e) = e else {
            panic!("a pull succeeded with nothing listening")
        };
        let text = format!("{e}");
        assert!(
            !text.contains("--insecure-registry"),
            "the policy still refused it: {text}"
        );
        let _ = std::fs::remove_dir_all(store.root());
    }
}
