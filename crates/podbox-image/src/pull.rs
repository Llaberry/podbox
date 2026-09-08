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
use crate::reference::Reference;
use crate::registry::Client;
use crate::space;
use crate::store::{self, Record, Store};

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
pub fn pull(store: &Store, want: &str, out: &mut dyn Write) -> Result<Pulled> {
    let reference = Reference::parse(want)?;
    let probe = crate::probe_cache::resolve(store);

    let mut client = Client::new();
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
            let picked = oci::select_platform(&index, oci::OS, oci::ARCH)?;
            let d = picked.parsed_digest()?;
            let inner = client.manifest(&endpoint, &repository, &d.to_string())?;
            match Fetched::parse(&inner.bytes, Some(&inner.media_type))? {
                Fetched::Manifest(m) => (inner.digest, inner.bytes, m),
                Fetched::Index(_) => {
                    return Err(Error::Oci(format!(
                        "{}/{repository} served an index where the {}/{} manifest \
                         should be. podbox does not follow an index into another \
                         index: an image is one level deep",
                        endpoint,
                        oci::OS,
                        oci::ARCH
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
        let result = client
            .blob(
                &endpoint,
                &repository,
                &d,
                Some(descriptor.size),
                std::io::BufWriter::new(file),
            )
            .and_then(|mut w| {
                w.flush()
                    .map_err(|e| Error::io(staged.display().to_string(), e))?;
                w.into_inner()
                    .map_err(|e| Error::io(staged.display().to_string(), e.into_error()))?
                    .sync_all()
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
        if config.os != oci::OS || config.architecture != oci::ARCH {
            return Err(Error::Oci(format!(
                "the config of {reference} declares {}/{}, and podbox asked for \
                 {}/{}. The store records the platform of what it holds, so a \
                 mismatch here is refused rather than recorded",
                config.os,
                config.architecture,
                oci::OS,
                oci::ARCH
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
        &format!("{}/{}", oci::OS, oci::ARCH),
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
