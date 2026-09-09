//! `TODO/probe.md` T-0111: `$store/probe.json`, keyed on what actually decides
//! the verdict.
//!
//! ⛔ **A cache keyed wrongly is worse than no cache.** `TOOL.md` section 6.1
//! says to key on the boot id and re-probe when it changes. Measured on
//! 2026-09-08, one command in three places: the host, `experiments/20-enter-target.sh`
//! and a plain docker container all read
//! `01703f0e-380a-462e-ab15-36460e5a8be7` from
//! `/proc/sys/kernel/random/boot_id`, and the first two produce `namespace` and
//! `chroot` respectively. The boot id is the **kernel's**, so it cannot tell
//! them apart, and a cache keyed on it alone would serve the host's verdict to
//! a confined process. That is podbox telling the exact lie it exists to refuse.
//!
//! ⭐ **The cache stores what `--json` prints, verbatim.** `podbox-probe`'s
//! `report::document` is the one writer and the key travels inside the
//! document, so there is no second serializer to drift against the first.
//!
//! ⛔ **A key that does not match is not partially refreshed.** A partial
//! refresh means two halves of one answer measured under two confinements,
//! which is the class of defect `TODO/probe.md` T-0109 is about.

use std::path::PathBuf;

use podbox_probe::identity::ConfinementKey;
use podbox_probe::select::Selection;
use podbox_probe::{identity, report};

use crate::error::{Error, Result};
use crate::store::Store;

pub const CACHE_FILE: &str = "probe.json";

/// Where a probe answer came from, so the caller can say so rather than imply
/// a measurement that did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Served from `$store/probe.json`, whose key matched.
    Cache,
    /// Measured now. `why` says what was wrong with the cache, and is what the
    /// evidence line prints.
    Measured(String),
}

pub struct Probe {
    pub document: String,
    pub source: Source,
    pub path: PathBuf,
    /// ⚠ `Some` when the fresh document could not be written back. The probe
    /// still answered; the cache simply did not persist, and that is reported
    /// rather than swallowed.
    pub not_written: Option<String>,
}

impl Probe {
    /// The selected rung, read back out of the document. `None` where the
    /// document has no `rung`, which a podbox-written one always does.
    pub fn rung(&self) -> Option<String> {
        field(&self.document, "rung")
    }

    /// `(path, source)` for every path the probe **obtained by writing**.
    /// T-0203 ranks these by free space to choose a destination.
    pub fn writable(&self) -> Vec<(String, String)> {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&self.document) else {
            return Vec::new();
        };
        v.get("writable")
            .and_then(|w| w.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.get("writable").and_then(|b| b.as_bool()) == Some(true))
                    .filter_map(|e| {
                        Some((
                            e.get("path")?.as_str()?.to_string(),
                            e.get("source")
                                .and_then(|s| s.as_str())
                                .unwrap_or("the probe")
                                .to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Measure the probe set now, without touching the store.
pub fn measure(why: impl Into<String>) -> (String, Source) {
    let findings = podbox_probe::run();
    let selection = Selection::choose(&findings);
    (
        report::document(&findings, &selection),
        Source::Measured(why.into()),
    )
}

/// The hot path: serve the cache when its key matches, and measure and rewrite
/// when it does not.
pub fn resolve(store: &Store) -> Probe {
    let path = store.root().join(CACHE_FILE);
    let live = identity::confinement_key(&identity::read());

    let why = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Some(format!("{} does not exist yet", path.display()))
        }
        Err(e) => Some(format!("{} could not be read: {e}", path.display())),
        Ok(text) => match key_of(&text) {
            None => Some(format!(
                "{} carries no cache_key, so nothing about it can be validated",
                path.display()
            )),
            Some(cached) => {
                if !live.is_complete() {
                    // ⛔ An unreadable component is not a matching one. Serving
                    // a cache whose key could not be established is a hit
                    // established by the absence of evidence.
                    Some(format!(
                        "this process cannot read {} of its own confinement, so no \
                         cached answer can be validated against it",
                        live.missing().join(", ")
                    ))
                } else {
                    let diff = live.differences(&cached);
                    if diff.is_empty() {
                        None
                    } else if diff == ["interpreter"] {
                        // ⛔ TODO/enter.md T-0506 point 5, and it is a DIFFERENT
                        // sentence: nothing about the confinement moved, the
                        // answer in the file was simply taken by another
                        // instrument. Calling that "the confinement changed"
                        // would send a reader looking for a namespace that did
                        // not change.
                        Some(format!(
                            "the instrument changed: {} took the cached answer and this \
                             process is {}, so that answer is about the other one",
                            cached
                                .interpreter
                                .as_deref()
                                .unwrap_or("an unrecorded instrument"),
                            live.interpreter.as_deref().unwrap_or("unrecorded"),
                        ))
                    } else {
                        Some(format!(
                            "the confinement changed: {} differ(s) from the cached run",
                            diff.join(", ")
                        ))
                    }
                }
            }
        },
    };

    match why {
        None => Probe {
            // Safe: `key_of` returned a key, so the file read succeeded.
            document: std::fs::read_to_string(&path).unwrap_or_default(),
            source: Source::Cache,
            path,
            not_written: None,
        },
        Some(why) => {
            let (document, source) = measure(why);
            let not_written = write(&path, &document).err().map(|e| e.to_string());
            Probe {
                document,
                source,
                path,
                not_written,
            }
        }
    }
}

/// ⛔ Written to a staging name and renamed over the cache, so a process killed
/// mid-write leaves the previous answer rather than a truncated document that
/// would then fail to parse on every run.
fn write(path: &std::path::Path, document: &str) -> Result<()> {
    let tmp = path.with_extension(format!("json.{}", std::process::id()));
    std::fs::write(&tmp, document).map_err(|e| Error::io(tmp.display().to_string(), e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path.display().to_string(), e))
}

/// The `cache_key` object of a document, as a [`ConfinementKey`].
///
/// ⛔ Missing keys stay `None` rather than becoming empty strings, so a
/// document written by a podbox that did not have this block cannot match.
fn key_of(document: &str) -> Option<ConfinementKey> {
    let v: serde_json::Value = serde_json::from_str(document).ok()?;
    let k = v.get("cache_key")?;
    let get = |name: &str| k.get(name).and_then(|x| x.as_str()).map(str::to_string);
    Some(ConfinementKey {
        boot_id: get("boot_id"),
        mnt_ns: get("mnt_ns"),
        uid_map: get("uid_map"),
        gid_map: get("gid_map"),
        setgroups: get("setgroups"),
        seccomp: get("seccomp"),
        seccomp_filters: get("seccomp_filters"),
        // ⛔ TODO/enter.md T-0506 point 5. A document written before podbox
        // keyed on the instrument has no `interpreter`, so it stays `None` and
        // never matches a live key, which always has one. That is the intended
        // behaviour and not a migration: an answer whose instrument is unknown
        // is exactly the answer that must not be served.
        interpreter: get("interpreter"),
    })
}

fn field(document: &str, name: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(document).ok()?;
    Some(v.get(name)?.as_str()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> Store {
        let d = std::env::temp_dir().join(format!("podbox-cache-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Store::open(d).unwrap()
    }

    #[test]
    fn the_first_run_measures_and_the_second_is_served_from_the_cache() {
        let s = scratch("hit");
        let first = resolve(&s);
        assert!(matches!(first.source, Source::Measured(_)));
        assert_eq!(first.not_written, None, "{:?}", first.not_written);
        assert!(s.root().join(CACHE_FILE).is_file());

        let second = resolve(&s);
        assert_eq!(second.source, Source::Cache);
        assert_eq!(second.rung(), first.rung());
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_cache_written_under_another_mount_namespace_is_refused_and_says_which() {
        // ⭐ The measured failure T-0111 exists for. The boot id is unchanged,
        // as it would be inside the reconstruction, and only `mnt_ns` moves.
        let s = scratch("mntns");
        let fresh = resolve(&s);
        let mut doc: serde_json::Value = serde_json::from_str(&fresh.document).unwrap();
        doc["cache_key"]["mnt_ns"] = serde_json::Value::String("mnt:[4026599999]".into());
        // Also change the verdict, so serving this cache would be visible.
        doc["rung"] = serde_json::Value::String("chroot".into());
        std::fs::write(s.root().join(CACHE_FILE), doc.to_string()).unwrap();

        let again = resolve(&s);
        match &again.source {
            Source::Measured(why) => assert!(why.contains("mnt_ns"), "{why}"),
            other => panic!("served a stale cache: {other:?}"),
        }
        assert_eq!(again.rung(), fresh.rung());
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_cache_keyed_on_the_boot_id_alone_would_not_have_caught_that() {
        // ⛔ The specification's key, stated as a test so the finding cannot be
        // lost: keying on the boot id alone calls the two runs above equal.
        let host = ConfinementKey {
            boot_id: Some("01703f0e-380a-462e-ab15-36460e5a8be7".into()),
            mnt_ns: Some("mnt:[4026531832]".into()),
            uid_map: Some("0 0 4294967295".into()),
            gid_map: Some("0 0 4294967295".into()),
            setgroups: Some("allow".into()),
            seccomp: Some("0".into()),
            seccomp_filters: Some("0".into()),
            interpreter: Some("none".into()),
        };
        let confined = ConfinementKey {
            mnt_ns: Some("mnt:[4026532567]".into()),
            seccomp: Some("2".into()),
            ..host.clone()
        };
        assert_eq!(host.boot_id, confined.boot_id);
        assert!(!host.differences(&confined).is_empty());
    }

    /// ⭐ TODO/enter.md T-0506 point 5. An answer taken by an emulator must
    /// never be served to a process that is not that emulator, and the reason
    /// must name the instrument rather than a confinement that did not move.
    #[test]
    fn an_answer_taken_by_another_instrument_is_never_served() {
        let s = scratch("interp");
        // Measure once, then rewrite only the instrument in the stored key, so
        // this test changes exactly one component and nothing else.
        let first = resolve(&s);
        assert!(matches!(first.source, Source::Measured(_)));
        let path = s.root().join(CACHE_FILE);
        let text = std::fs::read_to_string(&path).unwrap();
        let live = key_of(&text).unwrap();
        let mine = live.interpreter.clone().expect("the live key names one");
        let doctored = text.replace(
            &format!("\"interpreter\":\"{mine}\""),
            "\"interpreter\":\"/usr/bin/qemu-aarch64-static\"",
        );
        assert_ne!(doctored, text, "the document does not carry the instrument");
        std::fs::write(&path, &doctored).unwrap();

        let again = resolve(&s);
        match &again.source {
            Source::Measured(why) => {
                assert!(why.contains("the instrument changed"), "{why}");
                assert!(why.contains("qemu-aarch64-static"), "{why}");
                // ⛔ And NOT the confinement sentence: nothing about the
                // confinement moved, and sending a reader to look for that is
                // the wrong message however right the refusal is.
                assert!(!why.contains("the confinement changed"), "{why}");
            }
            other => panic!("served an answer another instrument took: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(s.root());
    }

    /// ⛔ A document written before podbox keyed on the instrument carries no
    /// `interpreter`, and must not match a live key that does. Not a migration:
    /// an answer whose instrument is unknown is exactly the one to re-measure.
    #[test]
    fn a_key_from_before_the_instrument_was_recorded_never_matches() {
        let live = identity::confinement_key(&identity::read());
        let old = ConfinementKey {
            interpreter: None,
            ..live.clone()
        };
        assert!(live.differences(&old).contains(&"interpreter"));
    }

    #[test]
    fn a_document_with_no_cache_key_is_never_served() {
        let s = scratch("nokey");
        std::fs::write(s.root().join(CACHE_FILE), r#"{"rung":"namespace"}"#).unwrap();
        let got = resolve(&s);
        match &got.source {
            Source::Measured(why) => assert!(why.contains("no cache_key"), "{why}"),
            other => panic!("served a keyless document: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_truncated_cache_is_measured_over_rather_than_failing_the_caller() {
        let s = scratch("truncated");
        std::fs::write(s.root().join(CACHE_FILE), "{\"rung\": \"nam").unwrap();
        let got = resolve(&s);
        assert!(matches!(got.source, Source::Measured(_)));
        assert!(got.rung().is_some());
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn the_writable_set_comes_out_of_the_document_the_probe_wrote() {
        let s = scratch("writable");
        let got = resolve(&s);
        let paths: Vec<String> = got.writable().into_iter().map(|(p, _)| p).collect();
        assert!(paths.iter().any(|p| p == "/tmp"), "{paths:?}");
        let _ = std::fs::remove_dir_all(s.root());
    }

    #[test]
    fn a_store_that_cannot_be_written_still_answers_and_says_it_did_not_persist() {
        // ⛔ "Could not run" is a third state: the probe answered, the cache
        // did not persist, and neither reads as the other.
        let d = std::env::temp_dir().join(format!("podbox-cache-{}-ro", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let s = Store::open(&d).unwrap();
        let mut perms = std::fs::metadata(&d).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o500);
        std::fs::set_permissions(&d, perms).unwrap();
        let got = resolve(&s);
        assert!(got.rung().is_some(), "the probe did not answer");
        // ⚠ uid 0 ignores the mode bits above, so the assertion that a
        // read-only store refuses a write is conditional on not being root.
        // A test that cannot fail is not evidence, and one that asserts the
        // wrong thing under the session's own uid is worse.
        if podbox_probe::identity::read().uid != 0 {
            assert!(got.not_written.is_some(), "a read-only store wrote anyway");
        }
        let mut perms = std::fs::metadata(&d).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        let _ = std::fs::set_permissions(&d, perms);
        let _ = std::fs::remove_dir_all(&d);
    }
}
