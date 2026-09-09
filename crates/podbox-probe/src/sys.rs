//! Raw Linux syscalls, and the only module in this crate that is allowed
//! `unsafe`.
//!
//! `TOOL.md` section 3.3 requires `unsafe` be confined to one module per
//! subsystem with the safe wrapper adjacent. Everything below the `syscall*`
//! functions is safe and returns `Result<i64, Errno>`.
//!
//! ⛔ No libc wrapper is used, and that is the point rather than an economy.
//! `TODO/deps.md` T-0901 rules the probe set is hand-declared and measured
//! before a crate is considered. Beyond that, a libc wrapper is the wrong
//! instrument here: glibc's `setuid(3)` runs a multi-threaded id-change dance
//! and musl's runs another, so the errno a wrapper returns is the library's
//! answer and not the kernel's. `TODO/probe.md` T-0101 requires the kernel's.
//!
//! ⚠ The raw return value carries the errno directly: a value in
//! `-4095..=-1` is `-errno`. Nothing passes through `errno`'s thread-local,
//! so there is no window in which another call overwrites it.

//! ---------------------------------------------------------------------------
//!
//! ⭐ **Every syscall number and every kernel struct layout below comes from a
//! crate, per architecture, and none of them is written here.**
//! [`TODO/deps.md`](../../../TODO/deps.md) T-0911 is the ruling and the
//! measurement. Until 2026-09-09 this module declared x86_64's numbers by hand
//! and refused to compile anywhere else, which made podbox a single-
//! architecture runtime for no reason but the table.
//!
//! - [`syscalls`] carries the kernel's own table for fourteen architectures and
//!   the `syscall` instruction for each. `nr!(openat, __NR_openat)` is that table,
//!   so an architecture podbox has never been built on gets its numbers from
//!   the kernel's source rather than from a session's transcription.
//! - [`linux_raw_sys`] carries `struct stat` and `struct statfs64` generated
//!   from the kernel headers. ⚠ Their **shape differs by architecture**,
//!   `stat` is 144 bytes on x86_64 and 128 on aarch64, which is precisely the
//!   class of thing a hand-written `#[repr(C)]` gets wrong silently, by handing
//!   the kernel a buffer shorter than what it writes.
//!
//! ⛔ **The transcription risk did not go away, it moved to one place and is
//! asserted.** A name that does not exist for an architecture is a compile
//! error, never a wrong number: `Sysno` has no variant for a syscall the kernel
//! does not define there. The `#[cfg]` list below names the architectures whose
//! kernels use `asm-generic/unistd.h`, which omits `open`, `stat`, `mkdir`,
//! `unlink`, `chown`, `lchown`, `readlink`, `mknod` and `dup2` in favour of the
//! `*at` forms, get that list wrong in either direction and the build fails
//! rather than measuring something nobody named.

/// A kernel error number. Never a boolean: `TODO/probe.md` T-0101 rules that
/// `EINVAL` from `setuid` or `chown` means an unmapped id and points at a
/// mapping, while `EPERM` points at a policy, and they are different remedies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Errno(pub i32);

impl Errno {
    /// The symbolic name, or `E<n>` where this table does not carry one.
    ///
    /// ⚠ `E<n>` is a statement that the number was measured and the name is
    /// not known here, not a guess at which errno it was.
    pub fn name(self) -> String {
        let n = match self.0 {
            1 => "EPERM",
            2 => "ENOENT",
            3 => "ESRCH",
            4 => "EINTR",
            5 => "EIO",
            9 => "EBADF",
            11 => "EAGAIN",
            12 => "ENOMEM",
            13 => "EACCES",
            14 => "EFAULT",
            16 => "EBUSY",
            17 => "EEXIST",
            18 => "EXDEV",
            19 => "ENODEV",
            20 => "ENOTDIR",
            21 => "EISDIR",
            22 => "EINVAL",
            24 => "EMFILE",
            26 => "ETXTBSY",
            28 => "ENOSPC",
            30 => "EROFS",
            36 => "ENAMETOOLONG",
            38 => "ENOSYS",
            39 => "ENOTEMPTY",
            40 => "ELOOP",
            95 => "EOPNOTSUPP",
            _ => return format!("E{}", self.0),
        };
        n.to_string()
    }
}

pub const EPERM: Errno = Errno(1);
pub const ENOENT: Errno = Errno(2);
pub const ESRCH: Errno = Errno(3);
pub const EINTR: Errno = Errno(4);
pub const EBADF: Errno = Errno(9);
pub const EACCES: Errno = Errno(13);
pub const EEXIST: Errno = Errno(17);
pub const EINVAL: Errno = Errno(22);
pub const ENOSYS: Errno = Errno(38);
pub const ENAMETOOLONG: Errno = Errno(36);
/// ⚠ On Linux `EAGAIN` and `EWOULDBLOCK` are the same number, and `flock` with
/// `LOCK_NB` returns it for "somebody else holds this". It is named here for
/// the reader, because "try again later" and "in use" are different sentences.
pub const EWOULDBLOCK: Errno = Errno(11);

pub type Sysres = Result<i64, Errno>;

// ---------------------------------------------------------------- syscall nrs
//
// ⛔ Not one number is written here. `Sysno::<name> as i64` is the kernel's own
// table for the architecture being built, carried by the `syscalls` crate.

// ---------------------------------------------------- where a number comes from
//
// ⭐ **T-0912. TWO SOURCES, ONE PER LINE, AND THE ARCHITECTURE PICKS.**
//
// `syscalls` carries the kernel's table for fourteen architectures, and until
// 2026-09-09 podbox took every number from it. ⛔ It also carries
// `#![feature(asm_experimental_arch)]` for `mips`, `mips64`, `s390x`,
// `powerpc` and `powerpc64`, which is a **crate-level attribute**: the whole
// crate refuses to compile for those targets on stable, whatever podbox does.
// That is why `powerpc64le-unknown-linux-musl` was the one architecture this
// workspace could not build for.
//
// ⭐ **The gate is the crate's and not the compiler's, and that was measured
// rather than assumed.** `experiments/260-multiarch.sh` clause 3 compiles a
// `powerpc64` `sc` sequence against the real rustc:
//
//     ⚠ powerpc64 inline asm COMPILES on rustc 1.98.1
//
// So on exactly the five architectures the crate gates, podbox takes the
// numbers from `linux-raw-sys` -- which it already depends on for the kernel
// structs -- and carries its own trap. ⛔ Not a fork of `syscalls` and not a
// patch to it in this tree: the numbers are already available from a crate
// podbox takes, and one more source of them is one more place to disagree.
//
// ⚠ **Both spellings are on the same line**, `Sysno::openat` and
// `__NR_openat`, so a reader can check the pair and a rename cannot silently
// take a different number on one architecture.

/// ⛔ The five the `syscalls` crate gates behind nightly. Written out at each
/// use rather than aliased, because Rust has no `cfg` alias without a
/// `build.rs`, and a `build.rs` to save three repetitions is a worse trade.
#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "powerpc",
    target_arch = "powerpc64",
)))]
use syscalls::Sysno;

#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "powerpc",
    target_arch = "powerpc64",
)))]
macro_rules! nr {
    ($sysno:ident, $raw:ident) => {
        crate::sys::Sysno::$sysno as i64
    };
}

#[cfg(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "powerpc",
    target_arch = "powerpc64",
))]
macro_rules! nr {
    ($sysno:ident, $raw:ident) => {
        linux_raw_sys::general::$raw as i64
    };
}

pub const SYS_READ: i64 = nr!(read, __NR_read);
pub const SYS_WRITE: i64 = nr!(write, __NR_write);
pub const SYS_CLOSE: i64 = nr!(close, __NR_close);
pub const SYS_GETPID: i64 = nr!(getpid, __NR_getpid);
pub const SYS_CLONE: i64 = nr!(clone, __NR_clone);
pub const SYS_EXECVE: i64 = nr!(execve, __NR_execve);
pub const SYS_WAIT4: i64 = nr!(wait4, __NR_wait4);
pub const SYS_KILL: i64 = nr!(kill, __NR_kill);
pub const SYS_PTRACE: i64 = nr!(ptrace, __NR_ptrace);
pub const SYS_GETUID: i64 = nr!(getuid, __NR_getuid);
pub const SYS_GETGID: i64 = nr!(getgid, __NR_getgid);
pub const SYS_SETUID: i64 = nr!(setuid, __NR_setuid);
pub const SYS_SETGID: i64 = nr!(setgid, __NR_setgid);
pub const SYS_SETSID: i64 = nr!(setsid, __NR_setsid);
pub const SYS_GETGROUPS: i64 = nr!(getgroups, __NR_getgroups);
pub const SYS_SETGROUPS: i64 = nr!(setgroups, __NR_setgroups);
pub const SYS_STATFS: i64 = nr!(statfs, __NR_statfs);
pub const SYS_FLOCK: i64 = nr!(flock, __NR_flock);
pub const SYS_FCNTL: i64 = nr!(fcntl, __NR_fcntl);
pub const SYS_PIVOT_ROOT: i64 = nr!(pivot_root, __NR_pivot_root);
pub const SYS_PRCTL: i64 = nr!(prctl, __NR_prctl);
pub const SYS_CHROOT: i64 = nr!(chroot, __NR_chroot);
pub const SYS_MOUNT: i64 = nr!(mount, __NR_mount);
pub const SYS_UMOUNT2: i64 = nr!(umount2, __NR_umount2);
pub const SYS_SETHOSTNAME: i64 = nr!(sethostname, __NR_sethostname);
pub const SYS_EXIT_GROUP: i64 = nr!(exit_group, __NR_exit_group);
pub const SYS_UNSHARE: i64 = nr!(unshare, __NR_unshare);
pub const SYS_OPENAT: i64 = nr!(openat, __NR_openat);
pub const SYS_PIPE2: i64 = nr!(pipe2, __NR_pipe2);
pub const SYS_PROCESS_VM_READV: i64 = nr!(process_vm_readv, __NR_process_vm_readv);
pub const SYS_KCMP: i64 = nr!(kcmp, __NR_kcmp);
pub const SYS_SECCOMP: i64 = nr!(seccomp, __NR_seccomp);
pub const SYS_GETRANDOM: i64 = nr!(getrandom, __NR_getrandom);
pub const SYS_MEMFD_CREATE: i64 = nr!(memfd_create, __NR_memfd_create);
pub const SYS_OPEN_TREE: i64 = nr!(open_tree, __NR_open_tree);
pub const SYS_MOVE_MOUNT: i64 = nr!(move_mount, __NR_move_mount);
pub const SYS_FSOPEN: i64 = nr!(fsopen, __NR_fsopen);
pub const SYS_FSCONFIG: i64 = nr!(fsconfig, __NR_fsconfig);
pub const SYS_FSMOUNT: i64 = nr!(fsmount, __NR_fsmount);
pub const SYS_PIDFD_GETFD: i64 = nr!(pidfd_getfd, __NR_pidfd_getfd);
// ⭐ M4's supervisor. TODO/supervise.md T-0601 and T-0602: a pidfd per direct
// child, `waitid` for its status, and `ppoll` to wait on the CONDITION rather
// than on a guessed duration.
// ⚠ `ppoll` and not `poll`: `poll` does not exist on aarch64 or riscv64, and
// this workspace compiles for six architectures (TODO/deps.md T-0911).
pub const SYS_PIDFD_OPEN: i64 = nr!(pidfd_open, __NR_pidfd_open);
pub const SYS_WAITID: i64 = nr!(waitid, __NR_waitid);
pub const SYS_PPOLL: i64 = nr!(ppoll, __NR_ppoll);
pub const SYS_LANDLOCK_CREATE_RULESET: i64 =
    nr!(landlock_create_ruleset, __NR_landlock_create_ruleset);
pub const SYS_DUP3: i64 = nr!(dup3, __NR_dup3);
// ⭐ M3's entry sequence. TODO/enter.md T-0504: the rootfs is held as a
// directory DESCRIPTOR and entered with `fchdir` then `chroot(".")`, so nothing
// between checking the path and changing the root can swap it.
pub const SYS_CHDIR: i64 = nr!(chdir, __NR_chdir);
pub const SYS_FCHDIR: i64 = nr!(fchdir, __NR_fchdir);

// ⭐ The `*at` family, taken by `crates/podbox-extract` (TODO/extract.md
// T-0304). Extraction resolves every entry against a directory FILE
// DESCRIPTOR rather than against a path, because a path is re-resolved by the
// kernel on every call and the tree is being written to between calls.
//
// ⛔ Since 2026-09-09 they are also what every path-taking wrapper in this
// module uses, on EVERY architecture and not only on the ones that have
// nothing else. `open(path)` is `openat(AT_FDCWD, path)` by definition, so
// routing through the `*at` form costs nothing and removes nine per-
// architecture cases. The three call sites where the entry point IS the
// measurement keep their own identity below.
pub const SYS_FCHMOD: i64 = nr!(fchmod, __NR_fchmod);
pub const SYS_GETDENTS64: i64 = nr!(getdents64, __NR_getdents64);
pub const SYS_MKDIRAT: i64 = nr!(mkdirat, __NR_mkdirat);
pub const SYS_FCHOWNAT: i64 = nr!(fchownat, __NR_fchownat);
pub const SYS_UNLINKAT: i64 = nr!(unlinkat, __NR_unlinkat);
pub const SYS_LINKAT: i64 = nr!(linkat, __NR_linkat);
pub const SYS_SYMLINKAT: i64 = nr!(symlinkat, __NR_symlinkat);
pub const SYS_READLINKAT: i64 = nr!(readlinkat, __NR_readlinkat);
pub const SYS_FCHMODAT: i64 = nr!(fchmodat, __NR_fchmodat);
pub const SYS_UTIMENSAT: i64 = nr!(utimensat, __NR_utimensat);
pub const SYS_MKNODAT: i64 = nr!(mknodat, __NR_mknodat);
/// ⚠ Linux 5.6. A kernel without it answers `ENOSYS`, which is why
/// `podbox-extract` carries an `O_NOFOLLOW` walk beside it rather than
/// requiring it.
pub const SYS_OPENAT2: i64 = nr!(openat2, __NR_openat2);

// ------------------------------------------------- stat, which has three names
//
// ⛔ **The same syscall is called three different things by the kernel, and on
// two architectures it writes a different struct.** Measured against the
// kernel's own tables on 2026-09-09:
//
//   newfstatat + struct stat     x86_64, riscv64, powerpc64, s390x
//   fstatat    + struct stat     aarch64, loongarch64
//   fstatat64  + struct stat64   arm, x86        (32-bit: `stat` is the narrow
//                                                 form and would truncate)
//
// ⚠ The pairing is the point. Taking `fstatat64`'s number with `struct stat`'s
// buffer is a kernel write of the wrong shape into the right-sized hole, which
// is silent. The type and the number move together here so they cannot drift.

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "sparc64",
    target_arch = "mips64",
))]
mod stat_call {
    pub use linux_raw_sys::general::stat as KernelStat;
    pub const SYS_FSTATAT: i64 = nr!(newfstatat, __NR_newfstatat);
}

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "loongarch64",
    target_arch = "csky"
))]
mod stat_call {
    pub use linux_raw_sys::general::stat as KernelStat;
    pub const SYS_FSTATAT: i64 = nr!(fstatat, __NR_fstatat);
}

#[cfg(any(
    target_arch = "arm",
    target_arch = "x86",
    target_arch = "powerpc",
    target_arch = "mips",
    target_arch = "sparc",
))]
mod stat_call {
    pub use linux_raw_sys::general::stat64 as KernelStat;
    pub const SYS_FSTATAT: i64 = nr!(fstatat64, __NR_fstatat64);
}

pub use stat_call::{KernelStat, SYS_FSTATAT};

// -------------------------------------------- the three that ARE the question
//
// ⭐ `TODO/probe.md` T-0101 measures the errno of a NAMED syscall, so for these
// three the entry point is not an implementation detail: it is what the row
// says was asked. On an architecture whose kernel has no `chown(2)` the honest
// answer is not to quietly call `fchownat(2)` and print `chown`, it is to call
// what exists and SAY which, which is what [`Entry`] carries into the report.

/// A syscall this probe set measures by name, and the entry point this
/// architecture actually has for it.
#[derive(Clone, Copy)]
pub struct Entry {
    pub nr: i64,
    /// The name the row prints. ⛔ Always the name of the syscall that was
    /// really issued, never the one the probe set is called after.
    pub name: &'static str,
    /// True where this kernel has no dedicated entry point and the `*at` form
    /// is the only way to ask. The report says so rather than hiding it.
    pub via_at: bool,
}

/// ⚠ **The one list in podbox that is a claim about the kernel rather than a
/// value read from it**: the architectures taking their table from
/// `asm-generic/unistd.h`, which defines only the `*at` forms.
///
/// ⛔ It is written exactly twice, positive and negated, on adjacent lines, and
/// the compiler checks it in both directions. Name an architecture here that
/// does have `chown` and nothing is lost but a more specific row; fail to name
/// one that does not and the build fails on `Sysno::chown`, which has no
/// variant there. Neither mistake can produce a wrong number at runtime, and
/// that is the property the old hand-written table did not have.
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "csky",
))]
mod entry_points {
    use super::Entry;
    pub const CHOWN: Entry = Entry {
        nr: nr!(fchownat, __NR_fchownat),
        name: "fchownat",
        via_at: true,
    };
    pub const LCHOWN: Entry = Entry {
        nr: nr!(fchownat, __NR_fchownat),
        name: "fchownat(AT_SYMLINK_NOFOLLOW)",
        via_at: true,
    };
    pub const MKNOD: Entry = Entry {
        nr: nr!(mknodat, __NR_mknodat),
        name: "mknodat",
        via_at: true,
    };
}

#[cfg(not(any(
    target_arch = "aarch64",
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "csky",
)))]
mod entry_points {
    use super::Entry;
    pub const CHOWN: Entry = Entry {
        nr: nr!(chown, __NR_chown),
        name: "chown",
        via_at: false,
    };
    pub const LCHOWN: Entry = Entry {
        nr: nr!(lchown, __NR_lchown),
        name: "lchown",
        via_at: false,
    };
    pub const MKNOD: Entry = Entry {
        nr: nr!(mknod, __NR_mknod),
        name: "mknod",
        via_at: false,
    };
}

pub use entry_points::{CHOWN, LCHOWN, MKNOD};

// ---------------------------------------------------------------- constants
pub const CLONE_NEWNS: u64 = 0x0002_0000;
pub const CLONE_NEWUTS: u64 = 0x0400_0000;
pub const CLONE_NEWUSER: u64 = 0x1000_0000;
pub const CLONE_NEWPID: u64 = 0x2000_0000;
pub const SIGCHLD: u64 = 17;

pub const MS_REC: u64 = 0x4000;
pub const MS_SLAVE: u64 = 0x0008_0000;

pub const S_IFMT: u32 = 0o170000;
pub const S_IFCHR: u64 = 0o0020000;
/// `makedev(1, 3)`, that is `/dev/null`. A real device number, unlike
/// `makedev(0, 0)`, which is `WHITEOUT_DEV` and is exempt from the capability
/// check. `TODO/probe.md` T-0106 is that pair.
pub const DEV_1_3: u64 = 0x103;

pub const O_RDONLY: u64 = 0;
pub const O_WRONLY: u64 = 1;
pub const O_RDWR: u64 = 2;
pub const O_CREAT: u64 = 0o100;
/// ⛔ Every scratch file a probe makes is created with this. A probe that
/// overwrites whatever is in its way has destroyed data to measure a
/// permission, and the errno it then reports is about its own file.
pub const O_EXCL: u64 = 0o200;
/// ⛔ Every probe that opens a terminal-shaped device passes this. Without it
/// an `open` of a tty can make it the prober's controlling terminal, which is
/// a mutation of the process doing the measuring.
pub const O_NOCTTY: u64 = 0o400;
pub const O_DIRECTORY: u64 = 0o200000;
pub const O_CLOEXEC: u64 = 0o2000000;
pub const O_TRUNC: u64 = 0o1000;
/// ⛔ The one flag that makes an `open` of a path component a statement about
/// that component rather than about wherever a symlink pointed. Every directory
/// descent in `crates/podbox-extract` carries it.
pub const O_NOFOLLOW: u64 = 0o400000;
pub const O_PATH: u64 = 0o10000000;

/// `openat2(2)`'s `struct open_how`. Three `u64`s, in this order, and the
/// kernel is told the size so it can reject a struct it does not know.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct OpenHow {
    pub flags: u64,
    pub mode: u64,
    pub resolve: u64,
}

/// ⛔ Refuse a symlink anywhere in the walk, including a trailing one. This is
/// what makes the check immune to a symlink the same layer created a moment
/// ago, which is the defect `TODO/extract.md` T-0304 exists for.
pub const RESOLVE_NO_SYMLINKS: u64 = 0x04;
/// ⛔ Refuse anything resolving outside the directory the descriptor names,
/// `..` and an absolute path included. The kernel enforces it; a lexical check
/// alone can be defeated by a component created between the check and the use.
pub const RESOLVE_BENEATH: u64 = 0x08;

/// `openat2`'s "treat the starting descriptor as `/`".
///
/// ⭐ Symlinks ARE followed and an absolute target is rebased onto that
/// descriptor, so nothing resolves outside it and `..` at the top stays at the
/// top. [`TODO/complete.md`](../../../TODO/complete.md) T-0405 turns on the
/// difference between this and [`RESOLVE_BENEATH`] `| ` [`RESOLVE_NO_SYMLINKS`]:
/// during extraction a symlink is the attack, and afterwards a rootfs's own
/// internal links are how its files are reached.
pub const RESOLVE_IN_ROOT: u64 = 0x10;

pub const AT_SYMLINK_NOFOLLOW: u64 = 0x100;
pub const AT_REMOVEDIR: u64 = 0x200;
pub const AT_EMPTY_PATH: u64 = 0x1000;

pub const S_IFDIR: u32 = 0o040000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFLNK: u32 = 0o120000;

/// `flock(2)` operations. ⚠ `LOCK_NB` is not optional anywhere in this tree:
/// `RULES.md` section 8 forbids an unbounded wait, and a blocking `flock` on a
/// lock another process holds through an exec is exactly one.
pub const LOCK_SH: u64 = 1;
pub const LOCK_EX: u64 = 2;
pub const LOCK_NB: u64 = 4;
pub const LOCK_UN: u64 = 8;

/// `fcntl(2)`'s descriptor-flag commands, and the only flag that lives there.
///
/// ⭐ `FD_CLOEXEC` is a property of the DESCRIPTOR, not of the open file
/// description, which is why it can be cleared on one fd immediately before an
/// exec without affecting any other fd onto the same file.
/// [`TODO/image.md`](../../../TODO/image.md) T-0211 is that distinction: an fd
/// opened inheritable is inherited by every fork, and an fd made inheritable
/// one call before `execve` is inherited by exactly the payload.
pub const F_GETFD: u64 = 1;
pub const F_SETFD: u64 = 2;
pub const FD_CLOEXEC: u64 = 1;

/// `AT_FDCWD` is `-100`, and every syscall taking it wants it sign-extended.
pub const AT_FDCWD: u64 = -100i64 as u64;

pub const PR_SET_PDEATHSIG: u64 = 1;
pub const PR_SET_NO_NEW_PRIVS: u64 = 38;

pub const SECCOMP_SET_MODE_FILTER: u64 = 1;
pub const SECCOMP_FILTER_FLAG_NEW_LISTENER: u64 = 1 << 3;

pub const FSCONFIG_CMD_CREATE: u64 = 6;
pub const OPEN_TREE_CLONE: u64 = 1;
pub const MOVE_MOUNT_F_EMPTY_PATH: u64 = 0x0000_0004;

// ---------------------------------------------------------------- the trap
//
// ⛔ The instruction and the register convention are per architecture, x86_64
// puts the number in rax and the arguments in rdi rsi rdx r10 r8 r9, aarch64
// puts them in x8 and x0..x5, and each has its own clobber set, and getting
// one wrong is not a build failure, it is a syscall with the arguments in the
// wrong places. `syscalls::raw` carries the correct sequence for every
// architecture it supports, which is why podbox no longer writes one.
//
// ⚠ The raw return value is still read the same way: `split` below turns the
// kernel's `-4095..=-1` window into an `Errno`. Nothing passes through libc's
// `errno` thread-local, which is `TODO/probe.md` T-0101's requirement and is
// the reason `syscalls::raw` is taken rather than its `Result`-returning API.

/// # Safety
/// The caller states that this syscall with these arguments is sound: any
/// pointer argument is valid for the kernel's access, and the effect on this
/// process is one the caller intends.
#[cfg(not(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "powerpc",
    target_arch = "powerpc64",
)))]
pub unsafe fn syscall6(nr: i64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    unsafe {
        syscalls::raw::syscall6(
            nr as usize,
            a0 as usize,
            a1 as usize,
            a2 as usize,
            a3 as usize,
            a4 as usize,
            a5 as usize,
        ) as i64
    }
}

// ------------------------------------------- podbox's own trap, T-0912
//
// ⭐ **Two architectures, and both are RUN rather than only compiled.**
// `experiments/260-multiarch.sh` builds `podbox` for
// `powerpc64le-unknown-linux-musl` and `s390x-unknown-linux-musl` and executes
// it under `qemu-ppc64le-static` and `qemu-s390x-static`, so the register
// conventions below are measured and not asserted. ⛔ That distinction is the
// whole reason these two are here and `mips` is not: getting a convention wrong
// is not a build failure, it is a syscall with the arguments in the wrong
// places, and asm nobody has executed is a claim.

/// powerpc64, both endiannesses.
///
/// ⛔ **The error convention is NOT x86_64's and this is where that is
/// handled.** powerpc does not return `-errno` in `r3`: it returns the positive
/// errno there and sets **CR0.SO** to say the call failed. `split` upstream
/// reads the kernel's `-4095..=-1` window, so the value is negated here when SO
/// is set, and every caller then sees one convention.
///
/// r0 carries the number, r3 through r8 the six arguments, `sc` is the trap,
/// and r9 through r12, cr0, ctr and xer are clobbered.
#[cfg(target_arch = "powerpc64")]
pub unsafe fn syscall6(nr: i64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "sc",
            // ⚠ `bns` is "branch if not summary overflow", on cr0. A local
            // numeric label, because a named one would collide when this is
            // inlined twice into one function.
            "bns 2f",
            "neg 3, 3",
            "2:",
            inlateout("r0") nr as u64 => _,
            inlateout("r3") a0 => ret,
            in("r4") a1,
            in("r5") a2,
            in("r6") a3,
            in("r7") a4,
            in("r8") a5,
            lateout("r9") _,
            lateout("r10") _,
            lateout("r11") _,
            lateout("r12") _,
            options(nostack)
        );
    }
    ret as i64
}

/// s390x.
///
/// ⚠ r1 carries the number and r2 through r7 the six arguments; `svc 0` is the
/// trap and the result comes back in r2 as `-errno`, which is the convention
/// `split` already reads.
#[cfg(target_arch = "s390x")]
pub unsafe fn syscall6(nr: i64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "svc 0",
            in("r1") nr as u64,
            inlateout("r2") a0 => ret,
            in("r3") a1,
            in("r4") a2,
            in("r5") a3,
            in("r6") a4,
            in("r7") a5,
            options(nostack)
        );
    }
    ret as i64
}

/// ⛔ **mips and mips64 are still gated, and this says so at compile time
/// rather than shipping a convention nobody has run.**
///
/// T-0912's Approach names three families; two of them are here and executed
/// under an emulator by `experiments/260-multiarch.sh`. mips is not, and the
/// reason is not effort: `qemu-mips*-static` is installed on this host, but
/// o32 passes arguments five and six **on the caller's stack**, which
/// `options(nostack)` forbids and which a wrong frame layout gets wrong
/// silently. ⚠ What would clear it is the same thing that cleared these two: a
/// clause in `260-multiarch.sh` that builds for a mips target and RUNS the
/// binary under the emulator.
#[cfg(any(target_arch = "mips", target_arch = "mips64", target_arch = "powerpc"))]
compile_error!(
    "podbox has no measured syscall trap for this architecture. TODO/deps.md \
     T-0912: powerpc64 and s390x carry podbox's own, executed under an emulator; \
     mips, mips64 and 32-bit powerpc do not, and shipping a register convention \
     nobody has run is what that entry refuses."
);

/// Split the raw return into a value and an errno.
///
/// ⛔ The `-4095..=-1` window is the kernel's own convention. A return outside
/// it is a value, including a large negative one from a syscall that returns a
/// signed quantity.
pub fn split(ret: i64) -> Sysres {
    if (-4095..0).contains(&ret) {
        Err(Errno(-ret as i32))
    } else {
        Ok(ret)
    }
}

/// # Safety
/// As `syscall6`.
pub unsafe fn sys(nr: i64, args: [u64; 6]) -> Sysres {
    split(unsafe { syscall6(nr, args[0], args[1], args[2], args[3], args[4], args[5]) })
}

// -------------------------------------------------- fds that a fork must shed
//
// ⛔ `O_CLOEXEC` is close-on-EXEC and there is no close-on-FORK. A `fork`
// duplicates every descriptor, and `flock(2)` is held on the OPEN FILE
// DESCRIPTION the duplicates share, so the lock lives until the last of them is
// closed. A child that never execs, and this tree makes one per `clone`
// probe, therefore holds an image lock that `O_CLOEXEC` cannot take from it.
// [`TODO/image.md`](../../../TODO/image.md) T-0211.
//
// ⭐ The list is exact rather than a blanket close of everything above stderr:
// podbox knows which descriptors these are, and closing a range would also shut
// fds a caller handed podbox deliberately.

/// How many such fds may be registered at once. ⚠ Fixed, because the child
/// drains this between `clone` and `execve`, where allocation is not permitted.
/// podbox holds one image lock per container; sixteen is far past what any
/// single process does, and a seventeenth is refused by name rather than
/// dropped silently.
pub const FORK_CLOSE_SLOTS: usize = 16;

static FORK_CLOSE: [core::sync::atomic::AtomicI64; FORK_CLOSE_SLOTS] =
    [const { core::sync::atomic::AtomicI64::new(-1) }; FORK_CLOSE_SLOTS];

/// Register `fd` to be closed in every child [`clone_fork`] makes.
///
/// Returns false where every slot is taken, and the caller reports that rather
/// than proceeding: an unregistered lock fd is the defect, not a detail.
pub fn close_in_children(fd: i64) -> bool {
    use core::sync::atomic::Ordering;
    for slot in &FORK_CLOSE {
        if slot
            .compare_exchange(-1, fd, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return true;
        }
    }
    false
}

/// Stop shedding `fd`, either because it is being closed or because it is about
/// to be handed to a payload on purpose.
pub fn stop_closing_in_children(fd: i64) {
    use core::sync::atomic::Ordering;
    for slot in &FORK_CLOSE {
        let _ = slot.compare_exchange(fd, -1, Ordering::AcqRel, Ordering::Acquire);
    }
}

/// Close every registered fd. ⛔ Called in the child by [`clone_fork`] itself
/// and nowhere else, so a caller that forgets cannot exist.
///
/// ⚠ Async-signal-safe: an atomic load and a `close(2)` per slot, no allocation
/// and no lock.
fn shed_registered_fds() {
    use core::sync::atomic::Ordering;
    for slot in &FORK_CLOSE {
        let fd = slot.load(Ordering::Acquire);
        if fd >= 0 {
            let _ = close(fd);
        }
    }
}

// ---------------------------------------------------------------- wrappers
//
// Each takes what the kernel takes and returns the kernel's answer. Nothing
// here interprets a verdict; that is `probes.rs`'s job.

/// A NUL-terminated byte buffer whose pointer is stable for as long as it is
/// held. Built before any fork, so nothing after a fork allocates.
pub struct CBuf(Vec<u8>);

impl CBuf {
    /// Returns `None` when `s` contains an interior NUL, because a path the
    /// kernel would silently truncate is not the path the caller named.
    pub fn new(s: &str) -> Option<CBuf> {
        if s.as_bytes().contains(&0) {
            return None;
        }
        let mut v = Vec::with_capacity(s.len() + 1);
        v.extend_from_slice(s.as_bytes());
        v.push(0);
        Some(CBuf(v))
    }

    pub fn ptr(&self) -> u64 {
        self.0.as_ptr() as u64
    }
}

/// The empty C string, for syscalls taking a path they must not resolve.
pub fn cempty() -> CBuf {
    CBuf(vec![0])
}

pub fn close(fd: i64) -> Sysres {
    unsafe { sys(SYS_CLOSE, [fd as u64, 0, 0, 0, 0, 0]) }
}

/// `fcntl(2)`, for the commands whose argument is an integer.
///
/// ⚠ Restricted to `F_GETFD`, `F_SETFD` and `F_DUPFD_CLOEXEC` by its own
/// callers rather than by its signature: the commands taking a pointer argument
/// (`F_GETLK`, `F_SETLK`, `F_GETOWN_EX`) need a struct this takes no room for,
/// and calling one through here would hand the kernel an integer where it reads
/// an address.
pub fn fcntl(fd: i64, cmd: u64, arg: u64) -> Sysres {
    unsafe { sys(SYS_FCNTL, [fd as u64, cmd, arg, 0, 0, 0]) }
}

/// `F_DUPFD_CLOEXEC`, which is `F_LINUX_SPECIFIC_BASE + 6`.
pub const F_DUPFD_CLOEXEC: u64 = 1030;

/// A second descriptor for the same open file description, at an unused number.
///
/// ⛔ **Why this and not [`dup2`].** A caller that wants the child's stdout to
/// be its stderr cannot pass `(1, 2)` to a `dup2`-then-`close` loop: the close
/// would take the child's stderr with it. It dups first, hands over the
/// duplicate, and the loop closes the duplicate instead.
///
/// ⚠ `O_CLOEXEC` on the duplicate is deliberate and is not inherited by the
/// `dup2` that installs it: a duplicate the caller forgot would otherwise reach
/// the payload, and a payload holding an extra descriptor onto podbox's own
/// stderr is a difference from docker nobody asked for.
pub fn dup_cloexec(fd: i64) -> Sysres {
    // ⚠ From 3 upwards: below that are the standard descriptors, and a
    // duplicate landing on one of them is the bug this function exists to
    // avoid.
    fcntl(fd, F_DUPFD_CLOEXEC, 3)
}

/// ⛔ `chroot(2)` on a path, used only as `chroot(".")` after an `fchdir` onto
/// a descriptor already held. TODO/enter.md T-0504: `chroot` follows a symlink,
/// so a path checked and then passed is a path that can be swapped in between.
pub fn chroot(path: &CBuf) -> Sysres {
    unsafe { sys(SYS_CHROOT, [path.ptr(), 0, 0, 0, 0, 0]) }
}

pub fn chdir(path: &CBuf) -> Sysres {
    unsafe { sys(SYS_CHDIR, [path.ptr(), 0, 0, 0, 0, 0]) }
}

pub fn fchdir(fd: i64) -> Sysres {
    unsafe { sys(SYS_FCHDIR, [fd as u64, 0, 0, 0, 0, 0]) }
}

/// `open(2)` by name, `openat(2)` in fact, see [`stat`] for why every
/// path-taking utility in this module routes through the `*at` form.
pub fn open(path: &CBuf, flags: u64, mode: u64) -> Sysres {
    openat(AT_FDCWD as i64, path, flags, mode)
}

pub fn read(fd: i64, buf: &mut [u8]) -> Sysres {
    unsafe {
        sys(
            SYS_READ,
            [
                fd as u64,
                buf.as_mut_ptr() as u64,
                buf.len() as u64,
                0,
                0,
                0,
            ],
        )
    }
}

pub fn write(fd: i64, buf: &[u8]) -> Sysres {
    unsafe {
        sys(
            SYS_WRITE,
            [fd as u64, buf.as_ptr() as u64, buf.len() as u64, 0, 0, 0],
        )
    }
}

/// `getrandom(2)`. ⛔ The only way podbox obtains random bytes.
///
/// Reading `/dev/urandom` with `std::fs::read` has no EOF and allocated 13 GB
/// here before the OOM killer took the process
/// ([`TODO/supervise.md`](../../../TODO/supervise.md) T-0602), and inside a
/// chroot the file is a shim this project wrote
/// ([`TODO/complete.md`](../../../TODO/complete.md) T-0401), so it is not an
/// entropy source at all. ⚠ The kernel may return short; the caller loops.
pub fn getrandom(buf: &mut [u8]) -> Sysres {
    unsafe {
        sys(
            SYS_GETRANDOM,
            [buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0, 0],
        )
    }
}

pub fn unlink(path: &CBuf) -> Sysres {
    unlinkat(AT_FDCWD as i64, path, 0)
}

pub fn mkdir(path: &CBuf, mode: u64) -> Sysres {
    mkdirat(AT_FDCWD as i64, path, mode)
}

// ------------------------------------------------------- the `*at` wrappers
//
// ⛔ Each resolves against a directory DESCRIPTOR. `crates/podbox-extract`
// holds one open on the destination for the whole of a layer, so no path it
// writes is ever re-resolved from the root through components another entry
// may have replaced in between (TODO/extract.md T-0304).

pub fn openat(dirfd: i64, path: &CBuf, flags: u64, mode: u64) -> Sysres {
    unsafe { sys(SYS_OPENAT, [dirfd as u64, path.ptr(), flags, mode, 0, 0]) }
}

/// `openat2(2)`, whose `resolve` mask is enforced by the kernel rather than by
/// the caller.
///
/// ⚠ Returns `ENOSYS` before Linux 5.6, and `E2BIG` where the kernel is older
/// than this `OpenHow`. Both mean "this kernel cannot answer", not "refused":
/// the caller falls back to the `O_NOFOLLOW` walk rather than treating either
/// as a denial.
pub fn openat2(dirfd: i64, path: &CBuf, how: &OpenHow) -> Sysres {
    unsafe {
        sys(
            SYS_OPENAT2,
            [
                dirfd as u64,
                path.ptr(),
                how as *const OpenHow as u64,
                std::mem::size_of::<OpenHow>() as u64,
                0,
                0,
            ],
        )
    }
}

pub fn mkdirat(dirfd: i64, path: &CBuf, mode: u64) -> Sysres {
    unsafe { sys(SYS_MKDIRAT, [dirfd as u64, path.ptr(), mode, 0, 0, 0]) }
}

pub fn unlinkat(dirfd: i64, path: &CBuf, flags: u64) -> Sysres {
    unsafe { sys(SYS_UNLINKAT, [dirfd as u64, path.ptr(), flags, 0, 0, 0]) }
}

/// `mknodat(2)`. ⭐ [`TODO/complete.md`](../../../TODO/complete.md) T-0401
/// CALLS THIS FIRST and shims only where it fails: on a machine that permits
/// it a real `/dev/null` is what the payload should get, and a shim reported as
/// a shim on a machine that did not need one is a degradation podbox invented.
///
/// ⚠ `dev` is the encoded device number: `makedev(major, minor)`.
pub fn mknodat(dirfd: i64, path: &CBuf, mode: u64, dev: u64) -> Sysres {
    unsafe { sys(SYS_MKNODAT, [dirfd as u64, path.ptr(), mode, dev, 0, 0]) }
}

/// The kernel's `makedev`. ⚠ The wide encoding, which is what `mknodat`
/// takes on every architecture podbox builds for.
pub fn makedev(major: u64, minor: u64) -> u64 {
    ((major & 0xfff) << 8)
        | (minor & 0xff)
        | ((minor & !0xffu64) << 12)
        | ((major & !0xfffu64) << 32)
}

pub fn symlinkat(target: &CBuf, dirfd: i64, path: &CBuf) -> Sysres {
    unsafe {
        sys(
            SYS_SYMLINKAT,
            [target.ptr(), dirfd as u64, path.ptr(), 0, 0, 0],
        )
    }
}

pub fn linkat(olddirfd: i64, old: &CBuf, newdirfd: i64, new: &CBuf, flags: u64) -> Sysres {
    unsafe {
        sys(
            SYS_LINKAT,
            [
                olddirfd as u64,
                old.ptr(),
                newdirfd as u64,
                new.ptr(),
                flags,
                0,
            ],
        )
    }
}

pub fn fchmod(fd: i64, mode: u64) -> Sysres {
    unsafe { sys(SYS_FCHMOD, [fd as u64, mode, 0, 0, 0, 0]) }
}

/// ⚠ `AT_SYMLINK_NOFOLLOW` is accepted by the libc wrapper and **rejected by
/// the kernel** with `EINVAL` on Linux: there is no `lchmod`. A caller that
/// wants to avoid following a symlink opens it `O_PATH|O_NOFOLLOW` and uses
/// `fchmod` on the descriptor, or does not chmod a symlink at all, which is
/// what extraction does because a symlink's own mode is not used.
pub fn fchmodat(dirfd: i64, path: &CBuf, mode: u64, flags: u64) -> Sysres {
    unsafe { sys(SYS_FCHMODAT, [dirfd as u64, path.ptr(), mode, flags, 0, 0]) }
}

pub fn fchownat(dirfd: i64, path: &CBuf, uid: u32, gid: u32, flags: u64) -> Sysres {
    unsafe {
        sys(
            SYS_FCHOWNAT,
            [dirfd as u64, path.ptr(), uid as u64, gid as u64, flags, 0],
        )
    }
}

/// `newfstatat(2)` into the same `Stat` [`stat`] fills.
///
/// ⚠ Pass `AT_SYMLINK_NOFOLLOW` to ask about a symlink rather than about what
/// it points at. Extraction always does: the question it asks is "what is
/// already at this name", and a symlink that answers for its target is the
/// whole of `TODO/extract.md` T-0304's defect.
pub fn fstatat(dirfd: i64, path: &CBuf, flags: u64) -> Result<Stat, Errno> {
    let mut raw = unsafe { core::mem::zeroed::<KernelStat>() };
    unsafe {
        sys(
            SYS_FSTATAT,
            [
                dirfd as u64,
                path.ptr(),
                &mut raw as *mut KernelStat as u64,
                flags,
                0,
                0,
            ],
        )?
    };
    Ok(Stat::from_kernel(&raw))
}

/// `DT_*`, the file type `getdents64` reports without a `stat`.
/// ⚠ `DT_UNKNOWN` is legal on any filesystem and is not "not a directory": a
/// caller that needs certainty stats.
pub const DT_UNKNOWN: u8 = 0;
pub const DT_DIR: u8 = 4;
pub const DT_LNK: u8 = 10;

/// One directory entry, as `getdents64(2)` reports it.
pub struct Dirent {
    pub name: Vec<u8>,
    pub d_type: u8,
}

/// Read a whole directory through a descriptor.
///
/// ⛔ Through the descriptor, never by path. The tree being listed is one this
/// process is writing to, and a path is re-resolved by the kernel on every
/// call.
///
/// ⚠ `.` and `..` are dropped here. Every caller wants the contents, and a
/// caller that removed what this returned without dropping them would try to
/// remove the directory it is standing in.
pub fn getdents64(fd: i64) -> Result<Vec<Dirent>, Errno> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 32 * 1024];
    loop {
        let n = unsafe {
            sys(
                SYS_GETDENTS64,
                [
                    fd as u64,
                    buf.as_mut_ptr() as u64,
                    buf.len() as u64,
                    0,
                    0,
                    0,
                ],
            )?
        } as usize;
        if n == 0 {
            break;
        }
        let mut off = 0usize;
        while off + 19 <= n {
            let reclen = u16::from_ne_bytes([buf[off + 16], buf[off + 17]]) as usize;
            if reclen == 0 || off + reclen > n {
                break;
            }
            let d_type = buf[off + 18];
            let name_bytes = &buf[off + 19..off + reclen];
            let end = name_bytes.iter().position(|b| *b == 0).unwrap_or(0);
            let name = name_bytes[..end].to_vec();
            if name != b"." && name != b".." {
                out.push(Dirent { name, d_type });
            }
            off += reclen;
        }
    }
    Ok(out)
}

/// `readlinkat(2)`. Returns the target as bytes, because a link target in an
/// image is not guaranteed to be UTF-8.
pub fn readlinkat(dirfd: i64, path: &CBuf) -> Result<Vec<u8>, Errno> {
    let mut buf = vec![0u8; 4096];
    let n = unsafe {
        sys(
            SYS_READLINKAT,
            [
                dirfd as u64,
                path.ptr(),
                buf.as_mut_ptr() as u64,
                buf.len() as u64,
                0,
                0,
            ],
        )?
    } as usize;
    // ⚠ readlink(2) truncates silently and does not NUL-terminate. A target
    // that exactly filled the buffer may have been longer, so it is refused
    // rather than returned short.
    if n >= buf.len() {
        return Err(ENAMETOOLONG);
    }
    buf.truncate(n);
    Ok(buf)
}

/// Never returns. Used only in a probe child.
pub fn exit_group(code: i32) -> ! {
    loop {
        unsafe {
            syscall6(SYS_EXIT_GROUP, code as u64, 0, 0, 0, 0, 0);
        }
    }
}

pub fn pipe2(fds: &mut [i32; 2], flags: u64) -> Sysres {
    unsafe { sys(SYS_PIPE2, [fds.as_mut_ptr() as u64, flags, 0, 0, 0, 0]) }
}

/// `dup2(2)` by name, `dup3(2)` in fact.
///
/// ⚠ **They differ in one case and it is handled by the caller, not here.**
/// `dup2(fd, fd)` is a no-op returning `fd`; `dup3(fd, fd, 0)` is `EINVAL`.
/// The one caller in this tree, `child.rs`'s verdict channel, already guards
/// `w != 1` before calling, because `dup2(1, 1)` followed by an unconditional
/// `close` was a real defect there. The guard is therefore load-bearing twice,
/// and the `debug_assert` says so where a future caller would otherwise
/// discover it as an `EINVAL` on one architecture only.
pub fn dup2(old: i64, new: i64) -> Sysres {
    debug_assert_ne!(old, new, "dup3 answers EINVAL where dup2 is a no-op");
    unsafe { sys(SYS_DUP3, [old as u64, new as u64, 0, 0, 0, 0]) }
}

/// `clone(flags, stack=0, ...)`, which is a fork when no `CLONE_VM` is set.
///
/// ⛔ Never pass `CLONE_VM`. The child below this call runs on the parent's
/// stack under copy-on-write; sharing the address space instead would have two
/// processes on one stack.
///
/// ⭐ The child sheds every fd registered with [`close_in_children`] before
/// this returns to it. That is here rather than in each caller's `Ok(0)` arm on
/// purpose: an image lock leaking into an unrelated child is
/// [`TODO/image.md`](../../../TODO/image.md) T-0211, and a guard belongs where a
/// caller who knows nothing about it still passes through.
///
/// # Safety
/// The caller must do nothing in the child but async-signal-safe work on
/// buffers that already exist: no allocation, no `std` IO, no locks.
pub unsafe fn clone_fork(flags: u64) -> Sysres {
    let r = unsafe { sys(SYS_CLONE, [flags, 0, 0, 0, 0, 0]) };
    if matches!(r, Ok(0)) {
        shed_registered_fds();
    }
    r
}

/// # Safety
/// `argv` and `envp` must be NUL-terminated arrays of valid C string pointers.
pub unsafe fn execve(path: &CBuf, argv: *const *const u8, envp: *const *const u8) -> Sysres {
    unsafe { sys(SYS_EXECVE, [path.ptr(), argv as u64, envp as u64, 0, 0, 0]) }
}

pub fn wait4(pid: i64, status: &mut i32) -> Sysres {
    unsafe {
        sys(
            SYS_WAIT4,
            [pid as u64, status as *mut i32 as u64, 0, 0, 0, 0],
        )
    }
}

pub fn kill(pid: i64, sig: i64) -> Sysres {
    unsafe { sys(SYS_KILL, [pid as u64, sig as u64, 0, 0, 0, 0]) }
}

/// Start a new session, so a detached supervisor outlives the shell that
/// started it and holds no controlling terminal.
///
/// ⛔ [`TODO/supervise.md`](../../../TODO/supervise.md) T-0603's other half. A
/// launcher still attached to the caller's session takes that terminal's
/// signals, and `podbox run -d` would then die with the shell that started it.
pub fn setsid() -> Sysres {
    unsafe { sys(SYS_SETSID, [0, 0, 0, 0, 0, 0]) }
}

pub fn getpid() -> i64 {
    unsafe { sys(SYS_GETPID, [0, 0, 0, 0, 0, 0]) }.unwrap_or(0)
}

/// A descriptor for a process, which cannot be redirected by pid reuse.
///
/// ⛔ [`TODO/supervise.md`](../../../TODO/supervise.md) T-0601. A pid plus a
/// start time is a heuristic; a pidfd names one process for as long as it is
/// held and answers `ESRCH` for one that has been reaped. ⚠ Say what it is not:
/// it addresses ONE process. It does not contain descendants and it is not a PID
/// namespace, so a grandchild that reparents is outside its reach.
pub fn pidfd_open(pid: i64) -> Sysres {
    unsafe { sys(SYS_PIDFD_OPEN, [pid as u64, 0, 0, 0, 0, 0]) }
}

/// `waitid(P_PIDFD, ...)`: a child's status, addressed by descriptor.
pub const P_PIDFD: u64 = 3;
pub const WEXITED: u64 = 0x0000_0004;
pub const WNOHANG: u64 = 0x0000_0001;

/// What `waitid` fills in, of the kernel's `siginfo_t`, and nothing more.
///
/// ⛔ **The kernel's own buffer, sized generously and read by offset.**
/// `siginfo_t` is 128 bytes on every Linux architecture and its layout after the
/// first three words is a union; the two fields wanted here (`si_code` and
/// `si_status`) sit at fixed offsets in the `SIGCHLD` arm. A hand-written
/// `#[repr(C)]` one field short is a kernel write past the end of it, which is
/// why the buffer is the full 128 bytes whatever is read out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exited {
    /// `CLD_EXITED` (1), `CLD_KILLED` (2), `CLD_DUMPED` (3).
    pub code: i32,
    /// The exit status for `CLD_EXITED`, the signal number otherwise.
    pub status: i32,
}

pub const CLD_EXITED: i32 = 1;

/// Reap a child through its pidfd. `Ok(None)` with `WNOHANG` when it is still
/// running.
pub fn waitid_pidfd(pidfd: i64, nohang: bool) -> Result<Option<Exited>, Errno> {
    // 128 bytes, zeroed, so a short read leaves zeros rather than stack noise.
    let mut buf = [0u8; 128];
    let opts = WEXITED | if nohang { WNOHANG } else { 0 };
    unsafe {
        sys(
            SYS_WAITID,
            [P_PIDFD, pidfd as u64, buf.as_mut_ptr() as u64, opts, 0, 0],
        )
    }?;
    // ⚠ `si_signo` is the first word and is zero when WNOHANG found nothing,
    // which is how "still running" is told from "exited with 0".
    let signo = i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if signo == 0 {
        return Ok(None);
    }
    // si_signo, si_errno, si_code are the first three 32-bit words.
    let code = i32::from_ne_bytes([buf[8], buf[9], buf[10], buf[11]]);
    // ⛔ THE UNION STARTS AT 16, NOT AT 12. `__ARCH_SI_PREAMBLE_SIZE` is
    // `3*sizeof(int) + sizeof(long)`-aligned, so on a 64-bit architecture the
    // three leading ints are followed by four bytes of padding and `_sifields`
    // begins at 16. The SIGCHLD arm is `{ pid_t si_pid; uid_t si_uid; int
    // si_status; ... }`, which puts `si_status` at 24.
    // ⚠ Measured on 2026-09-09 by reading 20 instead: a SIGTERMed payload
    // reported exit 128 rather than 143, because the field read was `si_uid`.
    let status = i32::from_ne_bytes([buf[24], buf[25], buf[26], buf[27]]);
    Ok(Some(Exited { code, status }))
}

pub const POLLIN: i16 = 0x0001;
pub const POLLHUP: i16 = 0x0010;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

/// Wait until one of `fds` is ready, or until `timeout_ms`.
///
/// ⛔ [`TODO/supervise.md`](../../../TODO/supervise.md) T-0602: this is what a
/// supervisor waits on instead of sleeping and looking. Every wait has an upper
/// bound and a distinct outcome for reaching it, per `TODO/RULES.md` section 8:
/// `Ok(0)` is the bound, and it is not an error and not a readiness.
pub fn ppoll(fds: &mut [PollFd], timeout_ms: i64) -> Sysres {
    let ts = Timespec {
        sec: timeout_ms / 1000,
        nsec: (timeout_ms % 1000) * 1_000_000,
    };
    let tsp = if timeout_ms < 0 {
        0
    } else {
        &ts as *const Timespec as u64
    };
    unsafe {
        sys(
            SYS_PPOLL,
            [
                fds.as_mut_ptr() as u64,
                fds.len() as u64,
                tsp,
                0, // sigmask: none
                0,
                0,
            ],
        )
    }
}

/// The four `statfs` fields this project reads, widened to one shape.
///
/// ⛔ **The kernel's own `struct statfs64` is the buffer**, taken per
/// architecture from `linux_raw_sys`, and this is a normalised view built from
/// it. The two are different things on purpose: the kernel's layout and field
/// widths differ by architecture, and a hand-written `#[repr(C)]` that is one
/// field short is a kernel write past the end of it.
#[derive(Default, Clone, Copy)]
pub struct Statfs {
    pub f_bsize: i64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
}

/// `flock(2)`. Used by `TODO/image.md` T-0204 to hold a rootfs against a
/// concurrent GC.
///
/// ⭐ An advisory lock on an **fd**, not a pid file. The mechanism is read out
/// of the corpus at
/// `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:275-278`, where
/// the cache lock guard is held through exec with its fd left inheritable so a
/// concurrent process's GC cannot delete the package while it is still in use.
/// A pid file is stale the moment a process dies unexpectedly, and the check
/// that clears a stale one is the race this closes.
pub fn flock(fd: i64, op: u64) -> Sysres {
    unsafe { sys(SYS_FLOCK, [fd as u64, op, 0, 0, 0, 0]) }
}

/// `readlink(2)` into a caller-sized buffer, returned as a `String`.
///
/// ⚠ The kernel does not NUL-terminate and does not report truncation, so a
/// return equal to the buffer size is reported as an error rather than as a
/// value: a silently truncated namespace identity would compare unequal to
/// itself and invalidate a cache on every run.
pub fn readlink(path: &CBuf) -> Result<String, Errno> {
    let mut buf = [0u8; 256];
    let n = unsafe {
        sys(
            SYS_READLINKAT,
            [
                AT_FDCWD,
                path.ptr(),
                buf.as_mut_ptr() as u64,
                buf.len() as u64,
                0,
                0,
            ],
        )?
    } as usize;
    if n >= buf.len() {
        return Err(ENAMETOOLONG);
    }
    Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
}

pub fn statfs(path: &CBuf) -> Result<Statfs, Errno> {
    // ⚠ `statfs64` and not `statfs`: on a 32-bit architecture the narrow form
    // truncates a filesystem larger than 4 GiB of blocks, and a free-space
    // check that silently wraps is worse than none. The syscall takes the
    // buffer's size so the kernel refuses a shape it does not know.
    let mut raw = unsafe { core::mem::zeroed::<linux_raw_sys::general::statfs64>() };
    unsafe {
        sys(
            SYS_STATFS,
            [
                path.ptr(),
                &mut raw as *mut linux_raw_sys::general::statfs64 as u64,
                0,
                0,
                0,
                0,
            ],
        )?
    };
    // ⚠ As `Stat::from_kernel`: redundant here, load-bearing elsewhere.
    #[allow(clippy::unnecessary_cast)]
    Ok(Statfs {
        f_bsize: raw.f_bsize as i64,
        f_bavail: raw.f_bavail as u64,
        f_files: raw.f_files as u64,
        f_ffree: raw.f_ffree as u64,
    })
}

/// The `stat` fields this project reads, widened to one shape.
///
/// ⛔ As [`Statfs`]: **the kernel's own `struct stat` is the buffer**, taken
/// per architecture from `linux_raw_sys`, and this is a normalised view built
/// from it. It is 144 bytes on x86_64 and 128 on aarch64, and a buffer short by
/// one field is a kernel write past the end of it, which is exactly what a
/// hand-written layout ported to a second architecture does.
#[derive(Default, Clone, Copy)]
pub struct Stat {
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: i64,
}

impl Stat {
    /// ⚠ **Every cast below is redundant on x86_64 and load-bearing somewhere
    /// else.** The kernel's field widths differ by architecture, `st_mode` and
    /// `st_uid` are not the same type on every one of them, so clippy's
    /// `unnecessary_cast` is correct for the architecture it was run on and
    /// wrong for this crate. Removing them breaks the cross-build, which is the
    /// only reason this allow is here.
    #[allow(clippy::unnecessary_cast)]
    fn from_kernel(raw: &KernelStat) -> Stat {
        Stat {
            st_mode: raw.st_mode as u32,
            st_uid: raw.st_uid as u32,
            st_gid: raw.st_gid as u32,
            st_rdev: raw.st_rdev as u64,
            st_size: raw.st_size as i64,
        }
    }
}

impl Stat {
    /// The file type, as the word `ls -l` would put in column one.
    pub fn kind(&self) -> &'static str {
        match self.st_mode & S_IFMT {
            0o140000 => "socket",
            0o120000 => "symlink",
            0o100000 => "file",
            0o060000 => "blockdev",
            0o040000 => "directory",
            0o020000 => "chardev",
            0o010000 => "fifo",
            _ => "unknown",
        }
    }

    pub fn is_chardev(&self) -> bool {
        self.st_mode & S_IFMT == S_IFCHR as u32
    }

    /// The glibc encoding, which is what the kernel writes: the major and minor
    /// are interleaved rather than a simple high and low half.
    pub fn rdev_major(&self) -> u64 {
        ((self.st_rdev >> 8) & 0xfff) | ((self.st_rdev >> 32) & !0xfff)
    }

    pub fn rdev_minor(&self) -> u64 {
        (self.st_rdev & 0xff) | ((self.st_rdev >> 12) & !0xff)
    }
}

/// `stat(2)` by name, `newfstatat(2)` in fact.
///
/// ⚠ Identical by definition: `stat(path)` is `newfstatat(AT_FDCWD, path, 0)`.
/// Routing through the `*at` form is what lets this compile on an architecture
/// whose kernel has no `stat` entry point at all, and it changes nothing on one
/// that does. ⛔ It is a **utility** call and not a probe: nothing in the report
/// says the word `stat`, so the entry point is not the measurement. The three
/// calls where it is are [`CHOWN`], [`LCHOWN`] and [`MKNOD`].
pub fn stat(path: &CBuf) -> Result<Stat, Errno> {
    fstatat(AT_FDCWD as i64, path, 0)
}

pub fn getgroups() -> Result<Vec<i32>, Errno> {
    // NGROUPS_MAX is 65536; ask for the count first so the buffer is the size
    // the kernel says rather than a ceiling this file picked.
    let n = unsafe { sys(SYS_GETGROUPS, [0, 0, 0, 0, 0, 0])? };
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0i32; n as usize];
    let got = unsafe {
        sys(
            SYS_GETGROUPS,
            [n as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0],
        )?
    };
    buf.truncate(got as usize);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ The worst failure available to this module is a wrong number or a
    /// wrong ABI: it would report a verdict for a syscall nobody named, and it
    /// would look exactly like a measurement. Two cross-checks against facts
    /// this process already knows by another route.
    ///
    /// ⚠ Neither proves the whole table. The table itself was checked once
    /// against `<sys/syscall.h>` on 2026-09-08 and the command is recorded in
    /// `TODO/probe.md` T-0101 so it can be re-run; the sixteen attribution
    /// numbers are additionally exercised on every run of
    /// `experiments/130-probe-parity.sh`, which compares them against a reading
    /// the reference instrument took.
    #[test]
    fn the_trap_and_the_abi_agree_with_what_this_process_already_knows() {
        let raw = unsafe { syscall6(SYS_GETPID, 0, 0, 0, 0, 0, 0) };
        assert_eq!(
            raw,
            std::process::id() as i64,
            "SYS_GETPID or the syscall ABI is wrong"
        );
    }

    #[test]
    fn a_negative_return_decodes_as_the_errno_and_not_as_a_value() {
        // close(-1) is EBADF on every Linux. If `split` had the window wrong,
        // this would come back as a huge positive value instead.
        assert_eq!(close(-1), Err(EBADF));
        assert_eq!(split(-1), Err(EPERM));
        assert_eq!(split(-4095), Err(Errno(4095)));
        // ⚠ Outside the window is a VALUE, including a large negative one from
        // a syscall that returns a signed quantity.
        assert_eq!(split(-4096), Ok(-4096));
        assert_eq!(split(0), Ok(0));
    }

    #[test]
    fn a_path_with_an_interior_nul_is_refused_rather_than_truncated() {
        assert!(CBuf::new("/tmp/a\0b").is_none());
        assert!(CBuf::new("/tmp/ab").is_some());
    }

    /// ⛔ A short buffer is a kernel write past the end of it, and the shape is
    /// **not the same on two architectures**: 144 bytes on x86_64 and 128 on
    /// aarch64. Until 2026-09-09 this asserted `144` against a hand-written
    /// struct, which is correct on exactly one architecture and is the reason
    /// the file refused to build anywhere else rather than be wrong.
    ///
    /// ⚠ Both numbers below were **measured, not read from a header**: the
    /// x86_64 one on this host, the aarch64 one by cross-building the same
    /// `size_of` and running it under `qemu-aarch64`. `experiments/260-multiarch.sh`
    /// re-takes both.
    ///
    /// ⭐ An architecture with no number here is not skipped: it asserts the
    /// buffer is at least as large as every field the kernel is told about,
    /// which is the property that actually matters and which podbox now gets by
    /// construction, because the buffer IS the kernel's own generated struct.
    #[test]
    fn the_stat_buffer_is_the_size_the_kernel_writes() {
        let n = core::mem::size_of::<KernelStat>();
        #[cfg(target_arch = "x86_64")]
        assert_eq!(n, 144, "x86_64 struct stat");
        #[cfg(target_arch = "aarch64")]
        assert_eq!(n, 128, "aarch64 struct stat");
        assert!(
            n >= core::mem::size_of::<Stat>(),
            "the kernel's buffer is smaller than the view podbox reads from it"
        );
    }

    #[test]
    fn a_char_device_reads_as_one_and_its_numbers_come_back() {
        // /dev/null is char 1:3 on every Linux, which is also DEV_1_3 above.
        let st = stat(&CBuf::new("/dev/null").unwrap()).expect("/dev/null");
        assert!(st.is_chardev(), "mode {:o}", st.st_mode);
        assert_eq!(st.kind(), "chardev");
        assert_eq!((st.rdev_major(), st.rdev_minor()), (1, 3));
    }

    #[test]
    fn a_directory_is_not_mistaken_for_a_device() {
        let st = stat(&CBuf::new("/tmp").unwrap()).expect("/tmp");
        assert_eq!(st.kind(), "directory");
        assert!(!st.is_chardev());
    }

    #[test]
    fn an_unknown_errno_names_its_number_rather_than_guessing_a_name() {
        assert_eq!(Errno(1).name(), "EPERM");
        assert_eq!(Errno(4093).name(), "E4093");
    }
}
