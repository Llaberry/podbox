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

#[cfg(not(target_arch = "x86_64"))]
compile_error!(
    "podbox-probe declares Linux syscall numbers for x86_64 only. Building it \
     for another architecture would use the wrong numbers silently, which is \
     the worst failure available to a probe: it would report a verdict for a \
     syscall nobody named. Add the architecture's table before enabling it."
);

use core::arch::asm;

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

pub type Sysres = Result<i64, Errno>;

// ---------------------------------------------------------------- syscall nrs
pub const SYS_READ: i64 = 0;
pub const SYS_WRITE: i64 = 1;
pub const SYS_OPEN: i64 = 2;
pub const SYS_CLOSE: i64 = 3;
pub const SYS_STAT: i64 = 4;
pub const SYS_DUP2: i64 = 33;
pub const SYS_GETPID: i64 = 39;
pub const SYS_CLONE: i64 = 56;
pub const SYS_EXECVE: i64 = 59;
pub const SYS_WAIT4: i64 = 61;
pub const SYS_KILL: i64 = 62;
pub const SYS_MKDIR: i64 = 83;
pub const SYS_UNLINK: i64 = 87;
pub const SYS_CHOWN: i64 = 92;
pub const SYS_LCHOWN: i64 = 94;
pub const SYS_PTRACE: i64 = 101;
pub const SYS_GETUID: i64 = 102;
pub const SYS_GETGID: i64 = 104;
pub const SYS_SETUID: i64 = 105;
pub const SYS_SETGID: i64 = 106;
pub const SYS_SETSID: i64 = 112;
pub const SYS_GETGROUPS: i64 = 115;
pub const SYS_SETGROUPS: i64 = 116;
pub const SYS_MKNOD: i64 = 133;
pub const SYS_STATFS: i64 = 137;
pub const SYS_PIVOT_ROOT: i64 = 155;
pub const SYS_PRCTL: i64 = 157;
pub const SYS_CHROOT: i64 = 161;
pub const SYS_MOUNT: i64 = 165;
pub const SYS_UMOUNT2: i64 = 166;
pub const SYS_SETHOSTNAME: i64 = 170;
pub const SYS_EXIT_GROUP: i64 = 231;
pub const SYS_UNSHARE: i64 = 272;
pub const SYS_OPENAT: i64 = 257;
pub const SYS_PIPE2: i64 = 293;
pub const SYS_PROCESS_VM_READV: i64 = 310;
pub const SYS_KCMP: i64 = 312;
pub const SYS_SECCOMP: i64 = 317;
pub const SYS_GETRANDOM: i64 = 318;
pub const SYS_MEMFD_CREATE: i64 = 319;
pub const SYS_OPEN_TREE: i64 = 428;
pub const SYS_MOVE_MOUNT: i64 = 429;
pub const SYS_FSOPEN: i64 = 430;
pub const SYS_FSCONFIG: i64 = 431;
pub const SYS_FSMOUNT: i64 = 432;
pub const SYS_PIDFD_GETFD: i64 = 438;
pub const SYS_LANDLOCK_CREATE_RULESET: i64 = 444;

// ---------------------------------------------------------------- constants
pub const CLONE_NEWNS: u64 = 0x0002_0000;
pub const CLONE_NEWUTS: u64 = 0x0400_0000;
pub const CLONE_NEWUSER: u64 = 0x1000_0000;
pub const CLONE_NEWPID: u64 = 0x2000_0000;
pub const SIGCHLD: u64 = 17;

pub const MS_REC: u64 = 0x4000;
pub const MS_SLAVE: u64 = 0x0008_0000;

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
pub const O_DIRECTORY: u64 = 0o200000;
pub const O_CLOEXEC: u64 = 0o2000000;

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
// x86_64 Linux: nr in rax, arguments in rdi rsi rdx r10 r8 r9, return in rax.
// The kernel clobbers rcx and r11. `asm!` adds `lateout` for those.

/// # Safety
/// The caller states that this syscall with these arguments is sound: any
/// pointer argument is valid for the kernel's access, and the effect on this
/// process is one the caller intends.
pub unsafe fn syscall6(nr: i64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            in("r8") a4,
            in("r9") a5,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    ret
}

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

pub fn open(path: &CBuf, flags: u64, mode: u64) -> Sysres {
    unsafe { sys(SYS_OPEN, [path.ptr(), flags, mode, 0, 0, 0]) }
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

pub fn unlink(path: &CBuf) -> Sysres {
    unsafe { sys(SYS_UNLINK, [path.ptr(), 0, 0, 0, 0, 0]) }
}

pub fn mkdir(path: &CBuf, mode: u64) -> Sysres {
    unsafe { sys(SYS_MKDIR, [path.ptr(), mode, 0, 0, 0, 0]) }
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

pub fn dup2(old: i64, new: i64) -> Sysres {
    unsafe { sys(SYS_DUP2, [old as u64, new as u64, 0, 0, 0, 0]) }
}

/// `clone(flags, stack=0, ...)`, which is a fork when no `CLONE_VM` is set.
///
/// ⛔ Never pass `CLONE_VM`. The child below this call runs on the parent's
/// stack under copy-on-write; sharing the address space instead would have two
/// processes on one stack.
///
/// # Safety
/// The caller must do nothing in the child but async-signal-safe work on
/// buffers that already exist: no allocation, no `std` IO, no locks.
pub unsafe fn clone_fork(flags: u64) -> Sysres {
    unsafe { sys(SYS_CLONE, [flags, 0, 0, 0, 0, 0]) }
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

/// `struct statfs` on x86_64: eleven `u64`-shaped fields then `f_spare[4]`.
/// Only the four this project reads are named; the rest is a size.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Statfs {
    pub f_type: i64,
    pub f_bsize: i64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: [i32; 2],
    pub f_namelen: i64,
    pub f_frsize: i64,
    pub f_flags: i64,
    pub f_spare: [i64; 4],
}

pub fn statfs(path: &CBuf) -> Result<Statfs, Errno> {
    let mut out = Statfs::default();
    unsafe {
        sys(
            SYS_STATFS,
            [path.ptr(), &mut out as *mut Statfs as u64, 0, 0, 0, 0],
        )?
    };
    Ok(out)
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

    #[test]
    fn an_unknown_errno_names_its_number_rather_than_guessing_a_name() {
        assert_eq!(Errno(1).name(), "EPERM");
        assert_eq!(Errno(4093).name(), "E4093");
    }
}
