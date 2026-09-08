//! `TODO/extract.md` T-0304 and T-0305: an entry lands inside the destination
//! or it does not land at all.
//!
//! ⭐ **Two halves, and the corpus carries one of them.**
//! `references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:19-48`
//! is `symlink_target_within_root`, a purely lexical check whose own comment
//! says why it is lexical: "so it cannot be defeated by filesystem races".
//! Beside it,
//! `references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:10-12`
//! rejects `""`, `.`, `..`, `/` and NUL as a path component.
//!
//! ⛔ **That is only half of what M2 acceptance 3 needs, and the missing half
//! is the attack.** onelf validates a symlink entry when the symlink is
//! created. It does not validate a later regular entry whose path *traverses*
//! that symlink. The crafted layer writes `evil -> /etc` as its first entry and
//! `evil/passwd` as its second: both are lexically inside the destination, and
//! the second lands outside it.
//!
//! So this module does both:
//!
//! 1. every entry's path is checked **lexically**, component by component,
//!    which is what catches `..` and an absolute path with no syscall at all;
//! 2. every entry is then opened **against the accumulated tree** with
//!    `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`, which is what catches
//!    the traversal, and the kernel enforces it rather than this code.
//!
//! ⚠ `openat2(2)` is Linux 5.6. Where it answers `ENOSYS` the same walk is done
//! by hand with `O_NOFOLLOW` on every component while holding a directory
//! descriptor, which is equivalent and slower. ⛔ The fallback is entered only
//! on `ENOSYS` or `E2BIG`, never on a refusal: a kernel that says "no" is
//! answering, and retrying an answer with a weaker mechanism is how a safety
//! check becomes a formality.

use podbox_probe::sys::{self, CBuf, Errno};

use crate::error::{Error, Result};

/// Why a path was refused. Each names the entry and, at the call site, the
/// layer digest, because "refused an entry" without either is a message nobody
/// can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Absolute,
    Empty,
    Component(String),
    NulByte,
    /// The kernel refused the resolution. This is the traversal case.
    Escapes(String),
}

impl Refusal {
    pub fn why(&self) -> String {
        match self {
            Refusal::Absolute => "it is absolute, so it names a path outside the \
                                  destination rather than inside it"
                .into(),
            Refusal::Empty => "it is empty".into(),
            Refusal::Component(c) => format!(
                "the component {c:?} is not a name: `.` and `..` move within the \
                 tree rather than naming a place in it"
            ),
            Refusal::NulByte => "it contains a NUL, which the kernel would \
                                 silently truncate, so the path written would \
                                 not be the path the layer named"
                .into(),
            Refusal::Escapes(e) => format!(
                "resolving it against what the layers have written so far leaves \
                 the destination ({e}). A symlink an earlier entry created is \
                 the usual cause, and it is exactly the case a lexical check \
                 cannot see"
            ),
        }
    }
}

/// The lexical half, and it is the same rule for an entry path and for a
/// rebased symlink target.
///
/// ⛔ Returns the components rather than a boolean, so the caller cannot use
/// the original string after the check and reintroduce what was rejected.
pub fn components(path: &str) -> std::result::Result<Vec<&str>, Refusal> {
    if path.contains('\0') {
        return Err(Refusal::NulByte);
    }
    if path.starts_with('/') {
        return Err(Refusal::Absolute);
    }
    let mut out = Vec::new();
    for c in path.split('/') {
        // ⚠ A tar path routinely carries a trailing slash on a directory and
        // `./` on a prefixed archive. Neither is a defect and neither is a
        // component: they are dropped, not refused. `..` is refused.
        if c.is_empty() || c == "." {
            continue;
        }
        if c == ".." {
            return Err(Refusal::Component(c.into()));
        }
        out.push(c);
    }
    if out.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(out)
}

/// ⭐ T-0305: an absolute symlink target is **rootfs-relative**, not a refusal.
///
/// A distro rootfs is full of legitimate absolute symlinks, and the refusal
/// that must not be copied is
/// `references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:27-29`,
/// which returns false for any absolute target. Rejecting them would refuse
/// almost every real image: `experiments/results/whiteout-contract.txt` check C
/// reads `var/cache/xbps -> /var/cache/xbps` out of a real layer.
///
/// The target is interpreted relative to the rootfs, which is what it will mean
/// once the payload is inside the chroot, and the result is then containment
/// checked like anything else. ⛔ Rebase, never dereference: dereferencing at
/// extraction time bakes the build host's tree into the image.
///
/// `link_dir` is the directory the link itself sits in, as components, because
/// a relative target is resolved from there and an absolute one is not.
pub fn rebase_symlink_target(
    link_dir: &[&str],
    target: &str,
) -> std::result::Result<Vec<String>, Refusal> {
    if target.contains('\0') {
        return Err(Refusal::NulByte);
    }
    if target.is_empty() {
        return Err(Refusal::Empty);
    }
    let mut stack: Vec<String> = if target.starts_with('/') {
        // Absolute: rootfs-relative, so the walk starts at the rootfs root.
        Vec::new()
    } else {
        link_dir.iter().map(|s| (*s).to_string()).collect()
    };
    for c in target.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c == ".." {
            // ⛔ Popping past the root is the escape. `/..` is `/` on a real
            // filesystem, but a target that needs that many `..` to be legal is
            // one whose meaning depends on where the rootfs is mounted, and
            // that is not a link this extractor can honour.
            if stack.pop().is_none() {
                return Err(Refusal::Escapes(format!(
                    "the target {target:?} climbs above the rootfs root"
                )));
            }
            continue;
        }
        stack.push(c.to_string());
    }
    Ok(stack)
}

/// A directory descriptor, closed on drop.
///
/// ⛔ Extraction holds one of these on the destination for the whole run. Every
/// write is relative to it, so no path is ever re-resolved from `/` through
/// components a previous entry may have replaced in the meantime.
pub struct Dir(i64);

impl Dir {
    pub fn open(path: &str) -> Result<Dir> {
        let c = cbuf(path)?;
        let fd = sys::open(&c, sys::O_RDONLY | sys::O_DIRECTORY | sys::O_CLOEXEC, 0)
            .map_err(|e| Error::Extract(format!("cannot open {path}: {}", e.name())))?;
        Ok(Dir(fd))
    }

    pub fn fd(&self) -> i64 {
        self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = sys::close(self.0);
    }
}

/// Which mechanism resolves a component.
///
/// ⛔ **This enum exists because the fallback is otherwise dead code on every
/// machine that can run the tests.** Measured on 2026-09-08: this host is
/// kernel 6.18.44 and `openat2(2)` is present, so an `Auto` run never enters
/// the `O_NOFOLLOW` walk, and the walk is what protects every kernel before
/// 5.6. A safety mechanism nobody has seen work is not a safety mechanism,
/// which is `scripts/plant.sh`'s whole argument applied to this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolve {
    /// `openat2` where the kernel has it, the walk otherwise. Production.
    Auto,
    /// ⛔ Force the `O_NOFOLLOW` walk. Used by the tests to drive the pre-5.6
    /// path on a kernel that does not need it.
    Walk,
}

impl Resolve {
    fn use_openat2(self) -> bool {
        self == Resolve::Auto && have_openat2()
    }
}

/// Does this kernel have `openat2(2)`?
///
/// ⭐ Measured once, by calling it, and never inferred from a release string. A
/// kernel version is a claim; the syscall's own answer is the fact. The result
/// is cached because the answer cannot change while this process runs.
pub fn have_openat2() -> bool {
    use std::sync::OnceLock;
    static HAVE: OnceLock<bool> = OnceLock::new();
    *HAVE.get_or_init(|| {
        let Some(c) = CBuf::new(".") else {
            return false;
        };
        let how = sys::OpenHow {
            flags: sys::O_RDONLY | sys::O_DIRECTORY | sys::O_CLOEXEC,
            mode: 0,
            resolve: sys::RESOLVE_BENEATH | sys::RESOLVE_NO_SYMLINKS,
        };
        match sys::openat2(sys::AT_FDCWD as i64, &c, &how) {
            Ok(fd) => {
                let _ = sys::close(fd);
                true
            }
            // ⛔ Only these two mean "this kernel cannot answer". Anything else
            // is an answer about `.`, and the syscall exists.
            Err(e) if e == sys::ENOSYS || e.0 == 7 => false,
            Err(_) => true,
        }
    })
}

fn cbuf(s: &str) -> Result<CBuf> {
    CBuf::new(s)
        .ok_or_else(|| Error::Extract(format!("{s:?} contains a NUL and cannot reach the kernel")))
}

/// Open the **parent** of `parts` beneath `root`, creating directories as
/// needed, and return the descriptor plus the final component.
///
/// ⛔ This is the function T-0304 turns on. Every component is resolved with
/// symlinks refused, so `evil/passwd` after `evil -> /etc` cannot land: the
/// resolution of `evil` fails rather than succeeding somewhere else.
pub fn open_parent(
    root: &Dir,
    parts: &[&str],
    make_dirs: bool,
) -> Result<std::result::Result<(Dir, String), Refusal>> {
    open_parent_with(root, parts, make_dirs, Resolve::Auto)
}

/// As [`open_parent`], with the resolution mechanism named. ⛔ Production calls
/// [`open_parent`]; this exists so the tests can drive BOTH mechanisms on one
/// kernel.
pub fn open_parent_with(
    root: &Dir,
    parts: &[&str],
    make_dirs: bool,
    how: Resolve,
) -> Result<std::result::Result<(Dir, String), Refusal>> {
    let (last, dirs) = match parts.split_last() {
        Some(x) => x,
        None => return Ok(Err(Refusal::Empty)),
    };
    let mut cur = dup_dir(root, how)?;
    for d in dirs {
        match step(&cur, d, make_dirs, how)? {
            Ok(next) => cur = next,
            Err(r) => return Ok(Err(r)),
        }
    }
    Ok(Ok((cur, (*last).to_string())))
}

fn dup_dir(d: &Dir, how: Resolve) -> Result<Dir> {
    let c = cbuf(".")?;
    let fd = if how.use_openat2() {
        let how = sys::OpenHow {
            flags: sys::O_RDONLY | sys::O_DIRECTORY | sys::O_CLOEXEC,
            mode: 0,
            resolve: sys::RESOLVE_BENEATH | sys::RESOLVE_NO_SYMLINKS,
        };
        sys::openat2(d.fd(), &c, &how)
    } else {
        sys::openat(
            d.fd(),
            &c,
            sys::O_RDONLY | sys::O_DIRECTORY | sys::O_CLOEXEC,
            0,
        )
    }
    .map_err(|e| Error::Extract(format!("cannot re-open the destination: {}", e.name())))?;
    Ok(Dir(fd))
}

/// One component of the walk: descend into `name`, optionally creating it.
fn step(
    cur: &Dir,
    name: &str,
    make: bool,
    how: Resolve,
) -> Result<std::result::Result<Dir, Refusal>> {
    let c = cbuf(name)?;
    // ⚠ The directory is created BEFORE the descent is attempted, because a
    // layer names `usr/bin/x` with no entry for `usr/bin` more often than not.
    // `EEXIST` is the ordinary case and is not an error here; what matters is
    // that the descent below refuses a symlink whatever created it.
    if make {
        match sys::mkdirat(cur.fd(), &c, 0o755) {
            Ok(_) => {}
            Err(e) if e == sys::EEXIST => {}
            Err(e) => {
                return Err(Error::Extract(format!(
                    "cannot create the directory {name:?}: {}",
                    e.name()
                )))
            }
        }
    }
    let flags = sys::O_RDONLY | sys::O_DIRECTORY | sys::O_CLOEXEC;
    let r = if how.use_openat2() {
        let how = sys::OpenHow {
            flags,
            mode: 0,
            resolve: sys::RESOLVE_BENEATH | sys::RESOLVE_NO_SYMLINKS,
        };
        sys::openat2(cur.fd(), &c, &how)
    } else {
        // ⛔ The fallback, and `O_NOFOLLOW` is what carries it. Opening one
        // component at a time from a held descriptor means `..` cannot appear
        // (the lexical check removed it) and a symlink cannot be traversed
        // (this flag refuses it with ELOOP).
        sys::openat(cur.fd(), &c, flags | sys::O_NOFOLLOW, 0)
    };
    match r {
        Ok(fd) => Ok(Ok(Dir(fd))),
        // ⭐ THE FINDING, and the three errnos that carry it. The kernel is
        // saying the resolution left the destination or crossed a symlink.
        // ELOOP is O_NOFOLLOW's refusal, EXDEV is RESOLVE_BENEATH's, and
        // ENOTDIR is a component that is a file.
        Err(e) if e == Errno(40) || e == Errno(18) || e == Errno(20) => {
            Ok(Err(Refusal::Escapes(format!("{name:?}: {}", e.name()))))
        }
        Err(e) => Err(Error::Extract(format!(
            "cannot descend into {name:?}: {}",
            e.name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_and_dotdot_are_refused_lexically() {
        assert_eq!(components("/etc/passwd"), Err(Refusal::Absolute));
        assert_eq!(
            components("a/../../etc/passwd"),
            Err(Refusal::Component("..".into()))
        );
        assert_eq!(components("").unwrap_err(), Refusal::Empty);
        assert_eq!(components("./").unwrap_err(), Refusal::Empty);
    }

    /// ⚠ A real OCI layer writes `bin/` and `bin`, with no `./` prefix, and a
    /// trailing slash on a directory. Neither is a defect.
    /// `experiments/results/whiteout-contract.txt` check A is where that was
    /// measured.
    #[test]
    fn ordinary_layer_paths_survive() {
        assert_eq!(components("bin/").unwrap(), vec!["bin"]);
        assert_eq!(components("./etc/shadow").unwrap(), vec!["etc", "shadow"]);
        assert_eq!(
            components("usr/lib/x.so").unwrap(),
            vec!["usr", "lib", "x.so"]
        );
    }

    /// ⭐ T-0305, and the exact link
    /// `experiments/results/whiteout-contract.txt` check C read out of a real
    /// voidlinux layer.
    #[test]
    fn absolute_symlink_target_is_rebased_not_refused() {
        let got = rebase_symlink_target(&["var", "cache"], "/var/cache/xbps").unwrap();
        assert_eq!(got, vec!["var", "cache", "xbps"]);
    }

    #[test]
    fn relative_symlink_target_resolves_from_its_own_directory() {
        // /bin -> usr/bin
        assert_eq!(
            rebase_symlink_target(&[], "usr/bin").unwrap(),
            vec!["usr", "bin"]
        );
        // /etc/mtab -> /proc/self/mounts
        assert_eq!(
            rebase_symlink_target(&["etc"], "/proc/self/mounts").unwrap(),
            vec!["proc", "self", "mounts"]
        );
        // a legitimate climb: /usr/lib/x -> ../share/y
        assert_eq!(
            rebase_symlink_target(&["usr", "lib"], "../share/y").unwrap(),
            vec!["usr", "share", "y"]
        );
    }

    #[test]
    fn a_target_that_still_escapes_after_rebasing_is_refused() {
        let r = rebase_symlink_target(&["etc"], "../../../../etc/passwd");
        assert!(matches!(r, Err(Refusal::Escapes(_))), "got {r:?}");
    }

    /// ⭐ **BOTH MECHANISMS, ON ONE KERNEL, AGAINST THE SAME ATTACK.**
    ///
    /// This host has `openat2`, measured on 2026-09-08 against kernel
    /// 6.18.44, so an `Auto` run never touches the `O_NOFOLLOW` walk and the
    /// walk would ship having never refused anything. Every kernel before
    /// Linux 5.6 runs only the walk.
    ///
    /// ⛔ The two are asserted to give the SAME answer. A fallback that is
    /// merely present is not a fallback.
    #[test]
    fn the_walk_and_openat2_refuse_the_same_traversal() {
        let base = std::env::temp_dir().join(format!("podbox-safety-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("real")).unwrap();
        // `evil -> /etc`, the first entry of the crafted layer.
        std::os::unix::fs::symlink("/etc", base.join("evil")).unwrap();
        let root = Dir::open(&base.to_string_lossy()).unwrap();

        for how in [Resolve::Auto, Resolve::Walk] {
            let r = open_parent_with(&root, &["evil", "passwd"], false, how).unwrap();
            match r {
                Err(Refusal::Escapes(_)) => {}
                Err(other) => panic!("{how:?} refused for the wrong reason: {other:?}"),
                Ok(_) => panic!("{how:?} DID NOT REFUSE the traversal"),
            }
            // And a legitimate path still resolves under the same mechanism, so
            // the refusal above is discrimination rather than a blanket no.
            let ok = open_parent_with(&root, &["real", "f"], false, how).unwrap();
            assert!(ok.is_ok(), "{how:?} refused a legitimate path");
        }
        drop(root);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// ⚠ The claim in this module's header, asserted rather than assumed: if
    /// this stops being true the test above is silently only running one arm.
    #[test]
    fn this_host_has_openat2_so_the_walk_needs_forcing_to_be_exercised() {
        assert!(
            have_openat2(),
            "this kernel has no openat2, so Resolve::Auto already IS the walk \
             and the two arms above are the same arm"
        );
    }
}
