//! `TODO/image.md` T-0202 and T-0204: one content-addressed store, shared by
//! images and containers, with a lock.
//!
//! ⛔ **Every blob is verified as it is written, never afterwards.** The reason
//! is in the corpus at
//! `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:149-157`, which
//! checks a payload's content hash before an in-memory exec so an in-memory
//! exec never runs unverified bytes. A layer about to be extracted as root is
//! the same case. Bytes that fail verification never reach `blobs/`: they are
//! written to a staging name inside the store and renamed in only on success.
//!
//! ⛔ **Images are keyed by manifest digest**, and the digest recorded is the
//! one computed over the bytes the registry served for the reference that was
//! asked for. That is what `docker image inspect` reports in `RepoDigests`, and
//! parity with it is what M1 is accepted on ([`TODO/milestones.md`](../../../TODO/milestones.md) T-1102).
//!
//! ⚠ One store for images and containers, not one per container. The GC race
//! that decides it is T-0204, and the answer is [`Lock`]: an advisory lock on an
//! fd rather than a pid file, because a pid file is stale the moment a process
//! dies unexpectedly and the check that clears a stale one is the race being
//! closed.
//!
//! ⛔ That fd is `O_CLOEXEC`, and the one caller that wants the payload to
//! inherit it says so with [`Lock::keep_across_exec`] immediately before its
//! `execve`. T-0211 is what the other shape cost: an fd left inheritable from
//! open time is inherited by every unrelated `fork` while the lock is held, and
//! each such child keeps the `flock` alive for its own lifetime.
//!
//! # The concurrency contract
//!
//! ⭐ **[`TODO/image.md`](../../../TODO/image.md) T-0210, and it is written here
//! rather than in the entry because a contract nobody reads while changing the
//! code is not one.** Two concurrency defects landed in this crate before it
//! existed ([T-0211](../../../TODO/image.md), and `TODO/probe.md` T-0113 in the
//! probe), and the second was found by luck. Every invariant below is driven by
//! `experiments/210-store-concurrency.sh` against real concurrent processes,
//! because a race only a mock can produce is a race the mock's author imagined.
//!
//! **I1. One writer at a time, over the index.** Every read-modify-write of
//! `store.json` happens under [`Store::lock`], an exclusive `flock` on
//! `$store/lock`, waited for a bounded number of times.
//!
//! **I2. A reader needs no lock, and never sees a half-written index.** The
//! index is replaced by [`std::fs::rename`], which is atomic within a
//! filesystem, so a reader sees the previous document or the next one. ⚠ This is
//! why `staging/` is inside the store: a rename across filesystems is `EXDEV`.
//!
//! **I3. A blob is immutable once it is named.** `blobs/` is content-addressed,
//! so two writers racing to produce one name necessarily produce identical
//! bytes and the second rename is a no-op. Bytes that fail verification never
//! get a name. A reader therefore needs no lock for a blob either: it sees the
//! file complete, or not at all.
//!
//! **I4. Nothing deletes a blob a holder needs.** `rmi` and `prune` take the
//! index lock and check [`Store::in_use`] **inside it**, and [`Store::hold`]
//! takes that same lock while it acquires the image lock. ⛔ Checking `in_use`
//! outside the lock is a check whose answer is stale before it is used: a
//! `hold` taken in between kept its image lock and lost its blobs.
//!
//! **I5. A killed process leaves exactly one kind of litter, and it is swept.**
//! Blobs are only ever named by rename after verification and the index only
//! ever replaced by rename, so the only thing a `SIGKILL` can leave is a
//! `*.partial` under `staging/`. [`Store::open`] sweeps them, and it decides
//! what is orphaned by **trying to `flock` each one**: a live writer holds its
//! own staging file for as long as it is writing, so a file this process can
//! lock is a file nobody is writing. ⚠ Never by pid: a pid is reused, and the
//! check that clears a stale pid file is itself the race being closed.
//!
//! **I6. A staging name is unique per CALL, not per process.** ⛔ `TODO/probe.md`
//! T-0113 is this exact defect one crate over: a scratch name carrying only the
//! pid collides between two threads of one process, and `cargo test` and any
//! future concurrent layer fetch ([T-0207](../../../TODO/image.md)) are both
//! threads of one process.
//!
//! **I7. Locks are taken in one order: the index, then an image.** Both `prune`
//! and `hold` take them that way, so neither can wait on the other.

use std::io::Write;
use std::path::{Path, PathBuf};

use podbox_probe::sys::{self, CBuf};
use serde::{Deserialize, Serialize};

use crate::clock;
use crate::contain;
use crate::digest::Digest;
use crate::error::{Error, Result};
use crate::oci;
use crate::reference::Reference;

/// ⭐ A version discriminator on everything persisted.
/// `docs/conventions/code.md`: old data still reads and new code knows which
/// version it is looking at. A store written by a later podbox is refused by
/// name rather than parsed as though its fields meant what they mean here.
pub const INDEX_VERSION: u32 = 1;
const INDEX_FILE: &str = "store.json";
const BLOBS: &str = "blobs";
const LOCKS: &str = "locks";
const STAGING: &str = "staging";
const STORE_LOCK: &str = "lock";

/// How long the index lock is waited for before podbox says who is holding it.
/// ⚠ Bounded, per `RULES.md` section 8: a runtime whose audience is automated
/// may not wait unbounded.
const LOCK_ATTEMPTS: u32 = 100;
const LOCK_SLEEP: std::time::Duration = std::time::Duration::from_millis(50);

/// ⛔ Invariant I6. What makes a staging name unique per CALL rather than per
/// process, which is the difference `TODO/probe.md` T-0113 cost one crate over.
static STAGE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// `<domain>/<repository>`, canonical. ⛔ Never the display form: `alpine`
    /// and `docker.io/library/alpine` are one image and a store keyed on the
    /// display form holds it twice.
    pub repository: String,
    pub tag: Option<String>,
    /// What the reference resolved to, computed over the served bytes. This is
    /// the value `podbox images --format '{{.Digest}}'` prints.
    pub digest: String,
    pub digest_media_type: String,
    /// The single-platform manifest under it. Equal to `digest` when the
    /// registry served a manifest rather than an index.
    pub manifest_digest: String,
    /// docker's image ID: the digest of the config blob.
    pub config_digest: String,
    /// ⛔ Recorded, because `docs/conventions/forbidden-patterns.md`: fetching a
    /// variant into a cache keyed without the variant serves the variant to the
    /// next unqualified fetch.
    pub platform: String,
    pub layers: Vec<String>,
    /// What the layers and the config occupy in `blobs/`, summed from the
    /// descriptors and checked against what arrived.
    pub stored_bytes: u64,
    pub architecture: String,
    pub os: String,
    /// From the image config. `None` where the config declares none.
    pub created: Option<String>,
    pub pulled_at: String,
}

impl Record {
    /// Everything in `blobs/` this record needs.
    pub fn blobs(&self) -> Vec<&str> {
        let mut v: Vec<&str> = vec![
            self.digest.as_str(),
            self.manifest_digest.as_str(),
            self.config_digest.as_str(),
        ];
        v.extend(self.layers.iter().map(String::as_str));
        v.sort_unstable();
        v.dedup();
        v
    }

    /// How docker prints the repository column.
    pub fn display_repository(&self) -> String {
        match self.repository.strip_prefix("docker.io/") {
            Some(rest) => match rest.strip_prefix("library/") {
                Some(short) if !short.contains('/') => short.to_string(),
                _ => rest.to_string(),
            },
            None => self.repository.clone(),
        }
    }

    pub fn name(&self) -> String {
        match &self.tag {
            Some(t) => format!("{}:{t}", self.display_repository()),
            None => format!("{}@{}", self.display_repository(), self.digest),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub podbox_store: u32,
    #[serde(default)]
    pub images: Vec<Record>,
}

impl Default for Index {
    fn default() -> Index {
        Index {
            podbox_store: INDEX_VERSION,
            images: Vec::new(),
        }
    }
}

/// What a platform-qualified lookup found.
///
/// ⛔ Two states rather than an empty vector, because they need different
/// sentences: "podbox does not hold this image" and "podbox holds this image,
/// for another platform" send a caller to different remedies.
#[derive(Debug, Default)]
pub struct Found {
    pub matched: Vec<Record>,
    /// The platforms the store DOES hold for this reference, when none matched.
    pub other_platforms: Vec<String>,
}

impl Found {
    pub fn one(self) -> Option<Record> {
        self.matched.into_iter().next()
    }
}

#[derive(Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Where the store lives, and why.
    ///
    /// ⚠ `$PODBOX_STORE` first, then the XDG data directory, then `$HOME`.
    /// Deliberately **not** `$TMPDIR`: on the runtime podbox targets `/tmp` is
    /// 64 MiB, which almost no image fits in, and `env::temp_dir()` with no
    /// space check is the shipped default this project read in the corpus at
    /// `references/VHSgunzo__memfd-exec/tree/src/executable.rs:580-584`.
    /// Where none of the three is set, `fallback` is consulted; the caller
    /// supplies the probe's write allowlist, ranked by free space (T-0203).
    pub fn default_root(fallback: impl FnOnce() -> Option<String>) -> Result<PathBuf> {
        if let Some(p) = std::env::var_os("PODBOX_STORE") {
            let p = PathBuf::from(p);
            if !p.as_os_str().is_empty() {
                return Ok(p);
            }
        }
        for (var, suffix) in [("XDG_DATA_HOME", "podbox"), ("HOME", ".local/share/podbox")] {
            if let Some(base) = std::env::var_os(var) {
                let base = PathBuf::from(base);
                if base.is_absolute() {
                    return Ok(base.join(suffix));
                }
            }
        }
        match fallback() {
            Some(p) => Ok(PathBuf::from(p).join("podbox")),
            None => Err(Error::Store(
                "no store directory: $PODBOX_STORE, $XDG_DATA_HOME and $HOME are \
                 all unset or relative, and no probed writable path could hold \
                 one. Set $PODBOX_STORE to a directory podbox may write"
                    .into(),
            )),
        }
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Store> {
        let root = root.into();
        for d in [
            root.clone(),
            root.join(BLOBS).join(crate::digest::SHA256),
            root.join(LOCKS),
            root.join(STAGING),
        ] {
            std::fs::create_dir_all(&d).map_err(|e| Error::io(d.display().to_string(), e))?;
        }
        let store = Store { root };
        // ⛔ Refuse a store a later podbox wrote, rather than reading its fields
        // as though they meant what they mean here.
        let index = store.read_index()?;
        if index.podbox_store > INDEX_VERSION {
            return Err(Error::Store(format!(
                "{} declares store version {} and this podbox writes {INDEX_VERSION}. \
                 A newer podbox made it; this one will not edit it",
                store.index_path().display(),
                index.podbox_store
            )));
        }
        // ⛔ Invariant I5. Every command opens the store, so this is where an
        // abandoned staging file is reclaimed. `docs/conventions/code.md` calls
        // it the sweep that heals the drift the happy path let slip.
        store.sweep_staging();
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE)
    }

    pub fn blob_path(&self, d: &Digest) -> PathBuf {
        self.root.join(d.blob_path())
    }

    pub fn has_blob(&self, d: &Digest) -> bool {
        self.blob_path(d).is_file()
    }

    pub fn read_index(&self) -> Result<Index> {
        let path = self.index_path();
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                Error::Store(format!(
                    "{} does not parse: {e}. It is podbox's own index; move it \
                     aside to start a fresh store",
                    path.display()
                ))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
            Err(e) => Err(Error::io(path.display().to_string(), e)),
        }
    }

    /// ⛔ Written to a staging name inside the store and renamed over the index,
    /// so a process killed mid-write leaves the previous index intact rather
    /// than a truncated one. `rename(2)` is atomic within a filesystem, which
    /// is why the staging directory is inside the store and not in `/tmp`.
    pub fn write_index(&self, index: &Index) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(index)
            .map_err(|e| Error::Store(format!("serialising the index: {e}")))?;
        // ⛔ Invariants I5 and I6: unique per call, named `.partial` so the one
        // sweep rule covers it, and held while it is written so the sweep
        // cannot take it out from under this write.
        let tmp = self.root.join(STAGING).join(format!(
            "{INDEX_FILE}.{}.{}.partial",
            std::process::id(),
            STAGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        {
            let mut f = StagedFile::create(&tmp)?;
            f.write_all(&bytes)
                .and_then(|_| f.write_all(b"\n"))
                .and_then(|_| f.sync_all())
                .map_err(|e| Error::io(tmp.display().to_string(), e))?;
        }
        std::fs::rename(&tmp, self.index_path())
            .map_err(|e| Error::io(self.index_path().display().to_string(), e))
    }

    /// A staging file inside the store, for a blob whose digest is not yet
    /// proved. ⚠ Inside the store because the commit is a `rename(2)`, and a
    /// rename across filesystems fails with `EXDEV`.
    ///
    /// ⛔ **Invariant I6: unique per CALL.** The name carried only the pid until
    /// 2026-09-09, and `TODO/probe.md` T-0113 is that same defect one crate
    /// over: two threads of one process take one name and overwrite each other.
    /// `cargo test` runs tests in threads, and [T-0210](../../../TODO/image.md)'s
    /// own bounded-concurrency sibling would make it reachable in production.
    ///
    /// ⛔ **Invariant I5: the writer holds it.** The returned handle carries an
    /// exclusive `flock` for as long as it lives, which is what lets
    /// [`Store::sweep_staging`] tell an abandoned file from one being written
    /// without asking about a pid.
    pub fn stage(&self, hint: &str) -> Result<(PathBuf, StagedFile)> {
        let safe: String = hint
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .take(24)
            .collect();
        let path = self.root.join(STAGING).join(format!(
            "{safe}.{}.{}.partial",
            std::process::id(),
            STAGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let f = StagedFile::create(&path)?;
        Ok((path, f))
    }

    /// ⛔ **Invariant I5.** Remove every abandoned staging file, deciding what
    /// is abandoned by trying to lock it.
    ///
    /// ⚠ Returns what it removed rather than printing: the caller decides
    /// whether a sweep is worth a line, and `Store::open` runs on every command.
    /// ⛔ Only inside this store's own `staging/`, resolved through
    /// [`crate::contain`], because a sweep is an unlink and every unlink this
    /// crate performs is gated the same way.
    pub fn sweep_staging(&self) -> Vec<PathBuf> {
        let dir = self.root.join(STAGING);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut swept = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("partial") {
                continue;
            }
            let Ok(resolved) = contain::within(&self.root, &path) else {
                continue;
            };
            // ⭐ The whole test: a live writer holds this file, so a lock this
            // process can take means nobody is writing it.
            match Lock::try_acquire(&resolved, sys::LOCK_EX) {
                Ok(Some(_held)) => {
                    if std::fs::remove_file(&resolved).is_ok() {
                        swept.push(resolved);
                    }
                }
                _ => continue,
            }
        }
        swept
    }

    /// Name a staged file by its proved digest. ⛔ The caller has already
    /// verified it: [`crate::digest::Verifier`] is what produces the digest
    /// passed here, so a blob that failed verification has no path to this
    /// function.
    pub fn commit(&self, staged: &Path, d: &Digest) -> Result<()> {
        let dest = self.blob_path(d);
        contain::within(&self.root, &dest)?;
        std::fs::rename(staged, &dest).map_err(|e| Error::io(dest.display().to_string(), e))
    }

    /// A small blob already in memory, verified before it is written.
    pub fn put_bytes(&self, bytes: &[u8], want: &Digest, what: &str) -> Result<()> {
        let got = Digest::of(bytes);
        if got != *want {
            return Err(Error::DigestMismatch {
                what: what.to_string(),
                want: want.to_string(),
                got: got.to_string(),
            });
        }
        let (staged, mut f) = self.stage(what)?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| Error::io(staged.display().to_string(), e))?;
        drop(f);
        self.commit(&staged, want)
    }

    pub fn read_blob(&self, d: &Digest) -> Result<Vec<u8>> {
        let p = self.blob_path(d);
        let bytes = std::fs::read(&p).map_err(|e| Error::io(p.display().to_string(), e))?;
        // ⛔ Re-verified on the way out. `docs/conventions/code.md`: a guard
        // re-checking an invariant the happy path already knows earns its keep
        // the one time it fires, and here it fires when something outside
        // podbox has edited the store.
        let got = Digest::of(&bytes);
        if got != *d {
            return Err(Error::DigestMismatch {
                what: format!("stored blob {}", p.display()),
                want: d.to_string(),
                got: got.to_string(),
            });
        }
        Ok(bytes)
    }

    // ------------------------------------------------------------- the locks

    /// The store-wide lock, held across an index read-modify-write.
    pub fn lock(&self) -> Result<Lock> {
        Lock::acquire(&self.root.join(STORE_LOCK), sys::LOCK_EX)
    }

    fn image_lock_path(&self, record: &Record) -> Result<PathBuf> {
        let d = Digest::parse(&record.digest)?;
        Ok(self.root.join(LOCKS).join(format!("{}.lock", d.hex())))
    }

    /// ⭐ T-0204's mechanism. A **shared** advisory lock, released only when the
    /// last holder's fd is closed, however that process ends. Several
    /// containers may share one image, so the lock is shared; the GC asks for
    /// an exclusive one and is refused while any holder remains.
    ///
    /// ⛔ Two defences, because they cover different failures, and
    /// [`TODO/image.md`](../../../TODO/image.md) T-0211 is what having neither
    /// cost. `O_CLOEXEC` keeps the lock out of an unrelated **exec**, and
    /// registering the fd with [`sys::close_in_children`] keeps it out of an
    /// unrelated **fork**, which `O_CLOEXEC` cannot do, because there is no
    /// close-on-fork and `flock` is held on the open file description a fork
    /// duplicates. The one caller that wants a payload to inherit it says so
    /// with [`Lock::hand_to_payload`], which undoes both.
    pub fn hold(&self, record: &Record) -> Result<Lock> {
        let path = self.image_lock_path(record)?;
        // ⛔ INVARIANT I4 AND I7. The index lock is taken first and dropped at
        // the end of this function, so a `prune` cannot be between its own
        // `in_use` check and its unlink while this hold is being taken. Without
        // it the check is stale before it is used: measured as a reading of the
        // code on 2026-09-09, `prune` asked `in_use`, a `run` took its hold, and
        // `prune` then deleted the blobs the run was about to execute out of
        // AND the lock file it was holding. Both take index-then-image, so
        // neither can wait on the other.
        let _index = self.lock()?;
        let lock = Lock::acquire(&path, sys::LOCK_SH)?;
        if !sys::close_in_children(lock.fd) {
            return Err(Error::Store(format!(
                "{} image locks are already held by this process, which is every \
                 slot podbox has for fds a fork must shed. Holding one more \
                 would leak it into every child forked from here (T-0211), so \
                 it is refused rather than held unsafely",
                sys::FORK_CLOSE_SLOTS
            )));
        }
        Ok(lock)
    }

    /// Whether any process holds [`Store::hold`] on this image.
    ///
    /// ⚠ Tested by asking for the exclusive lock and reading `EWOULDBLOCK`,
    /// then dropping it immediately. There is no way to ask "is this locked"
    /// without trying, and a pid file that could be asked is the stale-state
    /// race this replaces.
    pub fn in_use(&self, record: &Record) -> Result<bool> {
        let path = self.image_lock_path(record)?;
        if !path.exists() {
            return Ok(false);
        }
        match Lock::try_acquire(&path, sys::LOCK_EX) {
            Ok(Some(_probe)) => Ok(false),
            Ok(None) => Ok(true),
            Err(e) => Err(e),
        }
    }

    // ----------------------------------------------------------- the queries

    /// Every record, newest pull first.
    pub fn list(&self) -> Result<Vec<Record>> {
        let mut images = self.read_index()?.images;
        images.sort_by(|a, b| {
            b.pulled_at
                .cmp(&a.pulled_at)
                .then(a.repository.cmp(&b.repository))
                .then(a.tag.cmp(&b.tag))
        });
        Ok(images)
    }

    /// Records a reference names. A tag or digest names at most one; a bare
    /// repository or an image id may name several.
    ///
    /// ⚠ Selected by NAME, never by position: `docs/conventions/code.md`.
    pub fn find(&self, want: &str) -> Result<Vec<Record>> {
        let all = self.list()?;
        // An image id, full or docker's twelve-digit short form.
        let id_like = want.len() >= 12
            && want
                .trim_start_matches("sha256:")
                .bytes()
                .all(|b| b.is_ascii_hexdigit());
        if id_like {
            let hex = want.trim_start_matches("sha256:");
            let hits: Vec<Record> = all
                .iter()
                .filter(|r| {
                    r.config_digest
                        .trim_start_matches("sha256:")
                        .starts_with(hex)
                })
                .cloned()
                .collect();
            if !hits.is_empty() {
                return Ok(hits);
            }
        }
        let Ok(reference) = Reference::parse(want) else {
            return Ok(Vec::new());
        };
        let repo = reference.canonical_repository();
        let hits: Vec<Record> = all
            .into_iter()
            .filter(|r| {
                r.repository == repo
                    && match (&reference.digest, &reference.tag) {
                        (Some(d), _) => r.digest == d.to_string(),
                        (None, Some(t)) => r.tag.as_deref() == Some(t.as_str()),
                        (None, None) => true,
                    }
            })
            .collect();
        Ok(hits)
    }

    /// The records a reference names, narrowed to one platform.
    ///
    /// ⛔ `None` means **this machine's**, not "any". A caller that asked for
    /// no platform is asking about the image it could run, and handing it an
    /// arm64 record on an amd64 host is the `Exec format error` this whole
    /// module exists to turn into a sentence. ⚠ Where nothing matches the host
    /// the hits are returned unfiltered rather than emptied, so the caller can
    /// say "podbox holds this image, for another platform" instead of "no such
    /// image", which is a different and more useful failure.
    pub fn find_for(
        &self,
        want: &str,
        platform: Option<&crate::platform::Platform>,
    ) -> Result<Found> {
        let hits = self.find(want)?;
        let host = crate::platform::Platform::host();
        let want_p = platform.unwrap_or(&host);
        // ⛔ Filtered whatever the count. An earlier version short-circuited on
        // a single hit, and a store holding only `linux/amd64` then answered a
        // `--platform linux/arm64` request with the amd64 record: podbox ran the
        // wrong architecture and said nothing. A count is not a match.
        let matched: Vec<Record> = hits
            .iter()
            .filter(|r| r.platform == want_p.to_string())
            .cloned()
            .collect();
        Ok(Found {
            other_platforms: if matched.is_empty() {
                let mut p: Vec<String> = hits.iter().map(|r| r.platform.clone()).collect();
                p.sort_unstable();
                p.dedup();
                p
            } else {
                Vec::new()
            },
            matched,
        })
    }

    pub fn find_one(&self, want: &str) -> Result<Record> {
        self.find_one_for(want, None)
    }

    /// One record, or a refusal that says why there is more than one.
    ///
    /// ⛔ **NEVER `.next()` ON AN AMBIGUOUS REFERENCE.** Until 2026-09-09 this
    /// took the first hit, and that was harmless only because a store could not
    /// hold two records for one tag. [T-0212](../../../TODO/image.md) made it
    /// hold one per platform, and the same line then meant `podbox extract
    /// alpine` silently unpacked whichever platform was pulled most recently:
    /// selected **by position**, which `docs/conventions/code.md` forbids and
    /// which `Store::find`'s own comment calls out three functions above.
    ///
    /// ⚠ The host's platform is preferred rather than demanded, because a store
    /// holding exactly one foreign platform and asked for no platform in
    /// particular is not ambiguous: there is one answer and refusing it would be
    /// pedantry. Ambiguity is two or more surviving that preference.
    pub fn find_one_for(
        &self,
        want: &str,
        platform: Option<&crate::platform::Platform>,
    ) -> Result<Record> {
        let hits = self.find(want)?;
        if hits.is_empty() {
            return Err(Error::NoSuchImage(want.to_string()));
        }
        if hits.len() == 1 && platform.is_none() {
            return Ok(hits.into_iter().next().expect("length checked"));
        }
        let host = crate::platform::Platform::host();
        let prefer = platform.unwrap_or(&host);
        let narrowed: Vec<Record> = hits
            .iter()
            .filter(|r| r.platform == prefer.to_string())
            .cloned()
            .collect();
        match narrowed.len() {
            1 => Ok(narrowed.into_iter().next().expect("length checked")),
            0 if platform.is_some() => {
                let mut have: Vec<String> = hits.iter().map(|r| r.platform.clone()).collect();
                have.sort_unstable();
                have.dedup();
                Err(Error::NoSuchImage(format!(
                    "{want} for {prefer}. The store holds it for {}",
                    have.join(", ")
                )))
            }
            0 if hits.len() == 1 => Ok(hits.into_iter().next().expect("length checked")),
            _ => {
                let mut have: Vec<String> = hits.iter().map(|r| r.platform.clone()).collect();
                have.sort_unstable();
                have.dedup();
                Err(Error::Usage(format!(
                    "{want} names {} images in this store, for {}. podbox will \
                     not pick one by position: name the platform with \
                     --platform, or the image by its digest",
                    hits.len(),
                    have.join(", ")
                )))
            }
        }
    }

    // ------------------------------------------------------------ the writes

    /// Record a pull. Replaces the record for the same repository, tag **and
    /// platform**, because a moving tag is the normal case.
    ///
    /// ⛔ **The platform is part of the key and that is a multi-architecture
    /// decision.** `alpine:latest` for `linux/amd64` and for `linux/arm64` are
    /// two different images that share one name, and keying without the
    /// platform means the second pull silently deletes the first: the record
    /// goes, the blobs are collected, and a caller who pulled both has one.
    /// podman keeps both and so does podbox. `Store::find` is where the
    /// resulting ambiguity is resolved, by name and never by position.
    pub fn put_record(&self, record: Record) -> Result<()> {
        let _guard = self.lock()?;
        let mut index = self.read_index()?;
        index.podbox_store = INDEX_VERSION;
        index.images.retain(|r| {
            !(r.repository == record.repository
                && r.tag == record.tag
                && r.platform == record.platform
                && (record.tag.is_some() || r.digest == record.digest))
        });
        index.images.push(record);
        self.write_index(&index)
    }

    /// `podbox tag <src> <dst>`. The destination points at the same manifest
    /// digest; nothing is fetched and no blob is copied.
    pub fn tag(&self, src: &str, dst: &str) -> Result<Record> {
        let source = self.find_one(src)?;
        let target = Reference::parse(dst)?;
        if target.digest.is_some() {
            return Err(Error::Usage(format!(
                "{dst:?} names a digest. A tag is a name podbox assigns, and a \
                 digest is one the content assigns; the second cannot be set"
            )));
        }
        let record = Record {
            repository: target.canonical_repository(),
            tag: target.tag.clone(),
            pulled_at: clock::now(),
            ..source
        };
        self.put_record(record.clone())?;
        Ok(record)
    }

    /// `podbox rmi`. Removes the records a reference names and then every blob
    /// no surviving record reaches.
    ///
    /// ⛔ Refuses while a container holds the image, and says which.
    pub fn remove(&self, want: &str) -> Result<Removed> {
        let doomed = self.find(want)?;
        if doomed.is_empty() {
            return Err(Error::NoSuchImage(want.to_string()));
        }
        // ⛔ Invariant I4: the `in_use` check is inside `delete`, under the
        // index lock, and never here. This function used to make it and it was
        // stale by the time `delete` acted on it.
        self.delete(&doomed, Held::Refuse)
    }

    /// `podbox image prune`. Without `all`, only untagged records; with it,
    /// every record no container holds.
    pub fn prune(&self, all: bool) -> Result<Removed> {
        let mut doomed = Vec::new();
        for r in self.list()? {
            if !all && r.tag.is_some() {
                continue;
            }
            doomed.push(r);
        }
        // ⛔ Invariant I4: what is held is decided under the lock, by `delete`,
        // and `Held::Skip` is what makes a held image a named skip here where
        // `rmi` refuses outright.
        self.delete(&doomed, Held::Skip)
    }

    fn delete(&self, doomed: &[Record], on_held: Held) -> Result<Removed> {
        let _guard = self.lock()?;
        // ⛔ INVARIANT I4. Inside the lock, so a `hold` cannot be taken between
        // this answer and the unlinks below: `Store::hold` takes the same lock.
        let mut skipped = Vec::new();
        let mut kept = Vec::new();
        for r in doomed {
            if self.in_use(r)? {
                match on_held {
                    Held::Refuse => {
                        return Err(Error::Store(format!(
                            "{} is in use by a running container and was not removed. \
                             A GC that deletes an extraction out from under a payload \
                             makes its failure read as a missing file rather than as a \
                             concurrent deletion (TODO/image.md T-0204)",
                            r.name()
                        )))
                    }
                    Held::Skip => {
                        skipped.push(r.name());
                        continue;
                    }
                }
            }
            kept.push(r.clone());
        }
        let doomed: &[Record] = &kept;
        let mut index = self.read_index()?;
        let doomed_keys: Vec<(String, Option<String>, String)> = doomed
            .iter()
            .map(|r| (r.repository.clone(), r.tag.clone(), r.digest.clone()))
            .collect();
        index.images.retain(|r| {
            !doomed_keys
                .iter()
                .any(|(repo, tag, dig)| r.repository == *repo && r.tag == *tag && r.digest == *dig)
        });

        // ⭐ Reachability over what SURVIVES, not over what was deleted. A blob
        // several tags share is kept while any of them remains, and computing
        // the doomed set instead would delete it with the first.
        let mut keep: Vec<String> = Vec::new();
        for r in &index.images {
            keep.extend(r.blobs().into_iter().map(str::to_string));
        }
        let mut freed_bytes = 0u64;
        let mut freed = Vec::new();
        for r in doomed {
            for b in r.blobs() {
                if keep.iter().any(|k| k == b) || freed.iter().any(|f| f == b) {
                    continue;
                }
                let d = Digest::parse(b)?;
                let path = self.blob_path(&d);
                // ⛔ One gate per action: every unlink this crate performs is
                // resolved against the store root first.
                let resolved = contain::within(&self.root, &path)?;
                if let Ok(meta) = std::fs::metadata(&resolved) {
                    freed_bytes += meta.len();
                }
                match std::fs::remove_file(&resolved) {
                    Ok(()) => freed.push(b.to_string()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => freed.push(b.to_string()),
                    Err(e) => return Err(Error::io(resolved.display().to_string(), e)),
                }
            }
            let lock = self.image_lock_path(r)?;
            if lock.exists() {
                let resolved = contain::within(&self.root, &lock)?;
                let _ = std::fs::remove_file(resolved);
            }
        }
        self.write_index(&index)?;
        Ok(Removed {
            untagged: doomed.iter().map(Record::name).collect(),
            deleted: freed,
            freed_bytes,
            skipped,
        })
    }
}

/// What [`Store::delete`] does about an image a holder is using.
///
/// ⛔ Two behaviours and one check, because the check has to happen under the
/// index lock (invariant I4) and only the caller knows whether being held is a
/// refusal (`rmi`, which names one image) or a skip (`prune`, which sweeps).
#[derive(Debug, Clone, Copy)]
enum Held {
    Refuse,
    Skip,
}

#[derive(Debug)]
pub struct Removed {
    pub untagged: Vec<String>,
    pub deleted: Vec<String>,
    pub freed_bytes: u64,
    /// ⛔ Named, never silent. T-0204: `prune` and `rmi` skip anything locked
    /// **and say which**.
    pub skipped: Vec<String>,
}

/// A staging file, held exclusively for as long as this value lives.
///
/// ⛔ **Invariant I5's mechanism.** [`Store::sweep_staging`] decides what is
/// abandoned by trying to lock each `*.partial`, so a file being written has to
/// be locked or the sweep would delete it out from under its writer. The lock
/// is on the same open file description as the writes, and it goes when this
/// value does, however the process ends.
pub struct StagedFile {
    file: std::fs::File,
    /// ⚠ Held for its `Drop`, and never read. The lock is the point.
    _lock: Lock,
}

impl StagedFile {
    fn create(path: &Path) -> Result<StagedFile> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::io(parent.display().to_string(), e))?;
        }
        // ⛔ The lock FIRST. Creating the file and locking it afterwards leaves
        // a window in which a sweep sees an unlocked `*.partial` and removes it.
        let lock = match Lock::try_acquire(path, sys::LOCK_EX)? {
            Some(l) => l,
            None => {
                return Err(Error::Store(format!(
                    "{} is already being written by another podbox. Invariant I6 \
                     makes this name unique per call, so two writers on one name is \
                     a defect rather than contention",
                    path.display()
                )))
            }
        };
        let file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| Error::io(path.display().to_string(), e))?;
        Ok(StagedFile { file, _lock: lock })
    }

    pub fn sync_all(&self) -> std::io::Result<()> {
        self.file.sync_all()
    }
}

impl Write for StagedFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// An advisory lock held for as long as this value lives.
pub struct Lock {
    fd: i64,
    pub path: PathBuf,
}

impl Lock {
    fn open(path: &Path) -> Result<i64> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::io(parent.display().to_string(), e))?;
        }
        let Some(c) = CBuf::new(&path.to_string_lossy()) else {
            return Err(Error::Store(format!(
                "{} contains a NUL and cannot reach the kernel",
                path.display()
            )));
        };
        // ⛔ `O_CLOEXEC`, like every other fd in this tree. T-0204's mechanism
        // wants the image lock to survive **one** exec, and T-0211 is what it
        // cost to get that by leaving the fd inheritable from the moment it was
        // opened: every unrelated fork in between inherits it too.
        // [`Lock::keep_across_exec`] is the deliberate act, made one call before
        // the `execve` it is for.
        let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
        sys::open(&c, flags, 0o644).map_err(|e| {
            Error::Store(format!(
                "opening the lock {}: {} ({})",
                path.display(),
                e.name(),
                e.0
            ))
        })
    }

    /// `None` where somebody else holds it. ⛔ Always `LOCK_NB`: a blocking
    /// `flock` on a lock held through an exec is an unbounded wait, which
    /// `RULES.md` section 8 forbids.
    /// An exclusive lock, or `None` where somebody holds it.
    ///
    /// ⚠ Public because `podbox-supervise` reconciles its container table with
    /// exactly this question ([`TODO/supervise.md`](../../../TODO/supervise.md)
    /// T-0604): a launcher holds its container's lock for its whole life, so a
    /// lock another process can take is a launcher that is gone. ⛔ The same
    /// mechanism and not a second one: a pid file would be stale the moment a
    /// launcher is killed, and the check that clears a stale one is the race.
    pub fn try_exclusive(path: &Path) -> Result<Option<Lock>> {
        Lock::try_acquire(path, sys::LOCK_EX)
    }

    /// Take an exclusive lock, waiting the bounded number of attempts.
    pub fn exclusive(path: &Path) -> Result<Lock> {
        Lock::acquire(path, sys::LOCK_EX)
    }

    fn try_acquire(path: &Path, op: u64) -> Result<Option<Lock>> {
        let fd = Lock::open(path)?;
        match sys::flock(fd, op | sys::LOCK_NB) {
            Ok(_) => Ok(Some(Lock {
                fd,
                path: path.to_path_buf(),
            })),
            Err(sys::EWOULDBLOCK) => {
                let _ = sys::close(fd);
                Ok(None)
            }
            Err(e) => {
                let _ = sys::close(fd);
                Err(Error::Store(format!(
                    "flock({}): {} ({})",
                    path.display(),
                    e.name(),
                    e.0
                )))
            }
        }
    }

    fn acquire(path: &Path, op: u64) -> Result<Lock> {
        for _ in 0..LOCK_ATTEMPTS {
            if let Some(l) = Lock::try_acquire(path, op)? {
                return Ok(l);
            }
            std::thread::sleep(LOCK_SLEEP);
        }
        Err(Error::Store(format!(
            "{} is held by another podbox after {:?}. Another pull or prune is \
             running; podbox waits a bounded time and then says so rather than \
             blocking (RULES.md section 8)",
            path.display(),
            LOCK_SLEEP * LOCK_ATTEMPTS
        )))
    }

    /// Hand this one lock to the payload, and to nothing else.
    ///
    /// ⭐ T-0204 needs the image lock to outlive `podbox` itself: the guard is
    /// what stops a concurrent `rmi` or `prune` deleting a rootfs a running
    /// container is executing out of, and podbox is not the process that holds
    /// the container open. This undoes both of [`Store::hold`]'s defences for
    /// this one descriptor, it stops being shed by `clone_fork` and its
    /// `FD_CLOEXEC` is cleared, so the very next `fork` and `execve` carry it
    /// into the payload.
    ///
    /// ⛔ Called immediately before the fork that leads to that `execve`, never
    /// at open time. T-0211 is what the second shape costs: a lock that is
    /// inheritable for its whole life is inherited by every unrelated `fork` in
    /// that window, in this tree, by the fifty short-lived children one
    /// `podbox probe` makes, and each one holds the `flock` open for its own
    /// lifetime. The image then reads as in use after its holder released it,
    /// and `rmi` refuses an image nothing is using.
    ///
    /// ⚠ The returned fd is deliberately raw: its consumer is the code between
    /// `fork` and `execve`, where allocating is not allowed.
    pub fn hand_to_payload(&self) -> Result<i64> {
        // ⛔ Order matters. The fd stops being shed only after `FD_CLOEXEC` is
        // cleared, so a fork racing this call either sheds it or inherits a
        // descriptor that is still close-on-exec. Neither outcome leaks a lock.
        let flags = sys::fcntl(self.fd, sys::F_GETFD, 0).map_err(|e| {
            Error::Store(format!(
                "reading the descriptor flags of {}: {} ({})",
                self.path.display(),
                e.name(),
                e.0
            ))
        })?;
        let cleared = (flags as u64) & !sys::FD_CLOEXEC;
        sys::fcntl(self.fd, sys::F_SETFD, cleared).map_err(|e| {
            Error::Store(format!(
                "clearing FD_CLOEXEC on {}: {} ({})",
                self.path.display(),
                e.name(),
                e.0
            ))
        })?;
        sys::stop_closing_in_children(self.fd);
        Ok(self.fd)
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // ⛔ Deregistered before the close, so a fork racing this never sheds a
        // descriptor number that has already been handed back to the kernel and
        // reused by another thread.
        sys::stop_closing_in_children(self.fd);
        // Closing the fd releases the flock. ⚠ That is also what makes the
        // lock correct across an unexpected death: the kernel closes the fd.
        let _ = sys::close(self.fd);
    }
}

/// Build the record a completed pull writes.
#[allow(clippy::too_many_arguments)]
pub fn record_of(
    reference: &Reference,
    resolved: &Digest,
    resolved_media_type: &str,
    manifest_digest: &Digest,
    manifest: &oci::Manifest,
    config: &oci::Config,
    platform: &str,
) -> Result<Record> {
    Ok(Record {
        repository: reference.canonical_repository(),
        tag: reference.tag.clone(),
        digest: resolved.to_string(),
        digest_media_type: resolved_media_type.to_string(),
        manifest_digest: manifest_digest.to_string(),
        config_digest: manifest.config.parsed_digest()?.to_string(),
        platform: platform.to_string(),
        layers: manifest.layers.iter().map(|l| l.digest.clone()).collect(),
        stored_bytes: manifest.stored_bytes(),
        architecture: config.architecture.clone(),
        os: config.os.clone(),
        created: config.created.clone(),
        pulled_at: clock::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> Store {
        let d = std::env::temp_dir().join(format!("podbox-store-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Store::open(d).unwrap()
    }

    /// ⛔ INVARIANT I6. Two `stage` calls in ONE process must not take one
    /// name. `TODO/probe.md` T-0113 is this defect one crate over, and it was
    /// found by luck there; this is the assertion that would have found it.
    #[test]
    fn two_staging_calls_in_one_process_take_two_names() {
        let s = scratch("stage-unique");
        let (a, _fa) = s.stage("layer").unwrap();
        let (b, _fb) = s.stage("layer").unwrap();
        assert_ne!(a, b, "one process staged two blobs over one name");
        assert!(a.is_file() && b.is_file());
        let _ = std::fs::remove_dir_all(s.root());
    }

    /// ⛔ INVARIANT I5. An abandoned staging file is swept and one being
    /// written is not, and the difference is a lock rather than a pid.
    #[test]
    fn the_sweep_takes_an_abandoned_partial_and_leaves_a_held_one() {
        let s = scratch("sweep");
        // Abandoned: staged, then the handle dropped without a commit, which is
        // what a SIGKILL leaves behind.
        let (dead, handle) = s.stage("abandoned").unwrap();
        drop(handle);
        // Live: still held, exactly as a writer mid-blob holds it.
        let (live, _held) = s.stage("live").unwrap();

        let swept = s.sweep_staging();
        assert!(swept.contains(&dead), "the abandoned file was not swept");
        assert!(!dead.exists(), "the abandoned file is still there");
        assert!(live.exists(), "the sweep took a file being written");
        assert!(!swept.contains(&live));
        let _ = std::fs::remove_dir_all(s.root());
    }

    /// ⚠ And opening a store is what runs it, because every command does that
    /// and nothing else would.
    #[test]
    fn opening_a_store_sweeps_what_a_killed_process_left() {
        let d = std::env::temp_dir().join(format!("podbox-store-{}-openswp", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let s = Store::open(&d).unwrap();
        let (dead, handle) = s.stage("abandoned").unwrap();
        drop(handle);
        assert!(dead.is_file());
        let _again = Store::open(&d).unwrap();
        assert!(!dead.exists(), "a second open did not sweep");
        let _ = std::fs::remove_dir_all(&d);
    }

    fn record(repo: &str, tag: Option<&str>, seed: u8) -> Record {
        let d = |b: u8| Digest::of(&[b, seed]).to_string();
        Record {
            repository: repo.into(),
            tag: tag.map(str::to_string),
            digest: d(1),
            digest_media_type: oci::MEDIA_OCI_INDEX.into(),
            manifest_digest: d(2),
            config_digest: d(3),
            platform: "linux/amd64".into(),
            layers: vec![d(4), d(5)],
            stored_bytes: 100,
            architecture: "amd64".into(),
            os: "linux".into(),
            created: Some("2024-01-01T00:00:00Z".into()),
            pulled_at: clock::now(),
        }
    }

    #[test]
    fn a_blob_whose_bytes_do_not_match_never_reaches_the_blob_directory() {
        // ⛔ The central rule of T-0202, driven rather than asserted about.
        let s = scratch("verify");
        let claimed = Digest::of(b"alpine");
        let e = s.put_bytes(b"not alpine", &claimed, "layer").unwrap_err();
        assert!(format!("{e}").contains("digest mismatch"), "{e}");
        assert!(!s.has_blob(&claimed));
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_blob_that_matches_is_stored_under_its_digest_and_reads_back() {
        let s = scratch("roundtrip");
        let d = Digest::of(b"alpine");
        s.put_bytes(b"alpine", &d, "layer").unwrap();
        assert!(s.has_blob(&d));
        assert!(s.blob_path(&d).ends_with(d.hex()));
        assert_eq!(s.read_blob(&d).unwrap(), b"alpine");
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_stored_blob_edited_behind_podboxs_back_is_caught_on_the_way_out() {
        let s = scratch("tamper");
        let d = Digest::of(b"alpine");
        s.put_bytes(b"alpine", &d, "layer").unwrap();
        std::fs::write(s.blob_path(&d), b"tampered").unwrap();
        let e = s.read_blob(&d).unwrap_err();
        assert!(format!("{e}").contains("digest mismatch"), "{e}");
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_shared_blob_survives_removing_one_of_the_two_images_that_reach_it() {
        // ⭐ Reachability is computed over what SURVIVES. Computing it over the
        // doomed set deletes a shared layer with the first image that goes.
        let s = scratch("shared");
        let a = record("docker.io/library/alpine", Some("3.20"), 7);
        let mut b = record("docker.io/library/alpine", Some("3.21"), 7);
        b.digest = Digest::of(b"another index").to_string();
        for r in [&a, &b] {
            for blob in r.blobs() {
                let d = Digest::parse(blob).unwrap();
                s.put_bytes(blob.as_bytes(), &Digest::of(blob.as_bytes()), "x")
                    .ok();
                std::fs::write(s.blob_path(&d), b"payload").unwrap();
            }
            s.put_record(r.clone()).unwrap();
        }
        let shared = Digest::parse(&a.layers[0]).unwrap();
        s.remove("alpine:3.20").unwrap();
        assert!(s.has_blob(&shared), "a layer 3.21 still needs was deleted");
        s.remove("alpine:3.21").unwrap();
        assert!(!s.has_blob(&shared), "the last reference did not free it");
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn an_image_a_container_holds_is_refused_by_rmi_and_skipped_by_prune() {
        // ⭐ T-0204's acceptance, minus the container: the lock is the whole
        // mechanism and it is driven here through the shipping functions.
        let s = scratch("inuse");
        let r = record("docker.io/library/alpine", Some("latest"), 9);
        s.put_record(r.clone()).unwrap();
        assert!(!s.in_use(&r).unwrap());

        let held = s.hold(&r).unwrap();
        assert!(s.in_use(&r).unwrap());
        let e = s.remove("alpine:latest").unwrap_err();
        assert!(format!("{e}").contains("in use"), "{e}");
        let pruned = s.prune(true).unwrap();
        assert_eq!(pruned.skipped, vec!["alpine:latest".to_string()]);
        assert!(pruned.untagged.is_empty());

        drop(held);
        assert!(!s.in_use(&r).unwrap());
        assert!(s.remove("alpine:latest").is_ok());
        let _ = std::fs::remove_dir_all(s.root());
    }

    /// ⭐ T-0211's plant. This is the defect that surfaced as an intermittent
    /// failure of the test above at 2 runs of 6 of the full workspace, and it is
    /// made to fail on demand here rather than once a week.
    ///
    /// ⛔ Red before the fix: with the image lock opened without `O_CLOEXEC`,
    /// the forked child below inherits the fd, the `flock` outlives
    /// `drop(held)`, and `in_use` answers true for an image nothing is using.
    /// The forking thread in the real suite is `probe_cache`'s, whose `measure`
    /// makes one fresh child per probe; the `clone_fork` here is that, reduced
    /// to the one call that matters.
    #[test]
    fn a_fork_while_the_lock_is_held_does_not_extend_it() {
        let _serialised = FORKING_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let s = scratch("forkhold");
        let r = record("docker.io/library/alpine", Some("latest"), 17);
        s.put_record(r.clone()).unwrap();

        let held = s.hold(&r).unwrap();
        assert!(s.in_use(&r).unwrap(), "the holder itself did not register");

        // ⛔ A pipe, not a sleep. `clone_fork` sheds the registered fds in the
        // child before it returns there, so the child is only known to have
        // shed once it has run at all, and a parent that asserts before the
        // child is scheduled reads the fd as still open and fails for a reason
        // that has nothing to do with the defect. That is the same shape of
        // intermittent failure T-0211 itself arrived as, so this test is made
        // to wait for the fact rather than for a duration.
        let mut fds = [0i32; 2];
        sys::pipe2(&mut fds, 0).unwrap();
        let (r_fd, w_fd) = (fds[0] as i64, fds[1] as i64);

        // ⚠ `clone_fork` and not `std::process::Command`: the defect is about
        // what a bare `fork` inherits, and spawning a process would exec and so
        // hide it behind the `FD_CLOEXEC` the other test covers.
        let pid = unsafe { sys::clone_fork(sys::SIGCHLD) }.unwrap();
        if pid == 0 {
            // ---- child. It has already shed; say so, then outlive the drop.
            let _ = sys::write(w_fd, b"x");
            std::thread::sleep(std::time::Duration::from_millis(400));
            // ⛔ Never returns into the test harness: exits without unwinding.
            sys::exit_group(0);
        }
        let _ = sys::close(w_fd);
        let mut ack = [0u8; 1];
        assert_eq!(sys::read(r_fd, &mut ack).unwrap(), 1, "the child never ran");

        drop(held);
        let free = s.in_use(&r).map(|u| !u);

        let mut status = 0;
        let _ = sys::close(r_fd);
        let _ = sys::wait4(pid, &mut status);
        assert!(
            free.unwrap(),
            "the holder released the lock and it is still held: a forked child \
             inherited the fd, which is TODO/image.md T-0211"
        );
        let _ = std::fs::remove_dir_all(s.root());
    }

    /// ⛔ **THE TWO FORK TESTS BELOW MAY NOT RUN AT THE SAME TIME**, and this is
    /// what stops them.
    ///
    /// `cargo test` runs tests in THREADS of one process. A bare `fork` copies
    /// every open descriptor of that process, including a lock another test is
    /// holding in another thread; that child then holds the lock for as long as
    /// it lives, and the other test's assertion -- "the holder released it and
    /// nobody else has it" -- fails for a reason that has nothing to do with its
    /// subject. Measured on 2026-09-09: `a_spawned_process_does_not_inherit_the_lock`
    /// failed once in a full-workspace run and passed alone and on the retry,
    /// which is the shape a flake takes and is not one.
    ///
    /// ⚠ It is the same trap as T-0603's, which counted `/proc/self/task`
    /// before and after a spawn and failed about one run in five.
    static FORKING_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// ⭐ T-0211's second half, and it is a **different** failure from the one
    /// above rather than the same one written twice.
    ///
    /// `clone_fork` sheds registered fds, so nothing podbox forks itself can
    /// carry a lock away. ⛔ `std::process::Command` forks inside libstd and
    /// never passes through `clone_fork`, so the shed list cannot reach it and
    /// `O_CLOEXEC` on the descriptor is the only thing that does. Reverting
    /// either defence turns exactly one of these two tests red, which is how it
    /// was confirmed they are independent.
    #[test]
    fn a_spawned_process_does_not_inherit_the_lock() {
        let _serialised = FORKING_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let s = scratch("spawnhold");
        let r = record("docker.io/library/alpine", Some("latest"), 19);
        s.put_record(r.clone()).unwrap();

        let held = s.hold(&r).unwrap();
        // ⛔ The child announces itself on stdout and the parent reads that
        // before asserting. `O_CLOEXEC` takes the fd away at the **exec**, so a
        // parent that asserts while the child is still between `fork` and
        // `execve` measures the fork window instead, which is the other test's
        // subject, and would make this one fail for the wrong reason.
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("echo ready; sleep 2")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("/bin/sh");
        let mut line = [0u8; 6];
        {
            use std::io::Read;
            child
                .stdout
                .as_mut()
                .unwrap()
                .read_exact(&mut line)
                .expect("the child never reached its exec");
        }
        assert_eq!(&line, b"ready\n");

        drop(held);
        let free = s.in_use(&r).map(|u| !u);
        // ⚠ Reaped before the assertion, so a failure does not also leave a
        // process behind for whatever runs next.
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            free.unwrap(),
            "the holder released the lock and a spawned process is still \
             holding it: the lock fd was not O_CLOEXEC, which is the exec half \
             of TODO/image.md T-0211"
        );
        let _ = std::fs::remove_dir_all(s.root());
    }

    /// ⭐ The multi-architecture case, and the one a platform-blind key gets
    /// wrong silently: the second pull deletes the first, its blobs are
    /// collected, and a caller who pulled both is left with one.
    /// ⭐ The door sweep's finding, and it is the same defect as `find_for`'s
    /// through a **different** door: `extract` and `inspect` reach the store by
    /// `find_one`, which took `.next()`.
    #[test]
    fn an_ambiguous_reference_is_refused_by_name_and_never_by_position() {
        let s = scratch("ambig");
        for (plat, arch, n) in [("linux/amd64", "amd64", 31), ("linux/arm64", "arm64", 33)] {
            let mut r = record("docker.io/library/alpine", Some("latest"), n);
            r.platform = plat.into();
            r.architecture = arch.into();
            s.put_record(r).unwrap();
        }

        // ⚠ No platform asked for: the HOST's is preferred, and there is
        // exactly one of those, so this is not ambiguous.
        let got = s.find_one("alpine:latest").unwrap();
        assert_eq!(got.platform, crate::platform::Platform::host().to_string());

        // ⛔ A platform the store does not hold is refused NAMING what it does.
        let e = s
            .find_one_for(
                "alpine:latest",
                Some(&crate::platform::Platform::parse("linux/riscv64").unwrap()),
            )
            .unwrap_err();
        let text = format!("{e}");
        assert!(text.contains("linux/amd64"), "{text}");
        assert!(text.contains("linux/arm64"), "{text}");

        // ⛔ And where the host's platform is not among them either, it refuses
        // rather than taking the first.
        let solo = scratch("ambig2");
        for (plat, arch, n) in [
            ("linux/riscv64", "riscv64", 35),
            ("linux/s390x", "s390x", 37),
        ] {
            let mut r = record("docker.io/library/alpine", Some("latest"), n);
            r.platform = plat.into();
            r.architecture = arch.into();
            solo.put_record(r).unwrap();
        }
        let e = solo.find_one("alpine:latest").unwrap_err();
        let text = format!("{e}");
        assert!(text.contains("will not pick one by position"), "{text}");
        assert!(text.contains("--platform"), "{text}");

        let _ = std::fs::remove_dir_all(s.root());
        let _ = std::fs::remove_dir_all(solo.root());
    }

    #[test]
    fn two_platforms_of_one_tag_are_two_images_and_not_one() {
        let s = scratch("twoplat");
        let mut amd = record("docker.io/library/alpine", Some("latest"), 21);
        amd.platform = "linux/amd64".into();
        amd.architecture = "amd64".into();
        let mut arm = record("docker.io/library/alpine", Some("latest"), 23);
        arm.platform = "linux/arm64".into();
        arm.architecture = "arm64".into();

        s.put_record(amd.clone()).unwrap();
        s.put_record(arm.clone()).unwrap();
        assert_eq!(
            s.find("alpine:latest").unwrap().len(),
            2,
            "the second pull replaced the first: the platform is not in the key"
        );

        // ⛔ Re-pulling ONE platform replaces only that one. A moving tag is
        // still the normal case and this must not accumulate.
        s.put_record(amd.clone()).unwrap();
        assert_eq!(s.find("alpine:latest").unwrap().len(), 2);

        // Asked for a platform: exactly that one comes back, by name.
        let p = crate::platform::Platform::parse("linux/arm64").unwrap();
        let got = s.find_for("alpine:latest", Some(&p)).unwrap();
        assert_eq!(got.matched.len(), 1);
        assert_eq!(got.matched[0].platform, "linux/arm64");

        // ⚠ Asked for a platform the store does not hold: NOTHING matches, and
        // what it does hold is reported separately so the caller can say "held,
        // for another platform" rather than "no such image".
        let none = crate::platform::Platform::parse("linux/riscv64").unwrap();
        let miss = s.find_for("alpine:latest", Some(&none)).unwrap();
        assert!(
            miss.matched.is_empty(),
            "a wrong-platform record was returned"
        );
        assert_eq!(miss.other_platforms.len(), 2);

        // ⛔ The regression that produced `Found`: with ONE record in the store
        // and a different platform asked for, a count-based short circuit
        // returned it and podbox ran the wrong architecture in silence.
        let solo = scratch("solo");
        let mut only = record("docker.io/library/alpine", Some("latest"), 29);
        only.platform = "linux/amd64".into();
        solo.put_record(only).unwrap();
        let arm = crate::platform::Platform::parse("linux/arm64").unwrap();
        let got = solo.find_for("alpine:latest", Some(&arm)).unwrap();
        assert!(
            got.matched.is_empty(),
            "one record in the store was returned for a platform it is not"
        );
        assert_eq!(got.other_platforms, vec!["linux/amd64".to_string()]);
        let _ = std::fs::remove_dir_all(solo.root());

        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn two_holders_of_one_image_both_have_to_go_before_it_is_free() {
        let s = scratch("twohold");
        let r = record("docker.io/library/alpine", Some("latest"), 11);
        s.put_record(r.clone()).unwrap();
        let a = s.hold(&r).unwrap();
        let b = s.hold(&r).unwrap();
        assert!(s.in_use(&r).unwrap());
        drop(a);
        assert!(s.in_use(&r).unwrap(), "one holder left and it read as free");
        drop(b);
        assert!(!s.in_use(&r).unwrap());
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_tag_points_at_the_same_manifest_without_copying_a_blob() {
        let s = scratch("tag");
        let r = record("docker.io/library/alpine", Some("latest"), 13);
        s.put_record(r.clone()).unwrap();
        let tagged = s.tag("alpine:latest", "myalpine:v1").unwrap();
        assert_eq!(tagged.digest, r.digest);
        // ⚠ docker's normalisation, not a shortcut: a single-component name
        // is a hub `library/` repository whichever verb produced it, and
        // `display_repository` is what turns it back into `myalpine`.
        assert_eq!(tagged.repository, "docker.io/library/myalpine");
        assert_eq!(tagged.display_repository(), "myalpine");
        assert_eq!(s.list().unwrap().len(), 2);
        // Removing one name leaves the other and its blobs.
        s.remove("alpine:latest").unwrap();
        assert_eq!(s.find("myalpine:v1").unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn tagging_something_a_digest_names_is_refused() {
        let s = scratch("tagdigest");
        s.put_record(record("docker.io/library/alpine", Some("latest"), 17))
            .unwrap();
        let e = s
            .tag("alpine:latest", &format!("x@sha256:{}", "0".repeat(64)))
            .unwrap_err();
        // ⚠ 1 and not 125: an invalid reference is refused AFTER the flags
        // parsed, and docker's own code for that is 1 (T-0802).
        assert_eq!(e.exit_code(), crate::error::EXIT_CLI_ERROR);
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn re_pulling_one_tag_replaces_its_record_rather_than_adding_a_second() {
        let s = scratch("retag");
        s.put_record(record("docker.io/library/alpine", Some("latest"), 19))
            .unwrap();
        let mut moved = record("docker.io/library/alpine", Some("latest"), 19);
        moved.digest = Digest::of(b"the tag moved").to_string();
        s.put_record(moved.clone()).unwrap();
        let got = s.find("alpine:latest").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].digest, moved.digest);
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn an_image_is_found_by_tag_by_digest_and_by_docker_short_id() {
        let s = scratch("find");
        let r = record("docker.io/library/alpine", Some("latest"), 23);
        s.put_record(r.clone()).unwrap();
        assert_eq!(s.find("alpine").unwrap().len(), 1);
        assert_eq!(s.find("alpine:latest").unwrap().len(), 1);
        assert_eq!(s.find(&format!("alpine@{}", r.digest)).unwrap().len(), 1);
        let short = &r.config_digest.trim_start_matches("sha256:")[..12];
        assert_eq!(s.find(short).unwrap().len(), 1);
        assert!(s.find("alpine:3.20").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn an_index_written_by_a_later_podbox_is_refused_rather_than_reinterpreted() {
        let d = std::env::temp_dir().join(format!("podbox-store-{}-future", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join(INDEX_FILE),
            format!("{{\"podbox_store\":{},\"images\":[]}}", INDEX_VERSION + 1),
        )
        .unwrap();
        let e = Store::open(&d).unwrap_err();
        assert!(format!("{e}").contains("newer podbox"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_store_root_prefers_podbox_store_and_never_falls_to_tmpdir_silently() {
        // ⚠ The corpus's shipped default is `env::temp_dir()` with no space
        // check, at
        // `references/VHSgunzo__memfd-exec/tree/src/executable.rs:580-584`.
        // podbox asks the caller instead, and the caller ranks by free space.
        let got = Store::default_root(|| Some("/from-the-probe".into())).unwrap();
        assert!(
            got.starts_with(
                std::env::var_os("PODBOX_STORE")
                    .map(PathBuf::from)
                    .unwrap_or(PathBuf::from(std::env::var_os("HOME").unwrap_or_default()))
            ) || got.starts_with("/from-the-probe"),
            "{}",
            got.display()
        );
    }
}
