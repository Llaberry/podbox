//! Every write the completion layer makes into somebody else's rootfs.
//!
//! ⭐ **[`TODO/complete.md`](../../../TODO/complete.md) T-0405 is this module,
//! not a special case for `/etc/mtab`.** `printf ... > "$R/etc/mtab"` follows
//! the link, and `/etc/mtab` is a symlink in most images. If its target is
//! absolute the write lands **outside the rootfs**, on the host. The same shape
//! is `/etc/resolv.conf -> ../run/systemd/resolve/stub-resolv.conf`, and the
//! next one nobody has met yet.
//!
//! So there is one door: [`Root`], which holds a descriptor on the rootfs and
//! resolves every DIRECTORY component through
//! [`podbox_extract::safety::open_parent_with`] under
//! [`podbox_extract::safety::Resolve::InRoot`] -- `openat2(RESOLVE_IN_ROOT)`,
//! which follows a symlink but treats the rootfs as `/`, so nothing resolves
//! outside it. The FINAL component is opened `O_NOFOLLOW`, and a replacement is
//! **unlink-then-create** rather than a truncate, because truncating through a
//! link is the same escape with a different syscall.
//!
//! ⭐ **`InRoot` and not extraction's `NO_SYMLINKS`, and the difference is the
//! whole of this module's correctness.** During extraction a symlink another
//! entry created is the attack. Afterwards, a finished rootfs is full of
//! legitimate internal links -- `/etc/ssl/certs -> /var/lib/ca-certificates/pem`
//! on openSUSE, `/lib -> usr/lib` on void -- and a writer that refuses those
//! cannot reach the file the payload will read. ⚠ Measured on 2026-09-09 by
//! `experiments/240-distro-sweep.sh`: under `NO_SYMLINKS` the CA-bundle fixup
//! reported `leaves the destination` on openSUSE and the libc probe read
//! `unknown` on void, and neither link leaves the rootfs.
//!
//! ⛔ This module is the only place in `podbox-complete` that opens anything by
//! path. A second one would be the one that forgets.

use podbox_extract::safety::{self, Dir, Refusal, Resolve};
use podbox_probe::sys::{self, CBuf};

use crate::{Error, Result};

/// What is at a path, without following the last component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Missing,
    Regular {
        len: u64,
        mode: u32,
    },
    Dir,
    /// ⚠ The target is kept because the diagnostic needs it: "replaced a
    /// symlink" without saying where it pointed is a line nobody can audit.
    Symlink(String),
    /// ⭐ A character device, with the numbers it carries. T-0401 needs them:
    /// a node that is there but is `1:5` where `1:3` was wanted is a wrong
    /// answer rather than a present one.
    CharDev {
        major: u64,
        minor: u64,
    },
    /// A block device, socket or fifo.
    Other(u32),
}

impl Kind {
    pub fn is_missing(&self) -> bool {
        *self == Kind::Missing
    }
}

/// What a write did. ⛔ `Unchanged` is a distinct outcome and never folded into
/// `Wrote`: a fixup that reports a change it did not make is the `sandlock`
/// failure [`TODO/cli.md`](../../../TODO/cli.md) T-0804 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrote {
    Created,
    Replaced,
    Unchanged,
}

/// A descriptor on the rootfs, plus the two operations the completion layer
/// needs.
pub struct Root {
    dir: Dir,
    path: String,
}

impl Root {
    pub fn open(rootfs: &str) -> Result<Root> {
        let dir = Dir::open(rootfs)
            .map_err(|e| Error::Complete(format!("cannot open the rootfs {rootfs}: {e}")))?;
        Ok(Root {
            dir,
            path: rootfs.to_string(),
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Split a rootfs-relative path into components, refusing anything that is
    /// not one. ⚠ Lexical, and it is only half the check: the other half is the
    /// kernel's, in [`safety::open_parent`].
    fn parts(rel: &str) -> Result<Vec<&str>> {
        safety::components(rel).map_err(|r: Refusal| Error::Complete(r.why()))
    }

    /// Open the parent of `rel`, creating directories where asked.
    ///
    /// ⛔ **Three outcomes, not two.** `Ok(Ok(..))` reached it; `Ok(Err(why))`
    /// did not, and `why` says which of the two reasons it was; `Err` is a
    /// lexical refusal of the path itself, which is a defect in podbox rather
    /// than a property of the image.
    ///
    /// ⚠ An ancestor that does not exist is NOT an error. `etc/apt/apt.conf.d`
    /// is absent in every rootfs that is not Debian-family, and a `kind()` on a
    /// file inside it must answer "missing" rather than fail the whole
    /// completion run. An ancestor that is a symlink OUT of the rootfs is the
    /// other reason, and the two are told apart because only one of them is a
    /// refusal the caller must report.
    fn parent(
        &self,
        rel: &str,
        make_dirs: bool,
    ) -> Result<std::result::Result<(Dir, String), String>> {
        let parts = Self::parts(rel)?;
        match safety::open_parent_with(&self.dir, &parts, make_dirs, Resolve::InRoot) {
            Ok(Ok(x)) => Ok(Ok(x)),
            // A refusal: a component resolved outside the rootfs, or the kernel
            // declined the resolution.
            Ok(Err(r)) => Ok(Err(r.why())),
            Err(e) => Ok(Err(format!("{e}"))),
        }
    }

    /// What is at `rel`, with the last component not followed.
    pub fn kind(&self, rel: &str) -> Result<Kind> {
        let Ok((parent, name)) = self.parent(rel, false)? else {
            return Ok(Kind::Missing);
        };
        let Some(c) = CBuf::new(&name) else {
            return Ok(Kind::Missing);
        };
        let st = match sys::fstatat(parent.fd(), &c, sys::AT_SYMLINK_NOFOLLOW) {
            Ok(st) => st,
            Err(_) => return Ok(Kind::Missing),
        };
        let fmt = st.st_mode & sys::S_IFMT;
        Ok(match fmt {
            0o100000 => Kind::Regular {
                len: st.st_size as u64,
                mode: st.st_mode & 0o7777,
            },
            0o040000 => Kind::Dir,
            0o020000 => Kind::CharDev {
                // ⚠ The kernel's wide encoding, which is what `st_rdev`
                // carries and what `makedev` builds.
                major: ((st.st_rdev >> 8) & 0xfff) | ((st.st_rdev >> 32) & !0xfffu64),
                minor: (st.st_rdev & 0xff) | ((st.st_rdev >> 12) & !0xffu64),
            },
            0o120000 => {
                let t = sys::readlinkat(parent.fd(), &c)
                    .map(|b| String::from_utf8_lossy(&b).to_string())
                    .unwrap_or_default();
                Kind::Symlink(t)
            }
            other => Kind::Other(other),
        })
    }

    /// Read a regular file at `rel`, never following the last component.
    ///
    /// ⛔ Bounded. `std::fs::read` on a character device reads to an EOF that
    /// never comes: `/dev/urandom` allocated 13 GB here before the OOM killer
    /// took it ([`TODO/supervise.md`](../../../TODO/supervise.md) T-0602). The
    /// completion layer reads configuration files, and a configuration file
    /// that is 8 MiB is not one podbox is going to edit correctly anyway.
    pub fn read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        const CEILING: usize = 8 * 1024 * 1024;
        let Ok((parent, name)) = self.parent(rel, false)? else {
            return Ok(None);
        };
        let Some(c) = CBuf::new(&name) else {
            return Ok(None);
        };
        let fd = match sys::openat(
            parent.fd(),
            &c,
            sys::O_RDONLY | sys::O_NOFOLLOW | sys::O_CLOEXEC,
            0,
        ) {
            Ok(fd) => fd,
            Err(_) => return Ok(None),
        };
        let mut out = Vec::new();
        let mut buf = [0u8; 65536];
        loop {
            match sys::read(fd, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    out.extend_from_slice(&buf[..n as usize]);
                    if out.len() > CEILING {
                        let _ = sys::close(fd);
                        return Err(Error::Complete(format!(
                            "{rel} is larger than {CEILING} bytes; podbox will not \
                             read a configuration file that size"
                        )));
                    }
                }
                Err(e) if e == sys::EINTR => continue,
                Err(e) => {
                    let _ = sys::close(fd);
                    return Err(Error::Complete(format!("cannot read {rel}: {}", e.name())));
                }
            }
        }
        let _ = sys::close(fd);
        Ok(Some(out))
    }

    /// Read the same way, as text, dropping a file that is not UTF-8.
    ///
    /// ⚠ `None` for both "absent" and "not text" is deliberate here: every
    /// caller of this treats a configuration file it cannot parse exactly like
    /// one that is not there, and the two would otherwise be one `match` arm
    /// written twice.
    pub fn read_text(&self, rel: &str) -> Result<Option<String>> {
        Ok(self
            .read(rel)?
            .and_then(|b| String::from_utf8(b).ok().map(|s| s.to_string())))
    }

    /// Write `bytes` to `rel`, replacing whatever is there.
    ///
    /// ⛔ **Unlink first, then create with `O_EXCL | O_NOFOLLOW`.** A truncate
    /// of an existing name follows a symlink exactly as an ordinary open does,
    /// so "the file is already there" is the case that escapes.
    pub fn write(&self, rel: &str, bytes: &[u8], mode: u32) -> Result<Wrote> {
        let existing = self.kind(rel)?;
        // ⛔ The MODE is part of "unchanged". T-0404 widens the owner write bit
        // of an `/etc/passwd` whose content it does not touch, and a comparison
        // on the bytes alone reported that as a no-op and left the file
        // read-only, which is the failure the fixup exists for.
        if let Kind::Regular { mode: have, .. } = existing {
            if self.read(rel)?.as_deref() == Some(bytes) {
                if have == mode {
                    return Ok(Wrote::Unchanged);
                }
                let Ok((parent, name)) = self.parent(rel, false)? else {
                    return Ok(Wrote::Unchanged);
                };
                let c = CBuf::new(&name).ok_or_else(|| {
                    Error::Complete(format!("{rel} contains a NUL and cannot be written"))
                })?;
                sys::fchmodat(parent.fd(), &c, mode as u64, 0).map_err(|e| {
                    Error::Complete(format!("cannot set the mode of {rel}: {}", e.name()))
                })?;
                return Ok(Wrote::Replaced);
            }
        }
        let (parent, name) = match self.parent(rel, true)? {
            Ok(x) => x,
            Err(why) => {
                return Err(Error::Complete(format!(
                    "{rel}: podbox will not write through this path: {why}"
                )))
            }
        };
        let c = CBuf::new(&name).ok_or_else(|| {
            Error::Complete(format!("{rel} contains a NUL and cannot be written"))
        })?;
        // ⛔ Unconditional, and it covers the symlink, the dangling symlink and
        // the ordinary file in one call. ENOENT here is the ordinary case.
        match sys::unlinkat(parent.fd(), &c, 0) {
            Ok(_) => {}
            Err(e) if e == sys::ENOENT => {}
            Err(e) if e.0 == 21 => {
                // EISDIR: a directory where the completion layer wants a file.
                return Err(Error::Complete(format!(
                    "{rel} is a directory in this image; podbox will not replace a \
                     directory with a file"
                )));
            }
            Err(e) => {
                return Err(Error::Complete(format!(
                    "cannot remove the existing {rel}: {}",
                    e.name()
                )))
            }
        }
        let fd = sys::openat(
            parent.fd(),
            &c,
            sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_NOFOLLOW | sys::O_CLOEXEC,
            mode as u64,
        )
        .map_err(|e| Error::Complete(format!("cannot create {rel}: {}", e.name())))?;
        let mut off = 0;
        while off < bytes.len() {
            match sys::write(fd, &bytes[off..]) {
                Ok(0) => break,
                Ok(n) => off += n as usize,
                Err(e) if e == sys::EINTR => continue,
                Err(e) => {
                    let _ = sys::close(fd);
                    return Err(Error::Complete(format!("cannot write {rel}: {}", e.name())));
                }
            }
        }
        // ⚠ The mode is set explicitly rather than left to the open: umask
        // subtracts from the mode an open asks for, and T-0404 needs the
        // payload's own `useradd` to be able to write `/etc/passwd` back.
        let _ = sys::fchmod(fd, mode as u64);
        let _ = sys::close(fd);
        Ok(match existing {
            Kind::Missing => Wrote::Created,
            _ => Wrote::Replaced,
        })
    }

    /// Try to create a character device at `rel`.
    ///
    /// ⭐ **T-0401 calls this FIRST and shims only where it fails.** `mknod` is
    /// denied on the runtimes podbox is built for, and that denial is the whole
    /// premise of the shim; on a machine that permits it, a real `/dev/null` is
    /// what the payload should get, and reporting a shim there would be podbox
    /// inventing a degradation. ⛔ Probed by calling it, never inferred from a
    /// capability bit: this runtime hands a process every capability and denies
    /// the operation anyway.
    ///
    /// `Ok(true)` a device is now there, `Ok(false)` the kernel refused and the
    /// errno is in the string, `Err` podbox could not even try.
    pub fn mknod_char(
        &self,
        rel: &str,
        mode: u32,
        major: u64,
        minor: u64,
    ) -> Result<std::result::Result<(), String>> {
        let (parent, name) = match self.parent(rel, true)? {
            Ok(x) => x,
            Err(why) => return Ok(Err(why)),
        };
        let c = CBuf::new(&name).ok_or_else(|| Error::Complete(format!("{rel} contains a NUL")))?;
        // ⛔ Unlink first, for the same reason `write` does: a name that is
        // already there is the case that escapes, and `mknodat` on an existing
        // name is EEXIST rather than a replacement.
        match sys::unlinkat(parent.fd(), &c, 0) {
            Ok(_) => {}
            Err(e) if e == sys::ENOENT => {}
            Err(e) => {
                return Ok(Err(format!(
                    "cannot remove the existing {rel}: {}",
                    e.name()
                )))
            }
        }
        match sys::mknodat(
            parent.fd(),
            &c,
            sys::S_IFCHR | mode as u64,
            sys::makedev(major, minor),
        ) {
            Ok(_) => {
                let _ = sys::fchmodat(parent.fd(), &c, mode as u64, 0);
                Ok(Ok(()))
            }
            Err(e) => Ok(Err(e.name())),
        }
    }

    /// Open `rel` **following the last component**, confined to the rootfs.
    ///
    /// ⭐ **The opposite of [`Root::write`], and the difference is who owns the
    /// file.** `write` replaces: `/etc/mtab`, `/etc/resolv.conf`,
    /// `/etc/hosts` and `/etc/passwd` are files podbox owns, and following a
    /// link there is T-0405's escape. This one FOLLOWS: the CA trust store is
    /// the image's file and podbox is adding to it, so writing to the link
    /// rather than through it puts the bytes where nothing reads them.
    ///
    /// ⛔ Measured on 2026-09-09 against `registry.opensuse.org/opensuse/leap`:
    /// `/etc/ssl/ca-bundle.pem` is a symlink to
    /// `/var/lib/ca-certificates/ca-bundle.pem`, and replacing the link left
    /// curl's default trust store untouched -- `curl --cacert
    /// /etc/ssl/ca-bundle.pem` returned 200 on the same host where the default
    /// returned `self-signed certificate in certificate chain`.
    ///
    /// ⚠ ONE `openat2` from the rootfs descriptor with `RESOLVE_IN_ROOT`, so
    /// the kernel does the confinement: an absolute target rebases onto the
    /// rootfs and nothing resolves outside it. On a kernel without `openat2`
    /// this returns `None` and the caller reports a fixup it could not apply.
    fn open_following(&self, rel: &str, create: bool, mode: u32) -> Result<Option<i64>> {
        if !podbox_extract::safety::have_openat2() {
            return Ok(None);
        }
        // ⚠ Lexically checked first, exactly as every other path here is.
        Self::parts(rel)?;
        let Some(c) = CBuf::new(rel) else {
            return Ok(None);
        };
        let flags = if create {
            sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC
        } else {
            sys::O_RDONLY | sys::O_CLOEXEC
        };
        let how = sys::OpenHow {
            flags,
            mode: mode as u64,
            resolve: sys::RESOLVE_IN_ROOT,
        };
        match sys::openat2(self.dir.fd(), &c, &how) {
            Ok(fd) => Ok(Some(fd)),
            Err(_) => Ok(None),
        }
    }

    /// Read `rel`, following the last component. See [`Root::open_following`].
    pub fn read_following(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        const CEILING: usize = 8 * 1024 * 1024;
        let Some(fd) = self.open_following(rel, false, 0)? else {
            return Ok(None);
        };
        let mut out = Vec::new();
        let mut buf = [0u8; 65536];
        loop {
            match sys::read(fd, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    out.extend_from_slice(&buf[..n as usize]);
                    if out.len() > CEILING {
                        let _ = sys::close(fd);
                        return Err(Error::Complete(format!(
                            "{rel} is larger than {CEILING} bytes"
                        )));
                    }
                }
                Err(e) if e == sys::EINTR => continue,
                Err(_) => break,
            }
        }
        let _ = sys::close(fd);
        Ok(Some(out))
    }

    /// Write `rel`, following the last component. See [`Root::open_following`].
    ///
    /// ⚠ `None` where the kernel cannot do it safely, so the caller reports a
    /// fixup it could not apply rather than one it silently skipped.
    pub fn write_following(&self, rel: &str, bytes: &[u8], mode: u32) -> Result<Option<Wrote>> {
        let existed = self.read_following(rel)?;
        if existed.as_deref() == Some(bytes) {
            return Ok(Some(Wrote::Unchanged));
        }
        let Some(fd) = self.open_following(rel, true, mode)? else {
            return Ok(None);
        };
        let mut off = 0;
        while off < bytes.len() {
            match sys::write(fd, &bytes[off..]) {
                Ok(0) => break,
                Ok(n) => off += n as usize,
                Err(e) if e == sys::EINTR => continue,
                Err(e) => {
                    let _ = sys::close(fd);
                    return Err(Error::Complete(format!("cannot write {rel}: {}", e.name())));
                }
            }
        }
        let _ = sys::fchmod(fd, mode as u64);
        let _ = sys::close(fd);
        Ok(Some(match existed {
            None => Wrote::Created,
            Some(_) => Wrote::Replaced,
        }))
    }

    /// Make `rel` a directory, and every parent of it.
    pub fn mkdirs(&self, rel: &str) -> Result<()> {
        let parts = Self::parts(rel)?;
        let mut all: Vec<&str> = parts.clone();
        // ⚠ `open_parent` creates the parents and hands back the last
        // component, so the directory itself is one more `mkdirat`.
        all.push(".");
        match safety::open_parent_with(&self.dir, &all, true, Resolve::InRoot)
            .map_err(|e| Error::Complete(format!("{rel}: {e}")))?
        {
            Ok(_) => Ok(()),
            Err(r) => Err(Error::Complete(format!("{rel}: {}", r.why()))),
        }
    }

    /// Every name directly inside `rel`, sorted, or an empty list where `rel` is
    /// not a directory podbox can reach.
    pub fn list(&self, rel: &str) -> Result<Vec<String>> {
        let parts = Self::parts(rel)?;
        let mut all: Vec<&str> = parts.clone();
        all.push(".");
        let dir = match safety::open_parent_with(&self.dir, &all, false, Resolve::InRoot) {
            Ok(Ok((d, _))) => d,
            // ⚠ A directory podbox cannot reach lists as empty rather than
            // failing: most of these directories are absent in most images.
            Ok(Err(_)) | Err(_) => return Ok(Vec::new()),
        };
        let mut names: Vec<String> = sys::getdents64(dir.fd())
            .map_err(|e| Error::Complete(format!("cannot list {rel}: {}", e.name())))?
            .into_iter()
            .map(|d| String::from_utf8_lossy(&d.name).to_string())
            .filter(|n| n != "." && n != "..")
            .collect();
        names.sort();
        Ok(names)
    }

    /// Remove `rel` if it is there, never following the last component.
    pub fn unlink(&self, rel: &str) -> Result<bool> {
        let Ok((parent, name)) = self.parent(rel, false)? else {
            return Ok(false);
        };
        let Some(c) = CBuf::new(&name) else {
            return Ok(false);
        };
        match sys::unlinkat(parent.fd(), &c, 0) {
            Ok(_) => Ok(true),
            Err(e) if e == sys::ENOENT => Ok(false),
            Err(e) => Err(Error::Complete(format!(
                "cannot remove {rel}: {}",
                e.name()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        let p = std::env::temp_dir().join(format!("podbox-complete-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("etc")).unwrap();
        p.to_string_lossy().to_string()
    }

    /// ⛔ T-0405's whole subject. The link points at an absolute path outside
    /// the rootfs; the write must land inside it and the canary must survive.
    #[test]
    fn a_write_through_an_absolute_symlink_does_not_escape() {
        let root = scratch("escape");
        let outside = std::env::temp_dir().join(format!("podbox-canary-{}", std::process::id()));
        std::fs::write(&outside, b"canary\n").unwrap();
        std::os::unix::fs::symlink(&outside, format!("{root}/etc/mtab")).unwrap();

        let r = Root::open(&root).unwrap();
        assert!(matches!(r.kind("etc/mtab").unwrap(), Kind::Symlink(_)));
        assert_eq!(
            r.write("etc/mtab", b"inside\n", 0o644).unwrap(),
            Wrote::Replaced
        );

        assert_eq!(std::fs::read(&outside).unwrap(), b"canary\n");
        assert_eq!(
            std::fs::read(format!("{root}/etc/mtab")).unwrap(),
            b"inside\n"
        );
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// ⛔ The other half: a DIRECTORY component that is a symlink out. The
    /// kernel refuses the resolution, so nothing is written at all.
    #[test]
    fn a_write_through_a_symlinked_directory_is_refused() {
        let root = scratch("direscape");
        let outside = std::env::temp_dir().join(format!("podbox-outdir-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, format!("{root}/evil")).unwrap();

        let r = Root::open(&root).unwrap();
        let e = r.write("evil/passwd", b"x\n", 0o644).unwrap_err();
        assert!(format!("{e}").contains("will not write through"), "{e}");
        assert!(!outside.join("passwd").exists());
        let _ = std::fs::remove_dir_all(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_identical_write_reports_unchanged() {
        let root = scratch("idem");
        let r = Root::open(&root).unwrap();
        assert_eq!(r.write("etc/hosts", b"a\n", 0o644).unwrap(), Wrote::Created);
        assert_eq!(
            r.write("etc/hosts", b"a\n", 0o644).unwrap(),
            Wrote::Unchanged
        );
        assert_eq!(
            r.write("etc/hosts", b"b\n", 0o644).unwrap(),
            Wrote::Replaced
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
