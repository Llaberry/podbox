//! `TOOL.md` section 6.7: the `LD_PRELOAD` cdylib.
//!
//! ⭐ **Ownership virtualization, which is the half a path interposer does not
//! have.** [`TODO/interpose.md`](../../../TODO/interpose.md) T-0704:
//! `references/VHSgunzo__pathmap/tree/path-mapping.c:1240-1241` routes `chown`
//! through the same path-rewriting macro as every other entry point and passes
//! the ids through untouched, so a payload's `chown 0:42` fails with `EINVAL`
//! exactly as it would with no interposer loaded. That is the wall that stops
//! the most tools, and clearing it is what makes M6 worth doing.
//!
//! ⚠ **Path virtualization is T-0703 and is not here yet.** This object
//! interposes the ownership family and the `stat` family and nothing else; it
//! rewrites no path. The two halves are separate entries because they are
//! separate mechanisms, and shipping one is not a claim about the other.
//!
//! # The four constraints, none of them optional
//!
//! This object runs inside other people's processes, so
//! [`TODO/interpose.md`](../../../TODO/interpose.md) T-0701 binds every line:
//!
//! 1. ⛔ **A version script exporting only the interposed symbols.**
//!    `interpose.map` is it, and `scripts/build-interpose.sh` passes it. A
//!    default Rust cdylib exports `rust_eh_personality` and friends into every
//!    process it is loaded into;
//! 2. ⛔ **No allocation and no lock on an interposed path.** Every buffer here
//!    is on the stack, the `dlsym` cache is one `AtomicPtr` per entry point, and
//!    the memo needs no lock because `O_APPEND` makes the kernel do the
//!    serialising. ⚠ The constraint is on THIS object's allocation: forwarding
//!    re-enters the payload's allocator and that is fine and unavoidable;
//! 3. ⛔ **No `println!`.** [`say`] writes to fd 2 directly, because stdout
//!    belongs to the payload;
//! 4. ⛔ **It must survive the payload forking**, which every workload here does
//!    constantly. Nothing is registered with `atfork`, nothing is held across a
//!    fork but an `AtomicPtr` of a function address, and the memo is a file
//!    rather than memory precisely so a `fork` and an `execve` both keep it.
//!
//! # What it does to a `chown`
//!
//! ⛔ The real call is TRIED first and only its failure is swallowed, which is
//! fakeroot's shape at
//! `references/salsa-debian__fakeroot/tree/libfakeroot.c:875-907`. ⚠ And
//! fakeroot's errno test is **wrong for this runtime**:
//! `references/salsa-debian__fakeroot/tree/libfakeroot.c:903` is
//! `if(r&&(errno==EPERM)) r=0;` and there are eight such sites. Here the errno
//! is `EINVAL`, so every one of them would hand the error back to the caller.
//! podbox swallows `EPERM` **and** `EINVAL` and records which.

#![forbid(unsafe_op_in_unsafe_fn)]
// ⛔ x86_64 ONLY, and refused by name rather than built wrong. The `stat`
// offsets below are this architecture's, measured under both libcs; on i686 and
// arm they differ, and an object that wrote a uid at the wrong offset would
// corrupt whatever field is there. `scripts/build-interpose.sh` builds the two
// x86_64 targets, so this is the set that exists rather than a limitation
// invented here.
#[cfg(not(target_arch = "x86_64"))]
compile_error!(
    "podbox-interpose is x86_64-only: src/lib.rs carries this architecture's \
     `struct stat` field offsets, measured under glibc and musl, and another \
     architecture needs its own measurement (TODO/interpose.md T-0704)"
);

pub mod memo;
pub mod real;
pub mod say;

use core::ffi::{c_char, c_int, c_uint, c_void};

// ------------------------------------------------------------- `struct stat`
//
// ⭐ MEASURED, not read out of a header. `experiments/105-interpose-ownership.sh`
// compiles one `offsetof` program with the host `cc` and once more through
// `scripts/zig-cc.sh` against musl, and both answer:
//
//     sizeof=144 dev=0 ino=8 mode=24 uid=28 gid=32 rdev=40 size=48
//
// ⚠ They agree on x86_64 and that is a property of this architecture rather
// than a general truth: `TODO/interpose.md` T-0702's whole premise is that
// struct layout is what a preload cannot bridge, and
// `references/pkgforge-dev__cross-libc-dlopen/tree/docs/limits.md:20` records
// `regoff_t` at 4 bytes on glibc and 8 on musl. This object is built once per
// libc anyway, so it never has to bridge them.
const ST_DEV: usize = 0;
const ST_INO: usize = 8;
const ST_UID: usize = 28;
const ST_GID: usize = 32;

// ------------------------------------------------------------ `struct statx`
//
// ⛔ **`statx` IS NOT A DUPLICATE OF `stat`, and leaving it out made the glibc
// arm answer `0:0` while the memo was written.** Measured on 2026-09-09 by
// `experiments/105-interpose-ownership.sh`: coreutils' `stat` on a glibc 2.41
// payload asks `statx(2)` and never reaches `stat`, `stat64` or `__xstat`, so
// every one of those was interposed and none of them was called. busybox's
// `stat` on musl does call `stat`, which is why the musl arm passed and the
// glibc one did not -- a one-libc test would have shipped this.
//
// ⚠ Its offsets are the KERNEL's and identical under both libcs, measured the
// same way: sizeof=256 mask=0 uid=20 gid=24 mode=28 ino=32 devmaj=136
// devmin=140.
const STX_MASK: usize = 0;
const STX_UID: usize = 20;
const STX_GID: usize = 24;
const STX_INO: usize = 32;
const STX_DEV_MAJOR: usize = 136;
const STX_DEV_MINOR: usize = 140;
const STATX_UID: u32 = 0x0000_0008;
const STATX_GID: u32 = 0x0000_0010;

/// The kernel's wide device encoding, which is what `st_dev` carries and what
/// `statx` splits into a major and a minor. ⛔ The same formula
/// `podbox_probe::sys::makedev` uses, because a memo written under one encoding
/// and looked up under another finds nothing and reads as "no memo".
fn makedev(major: u64, minor: u64) -> u64 {
    ((major & 0xfff) << 8)
        | (minor & 0xff)
        | ((major & !0xfffu64) << 32)
        | ((minor & !0xffu64) << 12)
}

/// Read `dev` and `ino` out of a filled `struct stat`.
///
/// # Safety
/// `st` must point at a `struct stat` the real call has just filled.
unsafe fn dev_ino(st: *const c_void) -> (u64, u64) {
    let b = st as *const u8;
    unsafe {
        (
            (b.add(ST_DEV) as *const u64).read_unaligned(),
            (b.add(ST_INO) as *const u64).read_unaligned(),
        )
    }
}

/// Report the memo back, over a filled `struct stat`.
///
/// ⛔ Only the fields the payload actually asked for are overwritten: a `chown`
/// that named a gid and left the uid alone must not make podbox invent a uid.
///
/// # Safety
/// As [`dev_ino`].
unsafe fn report(st: *mut c_void) {
    let (dev, ino) = unsafe { dev_ino(st) };
    let Some(o) = memo::lookup(dev, ino) else {
        return;
    };
    let b = st as *mut u8;
    unsafe {
        if o.set & memo::SET_UID != 0 {
            (b.add(ST_UID) as *mut u32).write_unaligned(o.uid);
        }
        if o.set & memo::SET_GID != 0 {
            (b.add(ST_GID) as *mut u32).write_unaligned(o.gid);
        }
    }
    if say::debug() {
        say::line(&[
            b"reported the recorded owner ",
            say::Num::new(o.uid as u64).as_bytes(),
            b":",
            say::Num::new(o.gid as u64).as_bytes(),
            b" for inode ",
            say::Num::new(ino).as_bytes(),
        ]);
    }
}

/// Report the memo back, over a filled `struct statx`.
///
/// # Safety
/// `st` must point at a `struct statx` the real call has just filled.
unsafe fn report_statx(st: *mut c_void) {
    let b = st as *mut u8;
    let (ino, major, minor) = unsafe {
        (
            (b.add(STX_INO) as *const u64).read_unaligned(),
            (b.add(STX_DEV_MAJOR) as *const u32).read_unaligned() as u64,
            (b.add(STX_DEV_MINOR) as *const u32).read_unaligned() as u64,
        )
    };
    let Some(o) = memo::lookup(makedev(major, minor), ino) else {
        return;
    };
    unsafe {
        let mut mask = (b.add(STX_MASK) as *const u32).read_unaligned();
        if o.set & memo::SET_UID != 0 {
            (b.add(STX_UID) as *mut u32).write_unaligned(o.uid);
            // ⚠ The mask bit too: `statx` says which fields it filled, and a uid
            // podbox wrote without setting the bit is one the caller is entitled
            // to ignore.
            mask |= STATX_UID;
        }
        if o.set & memo::SET_GID != 0 {
            (b.add(STX_GID) as *mut u32).write_unaligned(o.gid);
            mask |= STATX_GID;
        }
        (b.add(STX_MASK) as *mut u32).write_unaligned(mask);
    }
}

// ------------------------------------------------------------------ the errno
//
// ⚠ `__errno_location` on glibc and musl both; it is the one name both use for
// the thread's errno slot, and reading `errno` through it is what any C caller
// does.

extern "C" {
    fn __errno_location() -> *mut c_int;
}

const EPERM: c_int = 1;
const EINVAL: c_int = 22;

fn errno() -> c_int {
    unsafe { *__errno_location() }
}

fn clear_errno() {
    unsafe { *__errno_location() = 0 }
}

/// Is this the wall podbox exists for?
///
/// ⛔ **Both `EPERM` and `EINVAL`.** fakeroot tests `EPERM` alone at eight sites
/// and every one of them would return the error here: `chown` to an id this
/// runtime cannot map answers `EINVAL`, which is
/// [`TODO/extract.md`](../../../TODO/extract.md) T-0302's own finding.
/// ⛔ Anything else is the payload's answer and is handed back unchanged: a
/// `chown` on a read-only filesystem must still fail.
fn is_the_wall(e: c_int) -> bool {
    e == EPERM || e == EINVAL
}

// ------------------------------------------------------------- the real calls

crate::real!(pub fn next_chown = "chown"(*const c_char, c_uint, c_uint) -> c_int);
crate::real!(pub fn next_lchown = "lchown"(*const c_char, c_uint, c_uint) -> c_int);
crate::real!(pub fn next_fchown = "fchown"(c_int, c_uint, c_uint) -> c_int);
crate::real!(pub fn next_fchownat = "fchownat"(c_int, *const c_char, c_uint, c_uint, c_int) -> c_int);
crate::real!(pub fn next_stat = "stat"(*const c_char, *mut c_void) -> c_int);
crate::real!(pub fn next_lstat = "lstat"(*const c_char, *mut c_void) -> c_int);
crate::real!(pub fn next_fstat = "fstat"(c_int, *mut c_void) -> c_int);
crate::real!(pub fn next_fstatat = "fstatat"(c_int, *const c_char, *mut c_void, c_int) -> c_int);
crate::real!(pub fn next_stat64 = "stat64"(*const c_char, *mut c_void) -> c_int);
crate::real!(pub fn next_lstat64 = "lstat64"(*const c_char, *mut c_void) -> c_int);
crate::real!(pub fn next_fstat64 = "fstat64"(c_int, *mut c_void) -> c_int);
crate::real!(pub fn next_fstatat64 = "fstatat64"(c_int, *const c_char, *mut c_void, c_int) -> c_int);
crate::real!(pub fn next_xstat = "__xstat"(c_int, *const c_char, *mut c_void) -> c_int);
crate::real!(pub fn next_lxstat = "__lxstat"(c_int, *const c_char, *mut c_void) -> c_int);
crate::real!(pub fn next_fxstat = "__fxstat"(c_int, c_int, *mut c_void) -> c_int);
crate::real!(pub fn next_fxstatat = "__fxstatat"(c_int, c_int, *const c_char, *mut c_void, c_int) -> c_int);
crate::real!(pub fn next_statx = "statx"(c_int, *const c_char, c_int, c_uint, *mut c_void) -> c_int);

/// `AT_SYMLINK_NOFOLLOW`, for the `lchown` half of `fchownat`.
const AT_SYMLINK_NOFOLLOW: c_int = 0x100;
const AT_FDCWD: c_int = -100;

/// The shared body of every `chown`-family entry point.
///
/// ⛔ **Try the real call FIRST.** A runtime where the `chown` would have
/// succeeded must not get a memo instead of the real thing: the memo changes no
/// kernel permission check, and preferring it would make podbox weaker than the
/// bare chroot on a machine that can do the real work.
///
/// ⚠ `uid` and `gid` of `-1` mean "leave it", which is `chown(2)`'s own
/// convention, so `set` records what the payload actually asked for.
unsafe fn owned(
    real: Option<i32>,
    stat_for_ids: Option<(u64, u64)>,
    uid: c_uint,
    gid: c_uint,
    what: &[u8],
) -> c_int {
    let Some(rc) = real else {
        // ⚠ The payload's libc does not define this entry point at all, which
        // this object cannot happen upon: it was resolved through
        // `dlsym(RTLD_NEXT)` and a null there means the symbol is gone.
        say::line(&[what, b": podbox could not resolve the real call"]);
        unsafe { *__errno_location() = EINVAL };
        return -1;
    };
    if rc == 0 {
        return 0;
    }
    let e = errno();
    if !is_the_wall(e) {
        return rc;
    }
    let Some((dev, ino)) = stat_for_ids else {
        // ⛔ podbox could not learn WHICH file, so it has nothing to record and
        // hands the payload the real failure. A success reported with no memo
        // behind it is the lie this whole object exists to avoid.
        say::line(&[
            what,
            b": the call was refused and podbox could not stat the target, so it \
              has nothing to record and the error is the payload's",
        ]);
        unsafe { *__errno_location() = e };
        return -1;
    };
    let mut set = 0u32;
    if uid != c_uint::MAX {
        set |= memo::SET_UID;
    }
    if gid != c_uint::MAX {
        set |= memo::SET_GID;
    }
    let o = memo::Owner { uid, gid, set };
    if !memo::record(dev, ino, o) {
        say::line(&[
            what,
            b": podbox could not write the ownership memo, so the failure is the \
              payload's rather than being hidden",
        ]);
        unsafe { *__errno_location() = e };
        return -1;
    }
    if say::debug() {
        say::line(&[
            what,
            b": the runtime refused it with errno ",
            say::Num::new(e as u64).as_bytes(),
            b"; podbox recorded the intended owner and reported success \
              (TODO/interpose.md T-0704)",
        ]);
    }
    clear_errno();
    0
}

/// `dev` and `ino` for a path, through the REAL `stat`, so this does not recurse
/// into podbox's own interposition.
unsafe fn ids_of_path(path: *const c_char, follow: bool) -> Option<(u64, u64)> {
    let mut st = [0u8; 256];
    let f = if follow { next_stat() } else { next_lstat() };
    let rc = match f {
        Some(f) => unsafe { f(path, st.as_mut_ptr() as *mut c_void) },
        None => {
            // ⚠ glibc before 2.33 exports `stat` only as `__xstat`, so the
            // fallback is not a nicety: on such a payload `dlsym("stat")`
            // answers null. `1` is `_STAT_VER_LINUX` on x86_64.
            let g = if follow { next_xstat() } else { next_lxstat() }?;
            unsafe { g(1, path, st.as_mut_ptr() as *mut c_void) }
        }
    };
    if rc != 0 {
        return None;
    }
    Some(unsafe { dev_ino(st.as_ptr() as *const c_void) })
}

unsafe fn ids_of_fd(fd: c_int) -> Option<(u64, u64)> {
    let mut st = [0u8; 256];
    let rc = match next_fstat() {
        Some(f) => unsafe { f(fd, st.as_mut_ptr() as *mut c_void) },
        None => {
            let g = next_fxstat()?;
            unsafe { g(1, fd, st.as_mut_ptr() as *mut c_void) }
        }
    };
    if rc != 0 {
        return None;
    }
    Some(unsafe { dev_ino(st.as_ptr() as *const c_void) })
}

// ------------------------------------------------------- the exported symbols
//
// ⛔ Every one of these is in `interpose.map` and nothing else is. A symbol
// exported by accident is a symbol some other library in the payload's process
// resolves to podbox.

/// # Safety
/// The payload's own contract for `chown(2)`.
#[no_mangle]
pub unsafe extern "C" fn chown(path: *const c_char, uid: c_uint, gid: c_uint) -> c_int {
    let ids = unsafe { ids_of_path(path, true) };
    let rc = next_chown().map(|f| unsafe { f(path, uid, gid) });
    unsafe { owned(rc, ids, uid, gid, b"chown") }
}

/// # Safety
/// The payload's own contract for `lchown(2)`.
#[no_mangle]
pub unsafe extern "C" fn lchown(path: *const c_char, uid: c_uint, gid: c_uint) -> c_int {
    let ids = unsafe { ids_of_path(path, false) };
    let rc = next_lchown().map(|f| unsafe { f(path, uid, gid) });
    unsafe { owned(rc, ids, uid, gid, b"lchown") }
}

/// # Safety
/// The payload's own contract for `fchown(2)`.
#[no_mangle]
pub unsafe extern "C" fn fchown(fd: c_int, uid: c_uint, gid: c_uint) -> c_int {
    let ids = unsafe { ids_of_fd(fd) };
    let rc = next_fchown().map(|f| unsafe { f(fd, uid, gid) });
    unsafe { owned(rc, ids, uid, gid, b"fchown") }
}

/// # Safety
/// The payload's own contract for `fchownat(2)`.
///
/// ⚠ Hand-written rather than macro-generated, which is the one shape
/// `references/VHSgunzo__pathmap/tree/path-mapping.c:1242-1256` also writes out:
/// the flags decide whether the target is the link or what it points at, and a
/// macro over the family cannot express that.
#[no_mangle]
pub unsafe extern "C" fn fchownat(
    dirfd: c_int,
    path: *const c_char,
    uid: c_uint,
    gid: c_uint,
    flags: c_int,
) -> c_int {
    let ids = if dirfd == AT_FDCWD || unsafe { *path } == b'/' as c_char {
        unsafe { ids_of_path(path, flags & AT_SYMLINK_NOFOLLOW == 0) }
    } else {
        // ⚠ A relative path against a directory descriptor. The real
        // `fstatat` answers it without podbox resolving anything, which is why
        // this object needs none of T-0703's `/proc/self/fd` machinery yet.
        let mut st = [0u8; 256];
        let rc = match next_fstatat() {
            Some(f) => unsafe { f(dirfd, path, st.as_mut_ptr() as *mut c_void, flags) },
            None => match next_fxstatat() {
                Some(g) => unsafe { g(1, dirfd, path, st.as_mut_ptr() as *mut c_void, flags) },
                None => -1,
            },
        };
        if rc == 0 {
            Some(unsafe { dev_ino(st.as_ptr() as *const c_void) })
        } else {
            None
        }
    };
    let rc = next_fchownat().map(|f| unsafe { f(dirfd, path, uid, gid, flags) });
    unsafe { owned(rc, ids, uid, gid, b"fchownat") }
}

/// Every `stat`-family entry point: call the real one, then report the memo.
///
/// ⛔ A macro, so the sixteen shapes cannot drift apart. T-0703's Decision names
/// the same reason for the path families.
macro_rules! stat_entry {
    ($name:ident, $real:ident, ( $($arg:ident : $ty:ty),* $(,)? ), $st:ident) => {
        /// # Safety
        /// The payload's own contract for this entry point.
        #[no_mangle]
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> c_int {
            let Some(f) = $real() else {
                unsafe { *__errno_location() = EINVAL };
                return -1;
            };
            let rc = unsafe { f($($arg),*) };
            if rc == 0 {
                unsafe { report($st) };
            }
            rc
        }
    };
}

stat_entry!(stat, next_stat, (path: *const c_char, st: *mut c_void), st);
stat_entry!(lstat, next_lstat, (path: *const c_char, st: *mut c_void), st);
stat_entry!(fstat, next_fstat, (fd: c_int, st: *mut c_void), st);
stat_entry!(
    fstatat,
    next_fstatat,
    (dirfd: c_int, path: *const c_char, st: *mut c_void, flags: c_int),
    st
);
// ⭐ THE `64` NAMES ARE NOT DUPLICATES, and T-0703 measured why: all four
// `libpython3.*` and `libglib-2.0.so.0` import only `stat64`, `lstat64` and
// `fstatat64`, while `libarchive` and `libdbus-1` import the plain names. Both
// sets are live in one process.
stat_entry!(stat64, next_stat64, (path: *const c_char, st: *mut c_void), st);
stat_entry!(lstat64, next_lstat64, (path: *const c_char, st: *mut c_void), st);
stat_entry!(fstat64, next_fstat64, (fd: c_int, st: *mut c_void), st);
stat_entry!(
    fstatat64,
    next_fstatat64,
    (dirfd: c_int, path: *const c_char, st: *mut c_void, flags: c_int),
    st
);
// ⭐ AND THE `__xstat` SHAPE, which is glibc's pre-2.33 one and which glibc 2.39
// still exports at `GLIBC_2.2.5`. The PAYLOAD's libc decides which shape its
// libraries call, not the host this object was built on.
stat_entry!(
    __xstat,
    next_xstat,
    (ver: c_int, path: *const c_char, st: *mut c_void),
    st
);
stat_entry!(
    __lxstat,
    next_lxstat,
    (ver: c_int, path: *const c_char, st: *mut c_void),
    st
);
stat_entry!(__fxstat, next_fxstat, (ver: c_int, fd: c_int, st: *mut c_void), st);
stat_entry!(
    __fxstatat,
    next_fxstatat,
    (ver: c_int, dirfd: c_int, path: *const c_char, st: *mut c_void, flags: c_int),
    st
);

/// `statx(2)`, and it is the entry point a modern `stat(1)` actually calls.
///
/// # Safety
/// The payload's own contract for `statx(2)`.
#[no_mangle]
pub unsafe extern "C" fn statx(
    dirfd: c_int,
    path: *const c_char,
    flags: c_int,
    mask: c_uint,
    st: *mut c_void,
) -> c_int {
    let Some(f) = next_statx() else {
        unsafe { *__errno_location() = EINVAL };
        return -1;
    };
    // ⛔ The caller's mask is widened to include the identity fields. A caller
    // that did not ask for `STATX_UID` gets it anyway, which `statx(2)` permits
    // -- the kernel may return more than was asked for -- and without it podbox
    // would have no uid to overwrite and no `stx_ino` to key the memo on.
    let rc = unsafe {
        f(
            dirfd,
            path,
            flags,
            mask | STATX_UID | STATX_GID | 0x0000_0100,
            st,
        )
    };
    if rc == 0 {
        unsafe { report_statx(st) };
    }
    rc
}
