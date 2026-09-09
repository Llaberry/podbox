//! The probe set, ported from `TOOL.md` section 6.1's minimum set and from the
//! Go instrument that implements it,
//! `references/Azathothas__container-research/tree/verification/probe/main.go`
//! and `.../probe/attribute.go`.
//!
//! ⭐ Row names are the reference's, character for character, so a podbox run
//! and a `probe census` / `probe attribute` run diff against each other. The
//! milestone's acceptance ([`TODO/milestones.md`](../../../TODO/milestones.md)
//! T-1101) is exactly that comparison.
//!
//! Four divergences from the Go instrument, each deliberate and each a defect
//! in it rather than a difference of taste:
//!
//! 1. ⛔ **The bare `mount(MS_SLAVE,/)` row is not ported.** In the caller's
//!    mount namespace it changes the propagation of `/` and everything under
//!    it, for the machine, permanently. `TOOL.md` section 6.1's minimum set
//!    names `mount(tmpfs)` in the current namespace and inside
//!    `clone(CLONE_NEWNS)`; it does not name that one. The `MS_SLAVE`
//!    operation is still measured, inside a private mount namespace where it
//!    cannot escape, which is also where bubblewrap performs it
//!    (`references/containers__bubblewrap/tree/bubblewrap.c:3257-3258`).
//! 2. **A probe that mounts, unmounts.** `mount(tmpfs,/mnt)` and
//!    `move_mount(-> /tmp/mm-probe)` succeed on an unconfined host, and the Go
//!    instrument leaves both mounted. A failure to remove one is reported in
//!    the row rather than left silent.
//! 3. **Nothing is overwritten.** Every scratch file is created `O_EXCL`, and
//!    an existing file at that path is a `skip` naming the file, not a probe
//!    that deletes somebody's data to make room for itself.
//! 4. ⭐ **A failed precondition is a `skip`, never the row's denial.** Where
//!    `fsmount` fails, the Go instrument reports its errno as
//!    `move_mount`'s verdict. `TODO/probe.md` T-0109 is that this is a defect:
//!    move_mount was not measured, so no verdict about it was established.

use crate::sys::{self, CBuf, Errno};
use crate::verdict::Outcome;

/// How a probe is run.
#[derive(Clone, Copy)]
pub enum Kind {
    /// The measurement is `clone(2)` with these flags: was the child built?
    /// The child does nothing but exit.
    Clone(u64),
    /// Run the body in a re-executed child, which is additionally placed in
    /// the namespaces named by the flags. A clone that the flags make fail is
    /// a failed precondition, so the row is a `skip` carrying its errno.
    Child {
        ns_flags: u64,
        body: fn() -> Outcome,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// `probe census`: what this machine permits.
    Census,
    /// `probe attribute`: the bogus-argument discriminator and its controls.
    /// `TODO/probe.md` T-0102.
    Attribution,
}

pub struct Probe {
    pub name: &'static str,
    pub group: Group,
    pub kind: Kind,
}

const NS: u64 = sys::CLONE_NEWNS;
const UTS: u64 = sys::CLONE_NEWUTS;

/// ⛔ Fixed order. Two runs of this set are diffable only if the rows come out
/// in the same sequence, which is the same reason the Go instrument carries an
/// `order` array beside its map.
///
/// ⚠ `rustfmt` is turned off for this item and only this item. It is a table,
/// one row per probe, and the default formatting expands each row to four
/// lines: the set stops being readable as a set, which is the one thing this
/// declaration is for. `docs/conventions/code.md` requires an escape hatch to
/// say why, and this is why.
#[rustfmt::skip]
pub static PROBES: &[Probe] = &[
    // ------------------------------------------------------------- census
    Probe { name: "unshare(CLONE_NEWNS)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_unshare_newns } },
    Probe { name: "unshare(CLONE_NEWUSER)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_unshare_newuser } },
    Probe { name: "unshare(CLONE_NEWPID)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_unshare_newpid } },
    Probe { name: "clone(CLONE_NEWNS)", group: Group::Census, kind: Kind::Clone(NS) },
    Probe { name: "clone(CLONE_NEWUTS|NEWNS)", group: Group::Census, kind: Kind::Clone(UTS | NS) },
    Probe { name: "clone(CLONE_NEWUSER)", group: Group::Census,
            kind: Kind::Clone(sys::CLONE_NEWUSER) },
    // ⭐ Named by TOOL.md section 6.1's minimum set and by the banner of
    // section 6.8, and absent from the Go instrument. The UTS namespace is the
    // one the `chroot` rung can still use, so the banner's `namespaces=` field
    // is a measurement rather than an assumption.
    Probe { name: "clone(CLONE_NEWUTS)", group: Group::Census, kind: Kind::Clone(UTS) },
    Probe { name: "sethostname(in NEWUTS)", group: Group::Census,
            kind: Kind::Child { ns_flags: UTS, body: p_sethostname } },
    Probe { name: "mount(tmpfs,/mnt)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_mount_tmpfs } },
    Probe { name: "mount(tmpfs,/mnt) in clone(NEWNS)", group: Group::Census,
            kind: Kind::Child { ns_flags: NS, body: p_mount_tmpfs } },
    Probe { name: "mount(MS_SLAVE,/) in clone(NEWNS)", group: Group::Census,
            kind: Kind::Child { ns_flags: NS, body: p_mount_slave } },
    Probe { name: "pivot_root(/tmp,/tmp)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_pivot_root } },
    Probe { name: "ptrace(PTRACE_TRACEME)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_ptrace } },
    // ⭐ The pair of TODO/probe.md T-0106. Both, in the same directory: a
    // whiteout that succeeds where a real device number fails proves the
    // denial is capability-based, because no path-scoped policy can tell two
    // device numbers apart at one path.
    Probe { name: "mknod(chr 1:3 /tmp/nodprobe)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_mknod_real } },
    Probe { name: "mknod(chr 0:0 = whiteout)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_mknod_whiteout } },
    Probe { name: "setuid(1000)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_setuid_1000 } },
    Probe { name: "setuid(0)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_setuid_0 } },
    Probe { name: "setgid(0)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_setgid_0 } },
    Probe { name: "setgroups(0,NULL)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_setgroups } },
    Probe { name: "chown(f,0,0)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_chown_0_0 } },
    Probe { name: "chown(f,0,42)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_chown_0_42 } },
    Probe { name: "lchown(f,0,42)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_lchown_0_42 } },
    Probe { name: "chown(f,1000,0)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_chown_1000_0 } },
    Probe { name: "chroot(/tmp)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_chroot } },
    Probe { name: "memfd_create", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_memfd } },
    Probe { name: "memfd_create+exec", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_memfd_exec } },
    Probe { name: "setsid", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_setsid } },
    Probe { name: "prctl(PR_SET_PDEATHSIG)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_pdeathsig } },
    Probe { name: "exec(/tmp/execprobe)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_exec_file } },
    Probe { name: "getrandom", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_getrandom } },
    Probe { name: "write(/etc/probe)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_write_etc } },
    Probe { name: "write into uid-1000-owned dir", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_write_squash } },
    // ⭐ TODO/enter.md T-0503 puts these two in THIS set, in the OUTER
    // environment, before any chroot. `-t` either works or it does not, and
    // the corpus disagrees with itself about which: the target's mount table
    // shows six device nodes and no `ptmx`, and an earlier account asserts the
    // host's works and published no capture. A mount table does not list plain
    // files, so neither settles it and one stat does. The open is the second
    // row because existence is not function: rule 1 of TOOL.md section 6.1 is
    // to probe the operation you need.
    Probe { name: "stat(/dev/ptmx)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_stat_ptmx } },
    Probe { name: "open(/dev/ptmx, O_RDWR)", group: Group::Census,
            kind: Kind::Child { ns_flags: 0, body: p_open_ptmx } },

    // -------------------------------------------------------- attribution
    // TODO/probe.md T-0102. A seccomp filter sees the syscall number and six
    // argument registers, cannot dereference a pointer, and runs before the
    // syscall body. An argument the kernel rejects INSIDE the body therefore
    // separates a filter from a policy that runs later.
    Probe { name: "mount(2) bogus target", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_mount_bogus } },
    Probe { name: "umount2(2) bogus target", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_umount_bogus } },
    Probe { name: "pivot_root(2) bogus paths", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_pivot_bogus } },
    Probe { name: "process_vm_readv(bogus pid)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_process_vm_readv } },
    // ⛔ The controls travel with the discriminator, in the same run. A control
    // that ran at a different time answers about a different machine state.
    Probe { name: "pidfd_getfd(-1,-1) [control]", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_pidfd_getfd } },
    Probe { name: "kcmp(-1,-1,...) [control]", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_kcmp } },
    Probe { name: "fsopen(tmpfs)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_fsopen } },
    Probe { name: "fsmount(tmpfs)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_fsmount } },
    Probe { name: "open_tree(/tmp, CLONE)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_open_tree } },
    Probe { name: "move_mount(-> bogus dest)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_move_mount_bogus } },
    Probe { name: "move_mount(-> /tmp/mm-probe)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_move_mount_real } },
    Probe { name: "openat(detached tmpfs, O_DIRECTORY)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_openat_detached } },
    Probe { name: "seccomp(NEW_LISTENER)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_seccomp_listener } },
    Probe { name: "open /proc/self/mem O_RDONLY", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_mem_rdonly } },
    Probe { name: "open /proc/self/mem O_RDWR", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_mem_rdwr } },
    Probe { name: "landlock_create_ruleset(VERSION)", group: Group::Attribution,
            kind: Kind::Child { ns_flags: 0, body: a_landlock } },
];

pub fn find(name: &str) -> Option<&'static Probe> {
    PROBES.iter().find(|p| p.name == name)
}

// ---------------------------------------------------------------- helpers

/// A path that cannot exist. A syscall that resolves paths and is not filtered
/// must answer `ENOENT` for it.
const BOGUS: &str = "/proc/self/nonexistent-probe-path";

fn c(s: &str) -> CBuf {
    // Every literal below is NUL-free, so this cannot fail. Constructing it
    // through the checked path anyway keeps one constructor.
    CBuf::new(s).expect("probe path literals carry no interior NUL")
}

fn exists(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Create a scratch file the probe owns.
///
/// ⛔ `O_EXCL`. A probe that deletes whatever is in its way has destroyed data
/// to measure a permission, and the errno it then reports is about a file it
/// created rather than the one that was there.
fn make_scratch(path: &str) -> Result<(), Outcome> {
    match create_exclusive(path, 0o600) {
        Ok(fd) => {
            let _ = sys::close(fd);
            Ok(())
        }
        Err(sys::EEXIST) => Err(already_there(path)),
        Err(e) => Err(Outcome::skip(
            Some(e),
            format!("could not create the scratch file {path}"),
        )),
    }
}

fn create_exclusive(path: &str, mode: u64) -> Result<i64, Errno> {
    sys::open(&c(path), sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL, mode)
}

fn already_there(path: &str) -> Outcome {
    Outcome::skip(
        Some(sys::EEXIST),
        format!("{path} already exists and this probe does not overwrite; remove it and re-run"),
    )
}

fn unscratch(path: &str) {
    let _ = sys::unlink(&c(path));
}

/// `/bin/sh` is the interpreter both exec probes need. Its absence is a
/// missing precondition, and the `ENOENT` it would otherwise produce reads
/// exactly like a denial. TODO/probe.md T-0109 rule 2.
fn need_sh() -> Result<(), Outcome> {
    if exists("/bin/sh") {
        Ok(())
    } else {
        Err(Outcome::skip(
            Some(sys::ENOENT),
            "/bin/sh is absent, so there is no interpreter for the payload this \
             probe executes; the exec was not attempted",
        ))
    }
}

/// Run a path as a child process and report what happened to it.
fn run_payload(path: &str) -> Outcome {
    match std::process::Command::new(path).status() {
        Ok(st) if st.success() => Outcome::ok(),
        Ok(st) => Outcome::skip(
            None,
            format!("the payload {path} ran and ended {st}; the exec itself succeeded"),
        ),
        Err(e) => match e.raw_os_error() {
            Some(n) => Outcome::denied(Errno(n)),
            None => Outcome::skip(None, format!("spawning {path} failed: {e}")),
        },
    }
}

/// Which of the three calls behind a detached mount answered.
///
/// ⭐ Named rather than compared as a string: `TODO/probe.md` T-0103 asks for
/// creation, configuration and attachment to be separate verdicts, and a
/// caller cannot tell them apart from one errno.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Fsopen,
    Fsconfig,
    Fsmount,
}

impl Step {
    fn label(self) -> &'static str {
        match self {
            Step::Fsopen => "fsopen(tmpfs)",
            Step::Fsconfig => "fsconfig(FSCONFIG_CMD_CREATE)",
            Step::Fsmount => "fsmount(tmpfs)",
        }
    }
}

/// A detached tmpfs mount fd, or the step that failed and its errno.
fn detached_tmpfs() -> Result<i64, (Errno, Step)> {
    let fs = c("tmpfs");
    let fd = unsafe { sys::sys(sys::SYS_FSOPEN, [fs.ptr(), 0, 0, 0, 0, 0]) }
        .map_err(|e| (e, Step::Fsopen))?;
    let r = unsafe {
        sys::sys(
            sys::SYS_FSCONFIG,
            [fd as u64, sys::FSCONFIG_CMD_CREATE, 0, 0, 0, 0],
        )
    };
    if let Err(e) = r {
        let _ = sys::close(fd);
        return Err((e, Step::Fsconfig));
    }
    let mfd = unsafe { sys::sys(sys::SYS_FSMOUNT, [fd as u64, 0, 0, 0, 0, 0]) };
    let _ = sys::close(fd);
    mfd.map_err(|e| (e, Step::Fsmount))
}

fn precondition(step: &str, e: Errno) -> Outcome {
    Outcome::skip(
        Some(e),
        format!(
            "{step} failed with {} ({}), so this operation was never attempted",
            e.name(),
            e.0
        ),
    )
}

// ---------------------------------------------------------------- census

fn p_unshare_newns() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_UNSHARE, [sys::CLONE_NEWNS, 0, 0, 0, 0, 0]) })
}

fn p_unshare_newuser() -> Outcome {
    // ⛔ TODO/probe.md T-0101: the kernel refuses `unshare(CLONE_NEWUSER)` to a
    // multithreaded caller with EINVAL regardless of policy, so an EINVAL from
    // a threaded prober is the prober's own answer and not the machine's.
    // podbox is single-threaded by construction (`TOOL.md` section 4.2) and
    // this child was freshly execed, so the invariant holds. It is checked
    // rather than assumed, because the day somebody adds a thread pool the
    // check is what says so.
    match threads_of_self() {
        Some(n) if n > 1 => {
            return Outcome::skip(
                None,
                format!(
                    "this prober has {n} threads, and the kernel refuses \
                         unshare(CLONE_NEWUSER) to any multithreaded caller with \
                         EINVAL whatever the policy; the machine was not asked"
                ),
            )
        }
        _ => {}
    }
    Outcome::from(unsafe { sys::sys(sys::SYS_UNSHARE, [sys::CLONE_NEWUSER, 0, 0, 0, 0, 0]) })
}

fn p_unshare_newpid() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_UNSHARE, [sys::CLONE_NEWPID, 0, 0, 0, 0, 0]) })
}

fn threads_of_self() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("Threads:"))
        .and_then(|v| v.trim().parse().ok())
}

fn p_sethostname() -> Outcome {
    // Runs inside clone(CLONE_NEWUTS), so a success renames that namespace's
    // host and nothing else. Without the namespace the row is a `skip`, which
    // the child harness produces from the clone's own errno.
    let name = c("podbox-probe");
    Outcome::from(unsafe { sys::sys(sys::SYS_SETHOSTNAME, [name.ptr(), 12, 0, 0, 0, 0]) })
}

fn p_mount_tmpfs() -> Outcome {
    // ⛔ The target is NOT checked before the call, and that is the point. A
    // seccomp filter runs before the syscall body, so a filtered mount(2)
    // answers EPERM whether or not /mnt exists; a `stat` first would turn that
    // measurement into a skip and this machine's policy would go unmeasured.
    // The precondition is separated afterwards instead, from the errno: only
    // ENOENT is a statement about the target rather than about the mount.
    // TOOL.md section 2.1 is the same argument, used the other way round.
    let (src, tgt, fs, data) = (c("none"), c("/mnt"), c("tmpfs"), sys::cempty());
    let r = unsafe {
        sys::sys(
            sys::SYS_MOUNT,
            [src.ptr(), tgt.ptr(), fs.ptr(), 0, data.ptr(), 0],
        )
    };
    match r {
        Err(sys::ENOENT) => Outcome::skip(
            Some(sys::ENOENT),
            "mount(2) executed and answered ENOENT, which is about the target \
             /mnt and not about the mount policy: /mnt does not exist here. The \
             policy itself is in the `mount(2) bogus target` row.",
        ),
        Err(e) => Outcome::denied(e),
        Ok(_) => {
            // A probe that mounts, unmounts. Where it cannot, the row says so.
            match unsafe { sys::sys(sys::SYS_UMOUNT2, [tgt.ptr(), 0, 0, 0, 0, 0]) } {
                Ok(_) => Outcome::ok(),
                Err(e) => Outcome::ok_with(format!(
                    "⚠ the tmpfs mounted at /mnt could not be removed: umount2 \
                     answered {} ({}). It is still mounted.",
                    e.name(),
                    e.0
                )),
            }
        }
    }
}

fn p_mount_slave() -> Outcome {
    // Only ever reached inside clone(CLONE_NEWNS): see the module header. This
    // is bubblewrap's `MS_SLAVE` remount, the one whose failure it reports as
    // "Failed to make / slave"
    // (`references/containers__bubblewrap/tree/bubblewrap.c:3257-3258`).
    let (src, tgt, fs, data) = (sys::cempty(), c("/"), sys::cempty(), sys::cempty());
    Outcome::from(unsafe {
        sys::sys(
            sys::SYS_MOUNT,
            [
                src.ptr(),
                tgt.ptr(),
                fs.ptr(),
                sys::MS_SLAVE | sys::MS_REC,
                data.ptr(),
                0,
            ],
        )
    })
}

fn p_pivot_root() -> Outcome {
    let p = c("/tmp");
    Outcome::from(unsafe { sys::sys(sys::SYS_PIVOT_ROOT, [p.ptr(), p.ptr(), 0, 0, 0, 0]) })
}

fn p_ptrace() -> Outcome {
    // PTRACE_TRACEME is request 0.
    Outcome::from(unsafe { sys::sys(sys::SYS_PTRACE, [0, 0, 0, 0, 0, 0]) })
}

fn p_mknod_real() -> Outcome {
    mknod_at("/tmp/nodprobe", sys::DEV_1_3)
}

fn p_mknod_whiteout() -> Outcome {
    // makedev(0,0) is WHITEOUT_DEV and `vfs_mknod` exempts it from the
    // capability check, which is why it tests nothing on its own and why
    // TODO/probe.md T-0106 requires the pair.
    mknod_at("/tmp/wprobe", 0)
}

fn mknod_at(path: &str, dev: u64) -> Outcome {
    if exists(path) {
        return already_there(path);
    }
    let p = c(path);
    // ⛔ `sys::MKNOD` and not a bare number: on an architecture whose kernel has
    // no `mknod(2)` this is `mknodat(2)`, with the directory descriptor first,
    // and the row says which was issued. `TODO/probe.md` T-0101 measures a
    // NAMED syscall, so calling one and printing another would be the exact
    // dishonesty this probe set exists to avoid.
    let e = sys::MKNOD;
    let args = if e.via_at {
        [sys::AT_FDCWD, p.ptr(), sys::S_IFCHR | 0o600, dev, 0, 0]
    } else {
        [p.ptr(), sys::S_IFCHR | 0o600, dev, 0, 0, 0]
    };
    let r = unsafe { sys::sys(e.nr, args) };
    if r.is_ok() {
        unscratch(path);
    }
    Outcome::from(r)
}

fn p_setuid_1000() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_SETUID, [1000, 0, 0, 0, 0, 0]) })
}

fn p_setuid_0() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_SETUID, [0, 0, 0, 0, 0, 0]) })
}

fn p_setgid_0() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_SETGID, [0, 0, 0, 0, 0, 0]) })
}

fn p_setgroups() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_SETGROUPS, [0, 0, 0, 0, 0, 0]) })
}

const CHOWN_TARGET: &str = "/tmp/chown-probe-target";

/// ⛔ The entry point is part of the measurement, so it is passed in rather
/// than assumed. Where the kernel has no `chown(2)`, every architecture taking
/// its table from `asm-generic/unistd.h`, [`sys::CHOWN`] is `fchownat(2)` and
/// the row prints that name. The **verdict** is about the operation (may this
/// process give a file an id it does not map?), and the entry point is the
/// evidence under it.
fn chown_probe(e: sys::Entry, uid: u64, gid: u64) -> Outcome {
    if let Err(o) = make_scratch(CHOWN_TARGET) {
        return o;
    }
    let p = c(CHOWN_TARGET);
    let at_flags = if e.name.contains("AT_SYMLINK_NOFOLLOW") {
        sys::AT_SYMLINK_NOFOLLOW
    } else {
        0
    };
    let args = if e.via_at {
        [sys::AT_FDCWD, p.ptr(), uid, gid, at_flags, 0]
    } else {
        [p.ptr(), uid, gid, 0, 0, 0]
    };
    let r = unsafe { sys::sys(e.nr, args) };
    unscratch(CHOWN_TARGET);
    Outcome::from(r)
}

fn p_chown_0_0() -> Outcome {
    chown_probe(sys::CHOWN, 0, 0)
}

fn p_chown_0_42() -> Outcome {
    chown_probe(sys::CHOWN, 0, 42)
}

fn p_lchown_0_42() -> Outcome {
    chown_probe(sys::LCHOWN, 0, 42)
}

fn p_chown_1000_0() -> Outcome {
    chown_probe(sys::CHOWN, 1000, 0)
}

fn p_chroot() -> Outcome {
    let p = c("/tmp");
    Outcome::from(unsafe { sys::sys(sys::SYS_CHROOT, [p.ptr(), 0, 0, 0, 0, 0]) })
}

fn memfd(name: &str) -> Result<i64, Errno> {
    // ⛔ No MFD_CLOEXEC: the descriptor has to survive a fork and exec for
    // /proc/self/fd/N to name it in the payload.
    let n = c(name);
    unsafe { sys::sys(sys::SYS_MEMFD_CREATE, [n.ptr(), 0, 0, 0, 0, 0]) }
}

fn p_memfd() -> Outcome {
    match memfd("probe") {
        Ok(fd) => {
            let _ = sys::close(fd);
            Outcome::ok()
        }
        Err(e) => Outcome::denied(e),
    }
}

fn p_memfd_exec() -> Outcome {
    if let Err(o) = need_sh() {
        return o;
    }
    let fd = match memfd("probe") {
        Ok(fd) => fd,
        Err(e) => return precondition("memfd_create", e),
    };
    let script = b"#!/bin/sh\nexit 0\n";
    if let Err(e) = sys::write(fd, script) {
        let _ = sys::close(fd);
        return precondition("write to the memfd", e);
    }
    let out = run_payload(&format!("/proc/self/fd/{fd}"));
    let _ = sys::close(fd);
    out
}

fn p_setsid() -> Outcome {
    Outcome::from(unsafe { sys::sys(sys::SYS_SETSID, [0, 0, 0, 0, 0, 0]) })
}

fn p_pdeathsig() -> Outcome {
    Outcome::from(unsafe {
        sys::sys(
            sys::SYS_PRCTL,
            [sys::PR_SET_PDEATHSIG, 15 /* SIGTERM */, 0, 0, 0, 0],
        )
    })
}

const EXEC_TARGET: &str = "/tmp/execprobe";

fn p_exec_file() -> Outcome {
    if let Err(o) = need_sh() {
        return o;
    }
    if exists(EXEC_TARGET) {
        return already_there(EXEC_TARGET);
    }
    let fd = match create_exclusive(EXEC_TARGET, 0o755) {
        Ok(fd) => fd,
        Err(sys::EEXIST) => return already_there(EXEC_TARGET),
        Err(e) => return precondition(&format!("creating {EXEC_TARGET}"), e),
    };
    let r = sys::write(fd, b"#!/bin/sh\nexit 0\n");
    let _ = sys::close(fd);
    if let Err(e) = r {
        unscratch(EXEC_TARGET);
        return precondition(&format!("writing {EXEC_TARGET}"), e);
    }
    let out = run_payload(EXEC_TARGET);
    unscratch(EXEC_TARGET);
    out
}

fn p_getrandom() -> Outcome {
    let mut buf = [0u8; 16];
    Outcome::from(unsafe {
        sys::sys(
            sys::SYS_GETRANDOM,
            [buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0, 0],
        )
    })
}

/// ⭐ The one probe whose failure to create a file IS the measurement, which
/// is why it does not go through `make_scratch`. An `EEXIST` is still a skip:
/// the file that was in the way was somebody else's, and nothing about the
/// write policy was learned.
fn p_write_etc() -> Outcome {
    const P: &str = "/etc/probe";
    match create_exclusive(P, 0o600) {
        Ok(fd) => {
            let _ = sys::close(fd);
            unscratch(P);
            Outcome::ok()
        }
        Err(sys::EEXIST) => already_there(P),
        Err(e) => Outcome::denied(e),
    }
}

/// The root-squash observation: a directory owned by an id that is not mapped
/// in this user namespace is unwritable even with `CAP_DAC_OVERRIDE`.
///
/// The fixture cannot be built from inside the confinement, because `chown` to
/// an unmapped id is exactly what is denied. `experiments/20-enter-target.sh`
/// stages it; the Go instrument looks for its own copy under `/tmp`.
fn p_write_squash() -> Outcome {
    const CANDIDATES: [&str; 2] = ["/workspace/.fixtures/squash-probe", "/tmp/squash-probe"];
    let Some(dir) = CANDIDATES.iter().find(|d| exists(d)) else {
        return Outcome::skip(
            Some(sys::ENOENT),
            format!(
                "no fixture directory owned by an unmapped id: neither {} nor {} \
                 exists. Create one as another uid outside the confinement.",
                CANDIDATES[0], CANDIDATES[1]
            ),
        );
    };
    let path = format!("{dir}/podbox-write-probe");
    match create_exclusive(&path, 0o600) {
        Ok(fd) => {
            let _ = sys::close(fd);
            unscratch(&path);
            Outcome::ok_with(format!("wrote into {dir}"))
        }
        Err(sys::EEXIST) => already_there(&path),
        Err(e) => Outcome::denied(e),
    }
}

pub const PTMX: &str = "/dev/ptmx";

fn p_stat_ptmx() -> Outcome {
    match sys::stat(&c(PTMX)) {
        Ok(st) => Outcome::ok_with(format!(
            "{} {}:{} mode {:o}",
            st.kind(),
            st.rdev_major(),
            st.rdev_minor(),
            st.st_mode & 0o7777
        )),
        // ⛔ ENOENT here is the answer, not a missing precondition: the
        // question this row exists for is whether the file is there.
        Err(e) => Outcome::denied(e),
    }
}

fn p_open_ptmx() -> Outcome {
    // ⚠ O_NOCTTY. Opening a terminal without it can make it the prober's
    // controlling terminal, which mutates the process doing the measuring.
    // The child is disposable either way; the flag is what makes the row a
    // measurement rather than a side effect.
    match sys::open(&c(PTMX), sys::O_RDWR | sys::O_NOCTTY, 0) {
        Ok(fd) => {
            // A successful open of a working ptmx allocates a pty pair. Closing
            // it releases the slave, so the probe leaves no pty behind.
            let _ = sys::close(fd);
            Outcome::ok()
        }
        Err(e) => Outcome::denied(e),
    }
}

// ------------------------------------------------------------ attribution

fn a_mount_bogus() -> Outcome {
    let (src, tgt, fs) = (c("none"), c(BOGUS), c("tmpfs"));
    Outcome::from(unsafe { sys::sys(sys::SYS_MOUNT, [src.ptr(), tgt.ptr(), fs.ptr(), 0, 0, 0]) })
}

fn a_umount_bogus() -> Outcome {
    let tgt = c(BOGUS);
    Outcome::from(unsafe { sys::sys(sys::SYS_UMOUNT2, [tgt.ptr(), 0, 0, 0, 0, 0]) })
}

fn a_pivot_bogus() -> Outcome {
    let p = c(BOGUS);
    Outcome::from(unsafe { sys::sys(sys::SYS_PIVOT_ROOT, [p.ptr(), p.ptr(), 0, 0, 0, 0]) })
}

fn a_process_vm_readv() -> Outcome {
    let mut buf = [0u8; 1];
    let local = [buf.as_mut_ptr() as u64, 1u64];
    let remote = [0x1000u64, 1u64];
    Outcome::from(unsafe {
        sys::sys(
            sys::SYS_PROCESS_VM_READV,
            [
                999_999,
                local.as_ptr() as u64,
                1,
                remote.as_ptr() as u64,
                1,
                0,
            ],
        )
    })
}

fn a_pidfd_getfd() -> Outcome {
    let m1 = -1i64 as u64;
    Outcome::from(unsafe { sys::sys(sys::SYS_PIDFD_GETFD, [m1, m1, 0, 0, 0, 0]) })
}

fn a_kcmp() -> Outcome {
    let m1 = -1i64 as u64;
    let r = unsafe { sys::sys(sys::SYS_KCMP, [m1, m1, 0, 0, 0, 0]) };
    match r {
        // ⭐ TODO/probe.md T-0102's correction. `kcmp(2)` exists only where the
        // kernel was built with CONFIG_CHECKPOINT_RESTORE. ENOSYS is that
        // kernel saying the control is unavailable, which is not the same
        // statement as "the control answered". A control that cannot answer is
        // reported as unable to, so the discriminator it belongs to says it has
        // stopped discriminating rather than reporting a mechanism.
        Err(sys::ENOSYS) => Outcome::skip(
            Some(sys::ENOSYS),
            "kcmp(2) is not present on this kernel (CONFIG_CHECKPOINT_RESTORE is \
             unset), so this control cannot answer here",
        ),
        other => Outcome::from(other),
    }
}

fn a_fsopen() -> Outcome {
    let fs = c("tmpfs");
    match unsafe { sys::sys(sys::SYS_FSOPEN, [fs.ptr(), 0, 0, 0, 0, 0]) } {
        Ok(fd) => {
            let _ = sys::close(fd);
            Outcome::ok()
        }
        Err(e) => Outcome::denied(e),
    }
}

fn a_fsmount() -> Outcome {
    match detached_tmpfs() {
        Ok(mfd) => {
            let _ = sys::close(mfd);
            // ⭐ TODO/probe.md T-0103: fsmount runs the same may_mount() check
            // move_mount and unshare(CLONE_NEWNS) use, so its success proves no
            // mount denial on this machine is a capability problem. Saying so
            // in the row stops the next reader hunting for a capability.
            Outcome::ok_with("may_mount() passes, so no mount denial here is a capability problem")
        }
        Err((e, Step::Fsmount)) => Outcome::denied(e),
        Err((e, step)) => precondition(step.label(), e),
    }
}

fn a_open_tree() -> Outcome {
    let p = c("/tmp");
    match unsafe {
        sys::sys(
            sys::SYS_OPEN_TREE,
            [sys::AT_FDCWD, p.ptr(), sys::OPEN_TREE_CLONE, 0, 0, 0],
        )
    } {
        Ok(fd) => {
            let _ = sys::close(fd);
            Outcome::ok()
        }
        Err(e) => Outcome::denied(e),
    }
}

fn a_move_mount_bogus() -> Outcome {
    let mfd = match detached_tmpfs() {
        Ok(fd) => fd,
        Err((e, step)) => return precondition(step.label(), e),
    };
    let (empty, dest) = (sys::cempty(), c(BOGUS));
    let r = unsafe {
        sys::sys(
            sys::SYS_MOVE_MOUNT,
            [
                mfd as u64,
                empty.ptr(),
                sys::AT_FDCWD,
                dest.ptr(),
                sys::MOVE_MOUNT_F_EMPTY_PATH,
                0,
            ],
        )
    };
    let _ = sys::close(mfd);
    Outcome::from(r)
}

fn a_move_mount_real() -> Outcome {
    const DEST: &str = "/tmp/mm-probe";
    let mfd = match detached_tmpfs() {
        Ok(fd) => fd,
        Err((e, step)) => return precondition(step.label(), e),
    };
    let dest = c(DEST);
    if let Err(e) = sys::mkdir(&dest, 0o755) {
        if e != sys::EEXIST {
            let _ = sys::close(mfd);
            return precondition(&format!("mkdir {DEST}"), e);
        }
    }
    let empty = sys::cempty();
    let r = unsafe {
        sys::sys(
            sys::SYS_MOVE_MOUNT,
            [
                mfd as u64,
                empty.ptr(),
                sys::AT_FDCWD,
                dest.ptr(),
                sys::MOVE_MOUNT_F_EMPTY_PATH,
                0,
            ],
        )
    };
    let _ = sys::close(mfd);
    match r {
        Err(e) => Outcome::denied(e),
        Ok(_) => match unsafe { sys::sys(sys::SYS_UMOUNT2, [dest.ptr(), 0, 0, 0, 0, 0]) } {
            Ok(_) => Outcome::ok(),
            Err(e) => Outcome::ok_with(format!(
                "⚠ the tmpfs attached at {DEST} could not be removed: umount2 \
                 answered {} ({}). It is still mounted.",
                e.name(),
                e.0
            )),
        },
    }
}

fn a_openat_detached() -> Outcome {
    let mfd = match detached_tmpfs() {
        Ok(fd) => fd,
        Err((e, step)) => return precondition(step.label(), e),
    };
    let dot = c(".");
    let r = unsafe {
        sys::sys(
            sys::SYS_OPENAT,
            [
                mfd as u64,
                dot.ptr(),
                sys::O_RDONLY | sys::O_DIRECTORY,
                0,
                0,
                0,
            ],
        )
    };
    if let Ok(fd) = r {
        let _ = sys::close(fd);
    }
    let _ = sys::close(mfd);
    Outcome::from(r)
}

/// `struct sock_filter` and `struct sock_fprog`, in the kernel's layout. The
/// six padding bytes are the alignment of the pointer that follows the `u16`,
/// and writing them out is what stops the pointer landing four bytes early.
#[repr(C)]
struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct SockFprog {
    len: u16,
    pad: [u8; 6],
    filter: *const SockFilter,
}

fn a_seccomp_listener() -> Outcome {
    // BPF_RET|BPF_K with SECCOMP_RET_ALLOW: a filter that permits everything
    // and exists only to carry the notification listener.
    let filter = [SockFilter {
        code: 0x06,
        jt: 0,
        jf: 0,
        k: 0x7fff_0000,
    }];
    let prog = SockFprog {
        len: 1,
        pad: [0; 6],
        filter: filter.as_ptr(),
    };
    if let Err(e) = unsafe { sys::sys(sys::SYS_PRCTL, [sys::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0, 0]) } {
        return precondition("prctl(PR_SET_NO_NEW_PRIVS)", e);
    }
    let r = unsafe {
        sys::sys(
            sys::SYS_SECCOMP,
            [
                sys::SECCOMP_SET_MODE_FILTER,
                sys::SECCOMP_FILTER_FLAG_NEW_LISTENER,
                &prog as *const SockFprog as u64,
                0,
                0,
                0,
            ],
        )
    };
    match r {
        Ok(fd) => {
            let _ = sys::close(fd);
            Outcome::ok()
        }
        Err(e) => Outcome::denied(e),
    }
}

fn open_probe(path: &str, flags: u64) -> Outcome {
    let p = c(path);
    match sys::open(&p, flags, 0) {
        Ok(fd) => {
            let _ = sys::close(fd);
            Outcome::ok()
        }
        Err(e) => Outcome::denied(e),
    }
}

fn a_mem_rdonly() -> Outcome {
    open_probe("/proc/self/mem", sys::O_RDONLY)
}

fn a_mem_rdwr() -> Outcome {
    open_probe("/proc/self/mem", sys::O_RDWR)
}

fn a_landlock() -> Outcome {
    // (NULL, 0, LANDLOCK_CREATE_RULESET_VERSION) returns the ABI rather than a
    // ruleset fd. This row reports whether an LSM of the class that explains
    // mechanism M is present at all, which is not a verdict about podbox.
    match unsafe { sys::sys(sys::SYS_LANDLOCK_CREATE_RULESET, [0, 0, 1, 0, 0, 0]) } {
        Ok(abi) => Outcome::ok_with(format!("ABI {abi}")),
        Err(e) => Outcome::denied(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_name_is_unique() {
        let mut names: Vec<&str> = PROBES.iter().map(|p| p.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "two probes share a row name");
    }

    #[test]
    fn the_set_is_at_least_the_size_its_acceptance_names() {
        // TODO/probe.md T-0101's Prove asserts `.probes | length >= 24`.
        assert!(PROBES.len() >= 24, "{} probes", PROBES.len());
    }

    #[test]
    fn the_attribution_block_is_the_reference_order() {
        // TODO/milestones.md T-1101 compares this block against
        // experiments/results/attribute.txt row for row, so its names and its
        // order are the reference's `attrOrder`.
        let got: Vec<&str> = PROBES
            .iter()
            .filter(|p| p.group == Group::Attribution)
            .map(|p| p.name)
            .collect();
        assert_eq!(
            got,
            vec![
                "mount(2) bogus target",
                "umount2(2) bogus target",
                "pivot_root(2) bogus paths",
                "process_vm_readv(bogus pid)",
                "pidfd_getfd(-1,-1) [control]",
                "kcmp(-1,-1,...) [control]",
                "fsopen(tmpfs)",
                "fsmount(tmpfs)",
                "open_tree(/tmp, CLONE)",
                "move_mount(-> bogus dest)",
                "move_mount(-> /tmp/mm-probe)",
                "openat(detached tmpfs, O_DIRECTORY)",
                "seccomp(NEW_LISTENER)",
                "open /proc/self/mem O_RDONLY",
                "open /proc/self/mem O_RDWR",
                "landlock_create_ruleset(VERSION)",
            ]
        );
    }

    #[test]
    fn the_propagation_change_is_only_ever_probed_inside_a_mount_namespace() {
        // The one probe with an unbounded effect on the caller's machine.
        for p in PROBES {
            if p.name.contains("MS_SLAVE") {
                match p.kind {
                    Kind::Child { ns_flags, .. } => {
                        assert_eq!(ns_flags & sys::CLONE_NEWNS, sys::CLONE_NEWNS, "{}", p.name)
                    }
                    Kind::Clone(_) => panic!("{} must run a body", p.name),
                }
            }
        }
    }
}
