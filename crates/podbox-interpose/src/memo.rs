//! The ownership memo: what the payload MEANT a file's owner to be.
//!
//! ⭐ [`TODO/interpose.md`](../../../TODO/interpose.md) T-0704. `chown 0:42`
//! answers `EINVAL` on this runtime, so podbox records the intent and reports it
//! back from the `stat` family. The model is fakeroot's
//! `references/salsa-debian__fakeroot/tree/libfakeroot.c:875-907`: read the real
//! metadata, overwrite `st_uid` and `st_gid` with the intended values, memo
//! them, attempt the real call, and swallow the failure.
//!
//! ⛔ **A FILE INSIDE THE ROOTFS, and T-0704's `Approach` said the sidecar.**
//! It cannot be the sidecar: `crates/podbox-extract/src/sidecar.rs` writes JSONL
//! beside the rootfs **in the store**, and this object runs after the `chroot`,
//! where the store does not exist. It is also JSON, and parsing it would
//! allocate on an interposed path, which T-0701 constraint 2 forbids. The memo
//! is therefore a fixed-width binary file at [`MEMO`], inside the rootfs, which
//! podbox owns and the payload can see -- and the payload being able to see it
//! is a disclosure rather than a leak: the honesty rules have no exception for
//! a helpful edit.
//!
//! ⛔ **IT MUST CROSS PROCESSES, and that is what rules out a table in memory.**
//! T-0704's own `Prove` is `sh -c 'chown 0:42 /tmp/f && stat -c %u:%g /tmp/f'`:
//! `chown` and `stat` are two `execve`s, so a memo that lived in this object's
//! own memory would be gone before the question was asked. fakeroot solves this
//! with a DAEMON, `faked`, which podbox has no room for inside somebody else's
//! chroot.
//!
//! ⚠ **What it is not.** It changes no kernel permission check, exactly as
//! [`TODO/extract.md`](../../../TODO/extract.md) T-0302's sidecar does not: a
//! file recorded here as `gid 42` is owned by the running id on disk, and every
//! access check the kernel makes uses the latter.

use core::ffi::{c_char, c_int, c_void};

use crate::say;

/// Where the memo lives, inside the rootfs.
///
/// ⚠ An absolute path, because this object runs after the `chroot` and the
/// payload's working directory is its own business. `/.podbox` is the directory
/// [`TODO/interpose.md`](../../../TODO/interpose.md) T-0702 places this object
/// in, so the memo sits beside the thing that writes it.
pub const MEMO: &core::ffi::CStr = c"/.podbox/ownership.memo";

/// One record. ⛔ Fixed width and little-endian, so a reader can scan without
/// parsing and without allocating.
///
/// ```text
///   0  dev   u64      the kernel's device number for the file
///   8  ino   u64      and its inode. ⭐ NOT the path: a path is renamed,
///                     hard-linked and resolved through symlinks, and the
///                     question "who owns this file" is about the inode
///  16  uid   u32
///  20  gid   u32
///  24  set   u32      bit 0 the uid is meant, bit 1 the gid
///  28  pad   u32      so the record is 32 bytes and a scan is one shift
/// ```
pub const RECORD: usize = 32;
pub const SET_UID: u32 = 1;
pub const SET_GID: u32 = 2;

/// ⚠ How much of the memo a lookup will read. Bounded, because this runs on
/// every `stat` a payload makes: 4 MiB is 131,072 records, and a container that
/// has recorded more than that has a different problem. ⛔ Reaching it is
/// announced once rather than silently truncating the answer.
const SCAN_CEILING: usize = 4 * 1024 * 1024;

extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn mkdir(path: *const c_char, mode: u32) -> c_int;
}

const O_RDONLY: c_int = 0;
const O_WRONLY: c_int = 1;
const O_CREAT: c_int = 0o100;
const O_APPEND: c_int = 0o2000;
const O_CLOEXEC: c_int = 0o2000000;

/// What the payload meant, for one file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
    pub set: u32,
}

fn u64_le(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    u64::from_le_bytes(a)
}

fn u32_le(b: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[..4]);
    u32::from_le_bytes(a)
}

/// Append what the payload meant.
///
/// ⛔ **`O_APPEND` and one `write` per record**, so two processes recording at
/// once cannot interleave a record: the kernel makes the seek and the write of
/// an appending descriptor one operation. There is no lock here at all, which is
/// T-0701 constraint 2 satisfied by not needing it rather than by holding one
/// carefully.
///
/// ⚠ A record is APPENDED rather than replacing an earlier one for the same
/// inode. A rewrite would need a lock and a scan on the write path; the reader
/// takes the LAST matching record, so the newest intent wins.
pub fn record(dev: u64, ino: u64, o: Owner) -> bool {
    let mut rec = [0u8; RECORD];
    rec[0..8].copy_from_slice(&dev.to_le_bytes());
    rec[8..16].copy_from_slice(&ino.to_le_bytes());
    rec[16..20].copy_from_slice(&o.uid.to_le_bytes());
    rec[20..24].copy_from_slice(&o.gid.to_le_bytes());
    rec[24..28].copy_from_slice(&o.set.to_le_bytes());
    unsafe {
        // ⚠ The directory may not be there on a rootfs podbox has not placed
        // this object into by hand. `EEXIST` is the normal answer and is
        // ignored; anything else shows up as the open failing, below.
        let _ = mkdir(c"/.podbox".as_ptr(), 0o755);
        let fd = open(
            MEMO.as_ptr(),
            O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC,
            0o644 as c_int,
        );
        if fd < 0 {
            return false;
        }
        let n = write(fd, rec.as_ptr() as *const c_void, RECORD);
        close(fd);
        n == RECORD as isize
    }
}

/// What the payload last meant for this file, or `None`.
///
/// ⚠ Scanned backwards through fixed-size chunks so the newest record wins
/// without holding the whole file: nothing here allocates, and the buffer is one
/// page of stack.
pub fn lookup(dev: u64, ino: u64) -> Option<Owner> {
    let fd = unsafe { open(MEMO.as_ptr(), O_RDONLY | O_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // ⛔ 4 KiB, a multiple of the record size, so a record never straddles two
    // reads and the scan needs no carry-over buffer.
    const CHUNK: usize = 128 * RECORD;
    let mut buf = [0u8; CHUNK];
    let mut found: Option<Owner> = None;
    let mut scanned = 0usize;
    loop {
        let mut have = 0usize;
        // ⚠ `read` may return short. Filled to a record boundary before the
        // scan, or a partial record at the end of a chunk would be read as a
        // whole one.
        while have < CHUNK {
            let n = unsafe { read(fd, buf[have..].as_mut_ptr() as *mut c_void, CHUNK - have) };
            if n <= 0 {
                break;
            }
            have += n as usize;
        }
        if have == 0 {
            break;
        }
        let mut at = 0usize;
        while at + RECORD <= have {
            let r = &buf[at..at + RECORD];
            if u64_le(&r[0..8]) == dev && u64_le(&r[8..16]) == ino {
                // ⚠ The LAST match in the file wins, so this overwrites rather
                // than breaking out.
                found = Some(Owner {
                    uid: u32_le(&r[16..20]),
                    gid: u32_le(&r[20..24]),
                    set: u32_le(&r[24..28]),
                });
            }
            at += RECORD;
        }
        scanned += have;
        if have < CHUNK {
            break;
        }
        if scanned >= SCAN_CEILING {
            // ⛔ Announced, always, and not only under the debug switch: an
            // answer podbox stopped looking for is not an answer.
            say::line(&[
                b"the ownership memo is over ",
                say::Num::new(SCAN_CEILING as u64).as_bytes(),
                b" bytes and podbox stopped scanning it. An owner recorded past \
                  that point is not reported (TODO/interpose.md T-0704)",
            ]);
            break;
        }
    }
    unsafe {
        close(fd);
    }
    found
}
