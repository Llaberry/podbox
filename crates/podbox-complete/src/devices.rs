//! [`TODO/complete.md`](../../../TODO/complete.md) T-0401: device shims as
//! regular files.
//!
//! ⭐ **The trap this exists for is silent and it fills the disk.** `mknod` is
//! denied on the runtimes podbox targets, so an extracted rootfs has no
//! `/dev/null`. The first shell redirection into it then **creates a growing
//! regular file**, and the container fills its own filesystem while every
//! command in it reports success. The same capture is in the corpus, against
//! `ruri`: `references/Azathothas__container-research/tree/verification/README.md:126`.
//!
//! ⛔ **A regular file, never a fifo.** A fifo blocks a writer with no reader,
//! which turns `cmd >/dev/null` into a hang, and a hang is the failure
//! [`TODO/RULES.md`](../../../TODO/RULES.md) section 8 exists to prevent.
//!
//! ⭐ **`mknod` IS TRIED FIRST, on every run, and the shim is the fallback.**
//! Probed by calling it rather than inferred: this runtime hands a process every
//! capability bit and denies the operation anyway, so the capability says
//! nothing. On a machine that permits `mknod` the payload gets a real
//! `/dev/null` and the row reads `Native`; where the kernel refuses, the errno
//! it refused with is in the banner beside the shim. ⛔ A shim reported on a
//! machine that did not need one is a degradation podbox invented, which is the
//! same class of wrong as hiding one.
//!
//! ⛔ **Every shim is reported as a shim.** These are not devices:
//!
//! | path | the shim | what it is not |
//! | --- | --- | --- |
//! | `/dev/null` | an empty regular file, truncated on every run | writes are kept until the next run, not discarded |
//! | `/dev/zero` | [`FILLED_BYTES`] of `0x00` | it ends at EOF |
//! | `/dev/urandom` | [`FILLED_BYTES`] read once from the host's `getrandom(2)` | it ends at EOF, and re-reading gives the same bytes |
//! | `/dev/random` | the same | the same |
//!
//! ⚠ `/dev/random` is not named in T-0401, which names `null`, `zero` and
//! `urandom`. It is here because its shape and its failure are identical and
//! its absence is the same silent one; the entry records the addition.
//!
//! ⚠ `getrandom(2)` already covers TLS and crypto in every modern libc, so
//! `/dev/urandom` is a compatibility shim and not an entropy source. Anything
//! that needs entropy and reads this file instead of calling `getrandom` is
//! reading a file, and the banner says so.

use crate::write::{Kind, Root, Wrote};
use crate::{Action, Fixup, Report, Result};

/// How much `/dev/zero`, `/dev/urandom` and `/dev/random` hold.
///
/// ⚠ A number, so it is finite, and 1 MiB rather than more because it is paid
/// per extracted rootfs and a payload that reads more than this from a shim is
/// a payload the shim was never going to satisfy.
pub const FILLED_BYTES: usize = 1024 * 1024;

/// Asked for beyond the three shims themselves, so the completion layer does
/// not fill a rootfs to its last block and leave the payload nothing.
pub const SHIM_HEADROOM_BYTES: u64 = 4 * 1024 * 1024;

/// Is the right character device already at this path?
///
/// ⚠ The NUMBERS are checked, not just the type. A node that is there and is
/// `1:5` where `1:3` was wanted is a wrong answer rather than a present one,
/// and a payload that stats it reads those numbers.
fn already_right(k: &Kind, major: u64, minor: u64) -> bool {
    matches!(k, Kind::CharDev { major: a, minor: b } if *a == major && *b == minor)
}

pub(crate) fn apply(root: &Root, r: &mut Report) {
    crate::record(r, "T-0401", "dev-shim", run(root));
}

/// The four devices, with the numbers the kernel assigns them.
///
/// ⚠ `major:minor` is a kernel constant and not a choice: `1:3` is `/dev/null`
/// on every Linux there is, and a payload that stats the node reads them.
const DEVICES: &[(&str, u64, u64)] = &[
    ("dev/null", 1, 3),
    ("dev/zero", 1, 5),
    ("dev/random", 1, 8),
    ("dev/urandom", 1, 9),
];

fn run(root: &Root) -> Result<Vec<Fixup>> {
    let mut out = Vec::new();
    root.mkdirs("dev")?;

    // ⚠ Drawn ONCE and used for both random shims, which is honest: they are
    // two names for one file of bytes, and drawing twice would imply a property
    // neither has. ⛔ Not drawn at all unless a shim is actually needed: a
    // megabyte of entropy per container start is a cost, and on a machine that
    // permits `mknod` there is nothing to fill.
    let mut filler: Option<(Vec<u8>, Vec<u8>)> = None;

    for (path, major, minor) in DEVICES {
        let k = root.kind(path)?;

        // 1. the right node is already there -- the image shipped one, or an
        // earlier run of this fixup made it. ⛔ Not degraded: it is the real
        // thing, whoever put it there.
        if already_right(&k, *major, *minor) {
            out.push(
                Fixup::new("T-0401", "dev-node", path, Action::Unchanged).why(format!(
                    "a real character device {major}:{minor} is already here"
                )),
            );
            continue;
        }

        // 2. ⭐ ASK THE KERNEL. On a machine that permits `mknod` the payload
        // gets the real thing and this is not a degradation at all.
        match root.mknod_char(path, 0o666, *major, *minor)? {
            Ok(()) => {
                out.push(
                    Fixup::new("T-0401", "dev-node", path, Action::Created).why(format!(
                        "a real character device {major}:{minor}. This machine permits \
                         mknod(2), so podbox made the node rather than a shim"
                    )),
                );
                continue;
            }
            Err(why) => {
                // 3. the wall this entry exists for. Fall through to the shim,
                // carrying the errno the kernel refused with.
                // ⛔ RULES.md section 8 and T-0806: `statvfs` before a large
                // write, and the error names the destination. Three shims of
                // one MiB each into a rootfs on a small tmpfs is exactly the
                // write that fills it, and the failure would otherwise arrive
                // as a truncated `/dev/zero` nothing reports.
                if filler.is_none() {
                    podbox_image::space::require(
                        root.path(),
                        "the /dev shims T-0401 falls back to",
                        (FILLED_BYTES * 3) as u64,
                        4,
                        SHIM_HEADROOM_BYTES,
                    )
                    .map_err(|e| crate::Error::Complete(format!("{e}")))?;
                }
                let (zeros, rand) = filler.get_or_insert_with(|| {
                    (
                        vec![0u8; FILLED_BYTES],
                        random_bytes(FILLED_BYTES).unwrap_or_else(|_| vec![0u8; FILLED_BYTES]),
                    )
                });
                out.extend(shim_for(root, path, &why, zeros, rand)?);
            }
        }
    }
    Ok(out)
}

/// The regular-file shim for one device, once `mknod` has refused.
fn shim_for(root: &Root, path: &str, why: &str, zeros: &[u8], rand: &[u8]) -> Result<Vec<Fixup>> {
    if path == "dev/null" {
        // ⭐ Truncated on EVERY run, not created once. The failure this fixup
        // is for leaves a multi-gigabyte regular file behind, and a rootfs is
        // shared between containers, so the second container inherits the
        // first one's spill unless this runs every time.
        let grew = match root.kind(path)? {
            Kind::Regular { len, .. } if len > 0 => Some(len),
            _ => None,
        };
        let w = root.write(path, b"", 0o666)?;
        let detail = match grew {
            Some(len) => format!(
                "mknod(2) refused with {why}, so this is a regular file, and it was \
                 {len} bytes: a payload had already redirected into it. Writes are \
                 kept until the next run, not discarded"
            ),
            None => format!(
                "mknod(2) refused with {why}, so this is a regular file: writes are \
                 kept until the next run rather than discarded, and reads give EOF"
            ),
        };
        return Ok(vec![shim(path, w, detail)]);
    }
    let (bytes, what) = if path == "dev/zero" {
        (zeros, "zero bytes")
    } else {
        (rand, "bytes read once from the host's getrandom(2)")
    };
    filled(root, path, bytes, what, why)
}

fn filled(root: &Root, path: &str, bytes: &[u8], what: &str, why: &str) -> Result<Vec<Fixup>> {
    // ⚠ A shim already the right length is left alone: rewriting a megabyte on
    // every container start is a cost with no reading behind it.
    if let Kind::Regular { len, .. } = root.kind(path)? {
        if len as usize == bytes.len() {
            return Ok(vec![Fixup::new(
                "T-0401",
                "dev-shim",
                path,
                Action::Unchanged,
            )
            .why(format!("already a {} byte shim", bytes.len()))
            .degraded()]);
        }
    }
    let w = root.write(path, bytes, 0o666)?;
    Ok(vec![shim(
        path,
        w,
        format!(
            "mknod(2) refused with {why}, so this is a regular file of {} {what}. \
             ⛔ It ENDS: a read past {} bytes gives EOF where the device would not",
            bytes.len(),
            bytes.len()
        ),
    )])
}

fn shim(path: &str, w: Wrote, detail: impl Into<String>) -> Fixup {
    Fixup::new("T-0401", "dev-shim", path, crate::act(w))
        .why(detail)
        .degraded()
}

/// `getrandom(2)`, in bounded chunks.
///
/// ⛔ **Never `std::fs::read("/dev/urandom")`.** That call has no EOF: it
/// allocated 13 GB here before the OOM killer took the process
/// ([`TODO/supervise.md`](../../../TODO/supervise.md) T-0602). The shim for a
/// device is not built by reading the device.
fn random_bytes(n: usize) -> Result<Vec<u8>> {
    let mut out = vec![0u8; n];
    let mut off = 0;
    while off < n {
        // ⚠ The kernel caps one getrandom at 32 MiB and can return short; the
        // loop is the contract, not a precaution.
        match podbox_probe::sys::getrandom(&mut out[off..]) {
            Ok(0) => break,
            Ok(k) => off += k as usize,
            Err(e) if e == podbox_probe::sys::EINTR => continue,
            Err(e) => {
                return Err(crate::Error::Complete(format!(
                    "getrandom(2) for the /dev/urandom shim: {}",
                    e.name()
                )))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        let p = std::env::temp_dir().join(format!("podbox-dev-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    fn zeros() -> Vec<u8> {
        vec![0u8; FILLED_BYTES]
    }

    /// ⭐ The failure T-0401 is for, driven end to end against the SHIM ARM.
    ///
    /// ⛔ [`shim_for`] rather than [`run`], deliberately: `run` asks the kernel
    /// whether `mknod` works, and on a machine that permits it there is no shim
    /// to assert on. A test that drove `run` here would measure the host and
    /// report the host's answer as the code's.
    #[test]
    fn a_grown_dev_null_is_truncated_and_the_growth_is_reported() {
        let d = scratch("grown");
        std::fs::create_dir_all(format!("{d}/dev")).unwrap();
        std::fs::write(format!("{d}/dev/null"), vec![b'x'; 4096]).unwrap();
        let root = Root::open(&d).unwrap();
        let fs = shim_for(&root, "dev/null", "EPERM", &zeros(), &zeros()).unwrap();
        assert_eq!(fs[0].action, Action::Rewrote);
        assert!(fs[0].detail.contains("4096 bytes"), "{}", fs[0].detail);
        assert!(fs[0].detail.contains("EPERM"), "{}", fs[0].detail);
        assert_eq!(std::fs::metadata(format!("{d}/dev/null")).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ Every shim is marked degraded, without exception, and every shim's
    /// line carries the errno `mknod` refused with. A shim that reads as
    /// `Native` is the class of lie this project exists to refuse.
    #[test]
    fn every_shim_is_reported_as_a_degradation_and_names_the_errno() {
        let d = scratch("degraded");
        let root = Root::open(&d).unwrap();
        root.mkdirs("dev").unwrap();
        let (z, r) = (zeros(), zeros());
        for (path, ..) in DEVICES {
            let fs = shim_for(&root, path, "EPERM", &z, &r).unwrap();
            assert_eq!(fs.len(), 1, "{path}");
            assert!(fs[0].degraded, "{path}: {:?}", fs[0]);
            assert!(fs[0].detail.contains("EPERM"), "{path}: {:?}", fs[0]);
        }
        assert_eq!(
            std::fs::metadata(format!("{d}/dev/zero")).unwrap().len() as usize,
            FILLED_BYTES
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⚠ The second pass must not rewrite a megabyte for nothing.
    #[test]
    fn a_second_pass_leaves_the_filled_shims_alone() {
        let d = scratch("idem");
        let root = Root::open(&d).unwrap();
        root.mkdirs("dev").unwrap();
        let (z, r) = (zeros(), zeros());
        for p in ["dev/zero", "dev/urandom", "dev/random"] {
            shim_for(&root, p, "EPERM", &z, &r).unwrap();
            let fs = shim_for(&root, p, "EPERM", &z, &r).unwrap();
            assert_eq!(fs[0].action, Action::Unchanged, "{p}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⭐ **The property that holds on EVERY machine**, whichever arm this one
    /// takes: a row claims a real device only where a real device is on disk,
    /// and a row that is a file is marked degraded. ⛔ This is the assertion
    /// that catches the two ways T-0401 can be wrong -- a shim presented as a
    /// device, and a device presented as a shim -- and it needs no knowledge of
    /// whether this host permits `mknod`.
    #[test]
    fn what_each_row_claims_is_what_is_on_disk() {
        use std::os::unix::fs::FileTypeExt;
        let d = scratch("claims");
        let root = Root::open(&d).unwrap();
        let fs = run(&root).unwrap();
        assert_eq!(fs.len(), DEVICES.len(), "{fs:#?}");
        for f in &fs {
            let md = std::fs::metadata(format!("{d}/{}", f.path)).unwrap();
            let is_dev = md.file_type().is_char_device();
            match f.id {
                "dev-node" => {
                    assert!(is_dev, "{f:?} claims a node and {} is not one", f.path);
                    assert!(!f.degraded, "{f:?} is a real device and is marked degraded");
                }
                "dev-shim" => {
                    assert!(!is_dev, "{f:?} claims a shim and {} is a device", f.path);
                    assert!(f.degraded, "{f:?} is a shim and is not marked degraded");
                }
                other => panic!("unknown fixup id {other:?}"),
            }
        }
        // ⚠ And running it again changes nothing, on either arm.
        for f in run(&root).unwrap() {
            assert_eq!(f.action, Action::Unchanged, "{f:?}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
