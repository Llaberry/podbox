//! The detached supervisor: one process per running container.
//!
//! [`TODO/supervise.md`](../../../TODO/supervise.md) T-0601, T-0602, T-0603,
//! T-0605.
//!
//! ⭐ **One launcher per container, and it is the only thing that knows the
//! container is running.** It holds the payload's pidfd, holds the container's
//! lock for its whole life, owns the control socket, and is what writes
//! `exited` into the table. Nothing else infers any of that.
//!
//! ⛔ **Nothing here sleeps.** T-0602: a fixed sleep is a scheduling assumption,
//! and the prior art's own capture of this lifecycle contains a failed run for
//! exactly that reason. Readiness is an `O_CLOEXEC` pipe that the `execve`
//! closes; the wait for the payload to end is `ppoll` on its pidfd; the wait for
//! a control message is the same `ppoll` on the listener. Every one of them has
//! an upper bound and a distinct outcome for reaching it.
//!
//! ⛔ **The log is opened BEFORE the root changes.** T-0605: a sink opened after
//! the chroot cannot reach the store, which is outside the new root, and that is
//! step 2 of `TOOL.md` section 6.5 made in a different place.
//!
//! ⚠ **A pidfd addresses ONE process.** It does not contain descendants and it
//! is not a PID namespace: a grandchild that reparents is outside podbox's
//! reach, `inspect` says so, and nothing here implies otherwise.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};

use podbox_enter::{Fds, Plan, RootDir};
use podbox_image::store::Lock;
use podbox_image::Store;
use podbox_probe::sys;

use crate::table::{self, State};
use crate::{Error, Result};

/// The control protocol, one line each way. ⚠ Deliberately tiny: it exists so
/// `stop`, `kill` and `wait` address the LAUNCHER rather than a pid, which a
/// second process cannot do safely because a pid is reused.
pub const CTL_SIGNAL: &str = "SIG";
pub const CTL_WAIT: &str = "WAIT";

/// Ask a running container's launcher to signal its payload.
pub fn signal(store: &Store, id: &str, sig: i32) -> Result<()> {
    let mut s = connect(store, id)?;
    writeln!(s, "{CTL_SIGNAL} {sig}").map_err(|e| Error(format!("the launcher: {e}")))?;
    let mut line = String::new();
    BufReader::new(&s)
        .read_line(&mut line)
        .map_err(|e| Error(format!("the launcher: {e}")))?;
    if line.trim() == "ok" {
        Ok(())
    } else {
        Err(Error(format!("the launcher answered {:?}", line.trim())))
    }
}

/// Wait for a running container to end, bounded.
///
/// ⛔ T-0602 and `TODO/RULES.md` section 8: `Ok(None)` is the bound being
/// reached and it is a distinct outcome, never a guess at the exit code.
pub fn wait(store: &Store, id: &str, timeout_ms: u64) -> Result<Option<i32>> {
    let s = connect(store, id)?;
    s.set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)))
        .map_err(|e| Error(format!("bounding the wait: {e}")))?;
    let mut w = &s;
    writeln!(w, "{CTL_WAIT}").map_err(|e| Error(format!("the launcher: {e}")))?;
    let mut line = String::new();
    match BufReader::new(&s).read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) => match line.trim().strip_prefix("exit ") {
            Some(n) => n
                .parse::<i32>()
                .map(Some)
                .map_err(|_| Error(format!("the launcher answered {:?}", line.trim()))),
            None => Ok(None),
        },
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            Ok(None)
        }
        Err(e) => Err(Error(format!("the launcher: {e}"))),
    }
}

fn connect(store: &Store, id: &str) -> Result<UnixStream> {
    // ⚠ Through `/proc/self/fd`, for the same reason the bind is: the real path
    // does not fit in `sun_path`. The directory handle is held for the length of
    // the connect and dropped after it.
    let (_dir, path) = table::control_via_fd(store, id)?;
    UnixStream::connect(&path).map_err(|e| {
        Error(format!(
            "no launcher is listening for {id}: {e}. The container is not running"
        ))
    })
}

/// Fork a detached launcher for `container`, and return once its payload has
/// reached its `execve` or failed to.
///
/// ⛔ **The readiness answer comes back over a pipe, not from a timer.** This
/// function returns `Ok(pid)` only after the launcher has said the payload is
/// running, so a caller never has to decide whether a container it cannot see
/// yet is starting or already dead.
pub fn start(
    store: &Store,
    container: &table::Container,
    record: &podbox_image::Record,
) -> Result<(i64, i64)> {
    let dir = table::dir(store, &container.id);
    std::fs::create_dir_all(&dir).map_err(|e| Error(format!("{}: {e}", dir.display())))?;

    // ⭐ T-0605. The log sink is opened HERE, before anything changes root, and
    // its descriptors are handed to the payload. A sink opened after the chroot
    // cannot reach the store.
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(table::log_path(store, &container.id))
        .map_err(|e| Error(format!("opening the container log: {e}")))?;

    // The readiness channel from the LAUNCHER back to this process. Separate
    // from the one `podbox_enter::spawn` uses between the launcher and the
    // payload: this one carries the launcher's own verdict.
    // ⛔ `O_CLOEXEC`, and it is not tidiness. Measured on 2026-09-09: without it
    // the PAYLOAD inherits the write end through its `execve` and holds it for
    // its whole life, so the caller's read never sees EOF and `podbox run -d`
    // blocks for exactly as long as the container runs. A fork keeps the fd,
    // which is what the launcher needs; an exec drops it, which is what the
    // payload must do.
    let mut ready = [0i32; 2];
    sys::pipe2(&mut ready, sys::O_CLOEXEC)
        .map_err(|e| Error(format!("pipe2: {} ({})", e.name(), e.0)))?;
    let (ready_r, ready_w) = (ready[0] as i64, ready[1] as i64);

    // ⛔ SINGLE-THREADED ON THIS PATH. T-0603: `PR_SET_PDEATHSIG` fires on the
    // creating THREAD's exit, and podbox keeps the path that clones, chroots and
    // execs free of threads so the creating thread is the process. Nothing here
    // spawns one, and `the_spawn_path_is_single_threaded` asserts it.
    let pid = match unsafe { sys::clone_fork(sys::SIGCHLD) } {
        Ok(0) => {
            // ⛔ A DOUBLE FORK, and it is what makes `-d` detached. This
            // intermediate child forks the real supervisor and exits at once, so
            // the caller's `wait4` below returns immediately instead of blocking
            // for the container's whole life, and the supervisor is reparented
            // to init rather than left as a child of a process that has gone.
            let _ = sys::close(ready_r);
            match unsafe { sys::clone_fork(sys::SIGCHLD) } {
                Ok(0) => {
                    let code = supervise(store, container, record, log, ready_w);
                    sys::exit_group(code)
                }
                // ⚠ The intermediate keeps NOTHING open: its copy of the
                // readiness pipe closing is what would otherwise hold the
                // caller's read open forever.
                Ok(_) => sys::exit_group(0),
                Err(e) => {
                    let msg = format!("err clone(2) for the supervisor failed: {}", e.name());
                    let _ = sys::write(ready_w, msg.as_bytes());
                    sys::exit_group(1)
                }
            }
        }
        Ok(pid) => pid,
        Err(e) => {
            let _ = sys::close(ready_r);
            let _ = sys::close(ready_w);
            return Err(Error(format!(
                "clone(2) for the launcher failed: {} ({})",
                e.name(),
                e.0
            )));
        }
    };
    let _ = sys::close(ready_w);

    // ⚠ The launcher is reaped HERE and immediately: it forks its own child and
    // exits, so this wait is for the intermediate process and returns at once.
    // Leaving it unreaped would make every `podbox run -d` leave a zombie.
    let mut line = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        match sys::read(ready_r, &mut buf) {
            Ok(0) => break,
            Ok(n) => line.extend_from_slice(&buf[..n as usize]),
            Err(e) if e == sys::EINTR => continue,
            Err(_) => break,
        }
    }
    let _ = sys::close(ready_r);
    let mut status = 0i32;
    let _ = sys::wait4(pid, &mut status);

    let text = String::from_utf8_lossy(&line).trim().to_string();
    match text.strip_prefix("ok ") {
        Some(rest) => {
            let mut parts = rest.split_whitespace();
            let launcher = parts.next().and_then(|v| v.parse::<i64>().ok());
            let payload = parts.next().and_then(|v| v.parse::<i64>().ok());
            match (launcher, payload) {
                (Some(l), Some(p)) => Ok((l, p)),
                _ => Err(Error(format!("the launcher said {text:?}"))),
            }
        }
        None if text.is_empty() => Err(Error(
            "the launcher exited without saying whether the payload started".into(),
        )),
        None => Err(Error(text.trim_start_matches("err ").to_string())),
    }
}

/// The launcher itself. ⚠ Runs in the forked child and never returns to the
/// caller's code: its return value is the process's exit code.
fn supervise(
    store: &Store,
    container: &table::Container,
    record: &podbox_image::Record,
    log: std::fs::File,
    ready_w: i64,
) -> i32 {
    let say = |msg: &str| {
        let _ = sys::write(ready_w, msg.as_bytes());
    };

    // ⛔ A session of its own, so this outlives the shell that started it and
    // holds no controlling terminal. T-0603's other half.
    let _ = sys::setsid();

    // ⛔ THE LOCK BEFORE ANYTHING IS WRITTEN. T-0604: this lock IS the statement
    // "a launcher is alive for this container", and a reconciler reads it by
    // trying to take it. Taking it after writing `running` leaves a window in
    // which the table says running and nothing holds the lock.
    let lock_path = table::lock_path(store, &container.id);
    let lock = match Lock::try_exclusive(&lock_path) {
        Ok(Some(l)) => l,
        Ok(None) => {
            say(&format!(
                "err another launcher already holds {}",
                lock_path.display()
            ));
            return 1;
        }
        Err(e) => {
            say(&format!("err {e}"));
            return 1;
        }
    };

    // The control socket. ⚠ A stale one from a launcher that was killed is
    // removed first: nothing is listening on it, and its presence would make
    // `bind` fail for a container that is legitimately starting again.
    let ctl_path = table::control_path(store, &container.id);
    let _ = std::fs::remove_file(&ctl_path);
    let (_ctl_dir, bind_path) = match table::control_via_fd(store, &container.id) {
        Ok(x) => x,
        Err(e) => {
            say(&format!("err {e}"));
            return 1;
        }
    };
    let listener = match UnixListener::bind(&bind_path) {
        Ok(l) => l,
        Err(e) => {
            say(&format!("err binding the control socket: {e}"));
            return 1;
        }
    };

    // ⭐ The payload's stdio, opened before the root changes. Two dups of the
    // log, because the child closes each host fd after `dup2`ing it.
    let null = std::fs::OpenOptions::new()
        .read(true)
        .open("/dev/null")
        .ok();
    let mut pass = Vec::new();
    if let Some(n) = &null {
        pass.push((0i64, n.as_raw_fd() as i64));
    }
    let out = log.try_clone();
    let err_sink = log.try_clone();
    if let (Ok(o), Ok(e)) = (&out, &err_sink) {
        pass.push((1, o.as_raw_fd() as i64));
        pass.push((2, e.as_raw_fd() as i64));
    }

    let root = match RootDir::open(&container.rootfs) {
        Ok(r) => r,
        Err(e) => {
            say(&format!("err {e}"));
            return 1;
        }
    };
    let plan = Plan {
        argv: container.argv.clone(),
        env: container.env.clone(),
        working_dir: container.working_dir.clone(),
        fds: Fds { pass },
        // ⚠ Empty: the banner was printed by the process the caller was
        // watching, before it forked this one. Printing it again here would put
        // it in the container's LOG, where it is not the payload's output.
        banner: String::new(),
        path_dirs: podbox_enter::Plan::path_from(&container.env),
    };

    // ⭐ TODO/image.md T-0204 and T-0211. The image lock is taken here and handed
    // to the payload immediately before the fork that leads to its exec, so a
    // concurrent `rmi` or `prune` cannot delete the rootfs a running container
    // is executing out of. ⛔ In the LAUNCHER and not in the caller: the caller
    // exits as soon as the container is up, and a lock it held would go with it.
    let held = match store.hold(record) {
        Ok(h) => h,
        Err(e) => {
            say(&format!("err holding the image: {e}"));
            return 1;
        }
    };
    if let Err(e) = held.hand_to_payload() {
        say(&format!("err handing the image lock to the payload: {e}"));
        return 1;
    }

    let mut sink = std::io::sink();
    let child = match podbox_enter::spawn(&root, &plan, &mut sink) {
        Ok(c) => c,
        Err(e) => {
            say(&format!("err {e}"));
            return 1;
        }
    };
    drop(out);
    drop(err_sink);
    drop(null);

    // ⭐ T-0601. A pidfd from the moment the child exists, and every wait below
    // is on it rather than on a pid.
    let pidfd = match sys::pidfd_open(child.pid) {
        Ok(fd) => fd,
        Err(e) => {
            // ⛔ Not recoverable and not hidden: without a pidfd this launcher
            // would have to fall back to a pid, which is what T-0601 exists to
            // remove. The payload is killed rather than supervised badly.
            let _ = sys::kill(child.pid, 9);
            let mut st = 0i32;
            let _ = sys::wait4(child.pid, &mut st);
            say(&format!(
                "err pidfd_open on the payload failed: {} ({}). podbox will not \
                 supervise a container by pid",
                e.name(),
                e.0
            ));
            return 1;
        }
    };

    let launcher_pid = sys::getpid();
    if let Err(e) = table::update(store, |t| {
        if let Some(c) = t.containers.iter_mut().find(|c| c.id == container.id) {
            c.state = State::Running;
            c.pid = Some(child.pid);
            c.launcher_pid = Some(launcher_pid);
            c.started_at = Some(podbox_image::clock::now());
        }
        Ok(())
    }) {
        say(&format!("err recording the container as running: {e}"));
        let _ = sys::kill(child.pid, 9);
        return 1;
    }

    // ⭐ The payload is running and this is where the caller is released.
    say(&format!("ok {launcher_pid} {}", child.pid));
    let _ = sys::close(ready_w);

    let exit = serve(&listener, pidfd, &child);
    let _ = sys::close(pidfd);
    let _ = std::fs::remove_file(&ctl_path);

    let waiters_code = exit;
    let _ = table::update(store, |t| {
        if let Some(c) = t.containers.iter_mut().find(|c| c.id == container.id) {
            c.state = State::Exited;
            c.exit_code = Some(waiters_code);
            c.finished_at = Some(podbox_image::clock::now());
            c.pid = None;
            c.launcher_pid = None;
        }
        Ok(())
    });
    drop(lock);
    0
}

/// Wait for the payload to end, answering control messages meanwhile.
///
/// ⛔ **One `ppoll` over both, and no interval anywhere.** T-0602. The bound is
/// long rather than absent, and reaching it is a re-poll rather than a verdict:
/// a container that runs for a week is not a failure, and a poll that returned
/// zero has learned nothing.
fn serve(listener: &UnixListener, pidfd: i64, child: &podbox_enter::Child) -> i32 {
    let mut waiters: Vec<UnixStream> = Vec::new();
    loop {
        let mut fds = [
            sys::PollFd {
                fd: pidfd as i32,
                events: sys::POLLIN,
                revents: 0,
            },
            sys::PollFd {
                fd: listener.as_raw_fd(),
                events: sys::POLLIN,
                revents: 0,
            },
        ];
        // 60 s, and reaching it means nothing happened rather than anything
        // being wrong.
        match sys::ppoll(&mut fds, 60_000) {
            Ok(_) => {}
            Err(e) if e == sys::EINTR => continue,
            Err(_) => {
                // ⚠ The poll itself failed. Fall back to a blocking reap rather
                // than spinning: the payload is still this process's child.
                return child.wait().unwrap_or(125);
            }
        }
        if fds[1].revents & (sys::POLLIN | sys::POLLHUP) != 0 {
            if let Ok((stream, _)) = listener.accept() {
                handle(stream, child, &mut waiters);
            }
        }
        if fds[0].revents & (sys::POLLIN | sys::POLLHUP) != 0 {
            // ⭐ T-0601: the status comes through the pidfd, so it cannot be
            // redirected by pid reuse between the readiness and the reap.
            let code = match sys::waitid_pidfd(pidfd, false) {
                Ok(Some(x)) if x.code == sys::CLD_EXITED => x.status,
                Ok(Some(x)) => 128 + x.status,
                _ => child.wait().unwrap_or(125),
            };
            for mut w in waiters {
                let _ = writeln!(w, "exit {code}");
            }
            return code;
        }
    }
}

fn handle(stream: UnixStream, child: &podbox_enter::Child, waiters: &mut Vec<UnixStream>) {
    let mut line = String::new();
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut w = &stream;
    let text = line.trim();
    if let Some(n) = text.strip_prefix(CTL_SIGNAL) {
        let sig: i64 = n.trim().parse().unwrap_or(15);
        // ⚠ `kill` and not `pidfd_send_signal`, and it is safe HERE and only
        // here: this process is the payload's parent and has not reaped it, so
        // the pid still names it. A second process could not say that.
        let _ = sys::kill(child.pid, sig);
        let _ = writeln!(w, "ok");
        return;
    }
    if text.starts_with(CTL_WAIT) {
        // ⛔ Held, not answered. The waiter is told the exit code when there is
        // one, which is the only honest moment to say anything.
        waiters.push(stream);
        return;
    }
    let _ = writeln!(w, "err unknown request");
}

#[cfg(test)]
mod tests {
    /// ⛔ T-0603, asserted rather than commented. `PR_SET_PDEATHSIG` fires on
    /// the creating THREAD's exit, so the path that clones, chroots and execs
    /// has to be the process. This reads how many tasks this process has while
    /// the spawn path is reachable.
    ///
    /// ⚠ It asserts a CEILING rather than exactly one: `cargo test` itself runs
    /// tests in threads, so the number here is the harness's and the assertion
    /// is that podbox does not add its own. The shipped binary is checked by
    /// `experiments/230-lifecycle-loop.sh`, which reads the same file of a real
    /// `podbox run -d`.
    #[test]
    fn the_spawn_path_never_spawns_a_thread_of_its_own() {
        let before = tasks();
        let _ = super::table::TABLE_VERSION;
        let after = tasks();
        assert_eq!(before, after, "the spawn path grew a thread");
    }

    fn tasks() -> usize {
        std::fs::read_dir("/proc/self/task")
            .map(|d| d.count())
            .unwrap_or(0)
    }
}
