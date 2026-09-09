//! `TOOL.md` section 6.5: the chroot entry sequence.
//!
//! [`TODO/milestones.md`](../../../TODO/milestones.md) T-1104 and
//! [`TODO/enter.md`](../../../TODO/enter.md) T-0501 to T-0506.
//!
//! ⭐ **The order of this sequence is the design, and every step of it is an
//! entry.** Doing them in a different order is not a style choice; each ordering
//! constraint below was paid for by a defect somebody shipped.
//!
//! 1. the rootfs is resolved from the store and **refused if it is a symlink**
//!    ([`TODO/enter.md`](../../../TODO/enter.md) T-0504), then held as a
//!    directory descriptor so nothing can swap the path afterwards;
//! 2. the image lock is taken and handed to the payload
//!    ([`TODO/image.md`](../../../TODO/image.md) T-0204, T-0211), so a
//!    concurrent `rmi` cannot delete a rootfs a running container is executing
//!    out of;
//! 3. **every descriptor is opened before the root changes** (T-0501): a chroot
//!    cuts off every path outside the new root, and a child with no explicit
//!    stdio opens `/dev/null`, which an extracted rootfs does not have;
//! 4. the platform is checked against the host's, and a foreign one is either
//!    run through a registered `binfmt_misc` interpreter or **refused by name**
//!    (T-0506), never exec'd into a bare `Exec format error`;
//! 5. `fchdir`, `chroot(".")`, `chdir("/")`, and only **then** is the program
//!    resolved, in the process that changed the root (T-0502).
//!
//! ⛔ **The banner goes to stderr and the payload owns stdout.** T-1104's own
//! decision: a banner on stdout corrupts every pipeline the payload is in.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod abi;
pub mod binfmt;
pub mod plan;

use std::io::Write;

use podbox_probe::sys::{self, CBuf};

pub use plan::{Plan, Program};

/// ⭐ **THE RUNG THIS CRATE IMPLEMENTS**, which is not the rung the probe
/// selects.
///
/// [`crate::run`] performs `TOOL.md` section 6.5's sequence and nothing else: it
/// `chroot`s. It does not `unshare`, it mounts nothing, and it creates no
/// network or pid namespace, on any machine.
///
/// ⛔ **The banner must say THIS and not what the machine would permit**, or it
/// breaks [`TODO/cli.md`](../../../TODO/cli.md) T-0804 rule 4: no output may
/// imply namespaces, cgroups or devices exist when they do not. Measured on
/// 2026-09-09: this host's probe selects `namespace`, and `podbox run` printed
/// `mode=namespace (namespaces: as configured; mounts: full)` while entering a
/// plain chroot. The machine's own selection is still reported, beside this,
/// because "what podbox did" and "what this machine would permit" are two facts
/// and a reader needs both.
///
/// ⚠ When the `namespace` rung is implemented this constant moves with it, and
/// it is one constant so it cannot move in one place and not another.
pub const ENTERED_RUNG: podbox_probe::select::Rung = podbox_probe::select::Rung::Chroot;

// ⛔ **docker's codes, from the one file that holds them.**
// [`TODO/cli.md`](../../../TODO/cli.md) T-0802. This crate declared its own 125,
// 126 and 127 and `podbox-image` declared 125 again; a correction to one of them
// did not reach the others.
pub use podbox_probe::exit::{EXIT_CANNOT_INVOKE, EXIT_NOT_FOUND, EXIT_RUNTIME_ERROR};

#[derive(Debug)]
pub enum Error {
    /// Something podbox could not do. Exits 125.
    Runtime(String),
    /// The payload's command could not be invoked. Exits 126.
    CannotInvoke(String),
    /// The payload's command was not found. Exits 127.
    NotFound(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Runtime(s) | Error::CannotInvoke(s) | Error::NotFound(s) => write!(f, "{s}"),
        }
    }
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Runtime(_) => EXIT_RUNTIME_ERROR,
            Error::CannotInvoke(_) => EXIT_CANNOT_INVOKE,
            Error::NotFound(_) => EXIT_NOT_FOUND,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A directory descriptor on the rootfs, opened before anything changes.
///
/// ⛔ T-0504. `chroot` **follows a symlink**, so a rootfs path that is one puts
/// the payload somewhere podbox did not choose and every containment claim
/// afterwards is about a different directory. The path is `lstat`ed, refused if
/// it is a symlink, and then held open: `fchdir` onto a descriptor cannot be
/// redirected by anything that happens to the path in between.
#[derive(Debug)]
pub struct RootDir {
    fd: i64,
    pub path: String,
}

impl RootDir {
    pub fn open(path: &str) -> Result<RootDir> {
        let c = CBuf::new(path)
            .ok_or_else(|| Error::Runtime(format!("{path:?} contains a NUL byte")))?;

        // ⛔ `AT_SYMLINK_NOFOLLOW`, so this asks about the path itself and not
        // about whatever it points at.
        let st = sys::fstatat(sys::AT_FDCWD as i64, &c, sys::AT_SYMLINK_NOFOLLOW)
            .map_err(|e| Error::Runtime(format!("{path}: {} ({})", e.name(), e.0)))?;
        if st.st_mode & sys::S_IFMT == sys::S_IFLNK {
            let target = sys::readlink(&c).unwrap_or_else(|_| "(unreadable)".into());
            return Err(Error::Runtime(format!(
                "{path} is a symlink to {target}. podbox refuses it rather than \
                 resolving it: chroot follows a symlink, so entering it would \
                 put the payload somewhere podbox did not choose, and the \
                 ownership sidecar and the image lock belong to the store's own \
                 directory (TODO/enter.md T-0504)"
            )));
        }
        if st.st_mode & sys::S_IFMT != sys::S_IFDIR {
            return Err(Error::Runtime(format!("{path} is not a directory")));
        }

        let fd = sys::open(
            &c,
            sys::O_RDONLY | sys::O_DIRECTORY | sys::O_CLOEXEC | sys::O_NOFOLLOW,
            0,
        )
        .map_err(|e| Error::Runtime(format!("opening {path}: {} ({})", e.name(), e.0)))?;
        Ok(RootDir {
            fd,
            path: path.to_string(),
        })
    }

    pub fn fd(&self) -> i64 {
        self.fd
    }
}

impl Drop for RootDir {
    fn drop(&mut self) {
        let _ = sys::close(self.fd);
    }
}

/// What the payload will be given, resolved before the fork.
///
/// ⛔ T-0501. Everything the child touches after the `chroot` exists before it.
/// A descriptor opened before the root changes keeps working after it, and it is
/// the only thing that crosses the boundary on this runtime: there is no attach
/// path and nothing can be bind-mounted.
/// ⚠ The default is EMPTY, and that is the design rather than a stub: stdio is
/// inherited rather than re-opened, so the caller's stdout IS the payload's,
/// which is what makes `podbox run ... | consumer` work at all. What goes in
/// here is what a `--device` or a `-t` pty pair will add, opened before the
/// root changes.
#[derive(Default)]
pub struct Fds {
    /// `(child_fd, host_fd)` pairs, dup'd into place in the child.
    pub pass: Vec<(i64, i64)>,
}

/// Run a payload inside `root`, and return its exit status.
///
/// ⛔ **The exit status is the payload's**, not podbox's. T-0802: an automated
/// caller reads the exit code first, and a runtime that improves a payload's
/// code lies in the field read first.
pub fn run(root: &RootDir, plan: &Plan, err: &mut dyn Write) -> Result<i32> {
    let child = spawn(root, plan, err)?;
    child.wait()
}

/// A payload that has reached its `execve`, and its pid.
///
/// ⛔ [`TODO/supervise.md`](../../../TODO/supervise.md) T-0602: this value only
/// exists once the exec has SUCCEEDED, established by reading the error pipe to
/// EOF rather than by sleeping and looking. A failure before the exec arrives on
/// that pipe as an errno and becomes an [`Error`] here, so a caller never has to
/// decide whether a process it cannot see yet is starting or already dead.
#[derive(Debug)]
pub struct Child {
    pub pid: i64,
}

/// How a bounded wait ended. ⛔ Three outcomes and not two: `TimedOut` is
/// `TODO/RULES.md` section 8's own rule, and a bound reached is neither a
/// failure nor an exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bounded {
    /// docker's exit code for however it ended, as [`Child::wait`] reports it.
    Exited(i32),
    /// The bound was reached and the payload was killed. ⚠ It carries the
    /// bound rather than a code, because there is no code: nothing exited on
    /// its own.
    TimedOut { after_ms: i64 },
}

impl Child {
    /// Reap it, and return docker's exit code for however it ended.
    pub fn wait(&self) -> Result<i32> {
        let mut status = 0i32;
        loop {
            match sys::wait4(self.pid, &mut status) {
                Ok(_) => break,
                // ⚠ `EINTR` is not a failure. A signal delivered to podbox while
                // it waits must not be reported as the payload having died.
                Err(e) if e == sys::EINTR => continue,
                Err(e) => {
                    return Err(Error::Runtime(format!(
                        "wait4 on the payload failed: {} ({})",
                        e.name(),
                        e.0
                    )))
                }
            }
        }
        Ok(exit_status(status))
    }

    /// Reap it, but never wait longer than `ms`.
    ///
    /// ⭐ **`ppoll` on a pidfd, which is `TODO/supervise.md` T-0601 and T-0602's
    /// mechanism rather than a second one.** A pid plus a sleep is a heuristic;
    /// a pidfd names one process and `ppoll` waits on the CONDITION with an
    /// upper bound, so there is no interval to be wrong about and no window in
    /// which a reused pid could be signalled instead.
    ///
    /// ⛔ **On the bound the child is `SIGKILL`ed and then reaped.** A step
    /// podbox started inside somebody else's image and then walked away from
    /// would keep the rootfs busy and hold the image lock, and `docs/AGENTS.md`
    /// makes bounded waits a requirement of podbox itself and not only of the
    /// agent working on it.
    pub fn wait_bounded(&self, ms: i64) -> Result<Bounded> {
        let pidfd = sys::pidfd_open(self.pid).map_err(|e| {
            Error::Runtime(format!(
                "pidfd_open on pid {} failed: {} ({}); podbox will not wait on a \
                 bare pid",
                self.pid,
                e.name(),
                e.0
            ))
        })?;
        let mut fds = [sys::PollFd {
            fd: pidfd as i32,
            events: sys::POLLIN,
            revents: 0,
        }];
        let ready = loop {
            match sys::ppoll(&mut fds, ms) {
                Ok(n) => break n,
                // ⚠ `EINTR` is not the bound. A signal delivered to podbox while
                // it waits must not read as the step having hung.
                Err(e) if e == sys::EINTR => continue,
                Err(e) => {
                    let _ = sys::close(pidfd);
                    return Err(Error::Runtime(format!(
                        "ppoll on the step's pidfd failed: {} ({})",
                        e.name(),
                        e.0
                    )));
                }
            }
        };
        if ready == 0 {
            let _ = sys::kill(self.pid, 9);
        }
        let ended = sys::waitid_pidfd(pidfd, false);
        let _ = sys::close(pidfd);
        if ready == 0 {
            return Ok(Bounded::TimedOut { after_ms: ms });
        }
        match ended {
            Ok(Some(e)) if e.code == sys::CLD_EXITED => Ok(Bounded::Exited(e.status)),
            // ⛔ 128+signal, which is `exit_status`'s own rule and docker's: a
            // killed step reporting 0 or 1 is the lie T-0802 is about.
            Ok(Some(e)) => Ok(Bounded::Exited(128 + e.status)),
            Ok(None) => Err(Error::Runtime(
                "waitid reported no status for a pidfd that was ready".into(),
            )),
            Err(e) => Err(Error::Runtime(format!(
                "waitid on the step failed: {} ({})",
                e.name(),
                e.0
            ))),
        }
    }
}

/// Enter `root` and exec the plan, returning as soon as the exec has succeeded.
///
/// ⛔ **The readiness signal is an `O_CLOEXEC` pipe, and it is the mechanism
/// T-0602 asks for.** The child holds the write end; a successful `execve`
/// closes it and the parent's read returns EOF, and any failure before the exec
/// is written to it as an errno. There is no interval anywhere in it, so there
/// is no scheduling assumption to be wrong about.
pub fn spawn(root: &RootDir, plan: &Plan, err: &mut dyn Write) -> Result<Child> {
    // ---------------------------------------------------------- before the fork
    //
    // ⛔ Every allocation the child needs happens HERE. Between `clone` and
    // `execve` only async-signal-safe work is permitted, so nothing below the
    // fork may allocate, format a string, or take a lock.
    let argv_owned: Vec<CBuf> = plan
        .argv
        .iter()
        .map(|a| {
            CBuf::new(a)
                .ok_or_else(|| Error::Runtime(format!("the argument {a:?} contains a NUL byte")))
        })
        .collect::<Result<_>>()?;
    let envp_owned: Vec<CBuf> = plan
        .env
        .iter()
        .map(|e| {
            CBuf::new(e)
                .ok_or_else(|| Error::Runtime(format!("the environment {e:?} contains a NUL")))
        })
        .collect::<Result<_>>()?;
    let workdir = CBuf::new(if plan.working_dir.is_empty() {
        "/"
    } else {
        &plan.working_dir
    })
    .ok_or_else(|| Error::Runtime("the working directory contains a NUL byte".into()))?;
    let slash = CBuf::new("/").expect("a literal");
    let dot = CBuf::new(".").expect("a literal");

    let mut argv: Vec<*const u8> = argv_owned.iter().map(|c| c.ptr() as *const u8).collect();
    argv.push(std::ptr::null());
    let mut envp: Vec<*const u8> = envp_owned.iter().map(|c| c.ptr() as *const u8).collect();
    envp.push(std::ptr::null());

    // ⭐ T-0502. The program is resolved INSIDE the new root, so the candidate
    // paths are built here and tried there. A parent-resolved absolute path
    // names the outer tree, and the outer tree is gone after the chroot.
    let candidates: Vec<CBuf> = plan
        .program_candidates()
        .into_iter()
        .filter_map(|p| CBuf::new(&p))
        .collect();
    if candidates.is_empty() {
        return Err(Error::NotFound(format!(
            "{:?}: no candidate path could be built for it",
            plan.argv.first().map(String::as_str).unwrap_or("")
        )));
    }

    // ⛔ The banner before the fork, so it cannot interleave with the payload's
    // own first output. T-1104: stderr, never stdout.
    let _ = write!(err, "{}", plan.banner);
    let _ = err.flush();

    // ⭐ T-0602's readiness channel, created before the fork like everything
    // else the child touches. `O_CLOEXEC` on both ends is the whole trick: the
    // `execve` closes the write end, and the parent's read then returns EOF.
    let mut pipe = [0i32; 2];
    sys::pipe2(&mut pipe, sys::O_CLOEXEC).map_err(|e| {
        Error::Runtime(format!(
            "pipe2 for the readiness channel failed: {} ({})",
            e.name(),
            e.0
        ))
    })?;
    let (read_end, write_end) = (pipe[0] as i64, pipe[1] as i64);

    // --------------------------------------------------------------- the fork
    let pid = match unsafe { sys::clone_fork(sys::SIGCHLD) } {
        Ok(0) => {
            // ------ child. Async-signal-safe work only, then execve. ------
            let _ = sys::close(read_end);
            // ⚠ Four bytes to the readiness pipe, which is async-signal-safe and
            // is all the parent needs to tell a failure from a successful exec.
            let report = |step: u8, e: sys::Errno| -> ! {
                let msg = [step, (e.0 & 0xff) as u8, ((e.0 >> 8) & 0xff) as u8, 0];
                let _ = sys::write(write_end, &msg);
                sys::exit_group(EXIT_RUNTIME_ERROR)
            };
            for (child_fd, host_fd) in &plan.fds.pass {
                if host_fd != child_fd {
                    let _ = sys::dup2(*host_fd, *child_fd);
                    let _ = sys::close(*host_fd);
                }
            }
            // ⛔ `fchdir` then `chroot(".")`, never `chroot(path)`: the
            // descriptor was checked and cannot be swapped, and a path can.
            if let Err(e) = sys::fchdir(root.fd) {
                report(1, e);
            }
            if let Err(e) = sys::chroot(&dot) {
                report(2, e);
            }
            // ⛔ `chdir("/")` after the chroot. Without it the working directory
            // is still the old root's inode, which is a documented way out of a
            // chroot and is not a containment podbox may claim.
            if let Err(e) = sys::chdir(&slash) {
                report(3, e);
            }
            // The image's WorkingDir, if it exists. ⚠ A missing one is not
            // fatal: docker creates it, and podbox running from `/` and saying
            // so is better than refusing after the point of no return.
            let _ = sys::chdir(&workdir);

            // ⭐ Resolved HERE, in the process that changed the root.
            let mut last = sys::Errno(2i32);
            for c in &candidates {
                if let Err(e) = unsafe { sys::execve(c, argv.as_ptr(), envp.as_ptr()) } {
                    last = e;
                }
            }
            // Every candidate failed. 127 is the shell's convention and
            // docker's for "not found", and the parent reports it unchanged.
            let msg = [4u8, (last.0 & 0xff) as u8, ((last.0 >> 8) & 0xff) as u8, 0];
            let _ = sys::write(write_end, &msg);
            sys::exit_group(EXIT_NOT_FOUND)
        }
        Ok(pid) => pid,
        Err(e) => {
            let _ = sys::close(read_end);
            let _ = sys::close(write_end);
            return Err(Error::Runtime(format!(
                "clone(2) for the payload failed: {} ({})",
                e.name(),
                e.0
            )));
        }
    };

    // -------------------------------------------------------------- the parent
    // ⛔ The parent closes ITS write end first, or the read below never sees
    // EOF: the pipe stays open on this process's own copy of it forever.
    let _ = sys::close(write_end);
    let mut buf = [0u8; 4];
    let got = loop {
        match sys::read(read_end, &mut buf) {
            Ok(n) => break n,
            Err(e) if e == sys::EINTR => continue,
            Err(_) => break 0,
        }
    };
    let _ = sys::close(read_end);
    if got > 0 {
        // The child failed before its exec and said where. It has already
        // exited, so it is reaped here rather than left as a zombie.
        let mut st = 0i32;
        let _ = sys::wait4(pid, &mut st);
        let errno = sys::Errno(i32::from(buf[1]) | (i32::from(buf[2]) << 8));
        let step = match buf[0] {
            1 => "fchdir onto the rootfs descriptor",
            2 => "chroot(\".\")",
            3 => "chdir(\"/\") after the chroot",
            _ => "execve of every candidate path",
        };
        let text = format!(
            "the payload could not be started: {step} failed with {} ({})",
            errno.name(),
            errno.0
        );
        let named = format!(
            "{:?}: {text}",
            plan.argv.first().map(String::as_str).unwrap_or("")
        );
        return Err(if buf[0] != 4 {
            Error::Runtime(text)
        } else if invocable_but_refused(errno) {
            // ⭐ 126 and not 127, and the difference is docker's. Measured by
            // `experiments/330-exit-codes.sh` on 2026-09-09: `docker run alpine
            // /etc/passwd` exits **126** and podbox exited 127, because every
            // execve failure was folded into "not found". A caller branching on
            // 127 retries with a different path; one branching on 126 does not.
            Error::CannotInvoke(named)
        } else {
            Error::NotFound(named)
        });
    }
    Ok(Child { pid })
}

/// Was the file there and refused, rather than absent?
///
/// ⛔ **The errno decides, not a guess.** `execve` answers `ENOENT` for a path
/// that is not there -- and also for a dynamic loader the binary names and that
/// is not there, which is the trap -- while `EACCES`, `ENOEXEC`, `EISDIR`,
/// `EPERM` and `ETXTBSY` all mean the file was found and could not be run.
/// docker's codes split exactly there: 127 for the first, 126 for the rest.
///
/// ⚠ `ELOOP` and `ENAMETOOLONG` are `NotFound`'s side deliberately: both are
/// answers about resolving the path rather than about the file at the end of it.
fn invocable_but_refused(e: sys::Errno) -> bool {
    matches!(e.0, 1 | 8 | 13 | 21 | 26)
}

/// docker's translation of a wait status into an exit code.
///
/// ⛔ A signalled payload exits `128 + signal`, which is the shell's convention
/// and docker's. Reporting 0 or 1 for a killed process is the lie T-0802 is
/// about: a caller that branches on the code cannot tell a clean exit from a
/// SIGKILL.
pub fn exit_status(status: i32) -> i32 {
    if status & 0x7f == 0 {
        (status >> 8) & 0xff
    } else {
        128 + (status & 0x7f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wait_status_becomes_dockers_exit_code() {
        // exited(0), exited(3), and SIGKILL(9).
        assert_eq!(exit_status(0), 0);
        assert_eq!(exit_status(3 << 8), 3);
        assert_eq!(exit_status(9), 137, "a SIGKILLed payload is 128+9");
        assert_eq!(exit_status(15), 143, "a SIGTERMed payload is 128+15");
    }

    #[test]
    fn a_symlinked_rootfs_is_refused_and_names_its_target() {
        let dir = std::env::temp_dir().join(format!("podbox-enter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real")).unwrap();
        let link = dir.join("link");
        std::os::unix::fs::symlink(dir.join("real"), &link).unwrap();

        let e = RootDir::open(link.to_str().unwrap()).unwrap_err();
        let text = format!("{e}");
        assert!(text.contains("is a symlink"), "{text}");
        assert!(text.contains("T-0504"), "{text}");
        // ⛔ And the real directory is accepted, so the check is about the
        // symlink and not about the path being rejected wholesale.
        assert!(RootDir::open(dir.join("real").to_str().unwrap()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rootfs_that_is_not_a_directory_is_refused() {
        let dir = std::env::temp_dir().join(format!("podbox-enter-f-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("file");
        std::fs::write(&f, b"x").unwrap();
        let e = RootDir::open(f.to_str().unwrap()).unwrap_err();
        assert!(format!("{e}").contains("not a directory"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
