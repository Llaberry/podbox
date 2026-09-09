//! `TOOL.md` section 6.6: pidfd, waitid, logs, the container lifecycle.
//!
//! Milestone M4, [`TODO/milestones.md`](../../../TODO/milestones.md) T-1105 and
//! [`TODO/supervise.md`](../../../TODO/supervise.md) T-0601 to T-0607.
//!
//! ⭐ **Running state is launcher state.** [`table`] is the record and
//! [`launcher`] is the only thing that writes `running` into it. Nothing infers
//! membership from `/proc`, nothing decides "running" by sleeping and looking,
//! and a launcher that was killed leaves a container marked
//! [`table::State::Dead`] with the time it was NOTICED rather than an exit code
//! nobody measured.
//!
//! ⛔ **What podbox does not have, said once here rather than implied
//! everywhere:** a container is a chroot, not a namespace. There is no PID
//! namespace, so a payload's grandchildren that reparent are outside podbox's
//! reach; `stop` signals the payload and says so.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod launcher;
pub mod table;

use podbox_image::Store;
use table::{Container, State};

/// Everything this crate can fail with, as one sentence for the caller.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// docker's exit code for a runtime-side failure.
pub const EXIT_RUNTIME_ERROR: i32 = 125;

/// A fresh container id: 64 hex characters, as docker's are.
///
/// ⚠ Not a hash of anything. docker's is random, two containers of one image
/// are different containers, and a content-derived id would collide for exactly
/// the case a lifecycle test drives twenty times.
pub fn new_id() -> String {
    let mut bytes = [0u8; 32];
    // ⛔ `read_exact` OF 32 BYTES, never `std::fs::read`. Measured on
    // 2026-09-09, the expensive way: `std::fs::read("/dev/urandom")` reads to
    // EOF and `/dev/urandom` has none, so `podbox create` allocated 13 GB and
    // was OOM-killed with exit 137. A character device is a stream and a whole-
    // file read is a request for all of it.
    match std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes))
    {
        Ok(()) => {}
        Err(_) => {
            // ⚠ A named fallback rather than a silent one: the id still has to
            // be unique, so it is built from the pid and the clock, and it is
            // written here so a reader knows which one they are looking at.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let pid = std::process::id() as u128;
            for (i, b) in bytes.iter_mut().enumerate() {
                *b = ((now >> (i % 16)) ^ (pid << 3) ^ (i as u128)) as u8;
            }
        }
    };
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A container name docker would generate, for a caller that gave none.
pub fn generated_name(id: &str) -> String {
    format!("podbox_{}", &id[..12.min(id.len())])
}

/// Write a `created` record. ⛔ Nothing is started here: `create` and `start`
/// are two verbs because docker's are, and a `create` that started something
/// would make `start` a no-op that reports success.
#[allow(clippy::too_many_arguments)]
pub fn create(
    store: &Store,
    name: Option<&str>,
    image: &str,
    manifest_digest: &str,
    rootfs: &str,
    argv: Vec<String>,
    env: Vec<String>,
    working_dir: String,
    rung: &str,
) -> Result<Container> {
    let id = new_id();
    let name = name
        .map(str::to_string)
        .unwrap_or_else(|| generated_name(&id));
    let c = Container {
        id: id.clone(),
        name: name.clone(),
        image: image.to_string(),
        manifest_digest: manifest_digest.to_string(),
        rootfs: rootfs.to_string(),
        argv,
        env,
        working_dir,
        created_at: podbox_image::clock::now(),
        started_at: None,
        finished_at: None,
        state: State::Created,
        pid: None,
        launcher_pid: None,
        exit_code: None,
        noticed: None,
        rung: rung.to_string(),
    };
    table::update(store, |t| {
        // ⛔ A name identifies one container. docker refuses a duplicate and so
        // does podbox, rather than making the second one unreachable by name.
        if t.containers.iter().any(|x| x.name == name) {
            return Err(Error(format!(
                "the name {name:?} is already used by a container. Remove it, or \
                 give this one another name"
            )));
        }
        t.containers.push(c.clone());
        Ok(())
    })?;
    Ok(c)
}

/// Start a created container, returning once its payload is running.
pub fn start(store: &Store, want: &str, record: &podbox_image::Record) -> Result<Container> {
    let table = table::reconcile(store)?;
    let c = table::find(&table, want)?;
    if c.state == State::Running {
        return Err(Error(format!("{} is already running", c.name)));
    }
    let (launcher, pid) = launcher::start(store, &c, record)?;
    let mut out = c;
    out.state = State::Running;
    out.launcher_pid = Some(launcher);
    out.pid = Some(pid);
    Ok(out)
}

/// Send a signal to a running container's payload.
pub fn kill(store: &Store, want: &str, sig: i32) -> Result<Container> {
    let table = table::reconcile(store)?;
    let c = table::find(&table, want)?;
    if c.state != State::Running {
        return Err(Error(format!(
            "{} is {} and not running, so there is nothing to signal",
            c.name,
            c.state.word()
        )));
    }
    launcher::signal(store, &c.id, sig)?;
    Ok(c)
}

/// SIGTERM, then wait, then SIGKILL. ⛔ Bounded at every step, and each outcome
/// is distinct: `TODO/RULES.md` section 8.
pub fn stop(store: &Store, want: &str, timeout_ms: u64) -> Result<(Container, bool)> {
    let table = table::reconcile(store)?;
    let c = table::find(&table, want)?;
    if c.state != State::Running {
        return Ok((c, false));
    }
    launcher::signal(store, &c.id, 15)?;
    // ⛔ **A LAUNCHER THAT IS ALREADY GONE IS A CONTAINER THAT ALREADY STOPPED**,
    // not a failure. Found by the door sweep of 2026-09-09 and it is the race
    // `experiments/230-lifecycle-loop.sh` kept failing on: `stop` connects
    // TWICE, once to signal and once to wait, and a payload that dies quickly
    // lets the launcher remove its control socket in between. The second
    // connect then answers ENOENT, and reporting that as "the container is not
    // running" turned the fastest possible success into an error. ⚠ Which is
    // why `wait_or_gone` distinguishes "the launcher answered" from "there is
    // no launcher to answer" from "the bound was reached", and only the last
    // means podbox does not know.
    match wait_or_gone(store, &c.id, timeout_ms) {
        Ended::Code(_) | Ended::LauncherGone => return Ok((c, false)),
        Ended::Bound => {}
    }
    // ⚠ The bound was reached, which is a fact rather than a failure, and the
    // caller is told that SIGKILL is what ended it.
    launcher::signal(store, &c.id, 9).ok();
    let _ = wait_or_gone(store, &c.id, timeout_ms);
    Ok((c, true))
}

/// Why a wait on a launcher ended.
///
/// ⛔ Three states, because collapsing the first two is what made a container
/// that stopped instantly read as one that was never running.
enum Ended {
    /// The launcher answered with the payload's exit code.
    Code(i32),
    /// There is no launcher: it has already finished and torn its socket down.
    LauncherGone,
    /// The bound was reached and podbox does not know.
    Bound,
}

fn wait_or_gone(store: &Store, id: &str, timeout_ms: u64) -> Ended {
    match launcher::wait(store, id, timeout_ms) {
        Ok(Some(c)) => Ended::Code(c),
        Ok(None) => Ended::Bound,
        // ⚠ The only failure `launcher::wait` reports is "nothing is listening",
        // which is the launcher having gone rather than an error to pass on.
        Err(_) => Ended::LauncherGone,
    }
}

/// Wait for a container to end. `None` is the bound, never a guess.
pub fn wait(store: &Store, want: &str, timeout_ms: u64) -> Result<(Container, Option<i32>)> {
    let table = table::reconcile(store)?;
    let c = table::find(&table, want)?;
    match c.state {
        State::Running => match wait_or_gone(store, &c.id, timeout_ms) {
            Ended::Code(n) => Ok((c, Some(n))),
            // ⚠ The launcher finished between the reconcile above and this
            // connect. Its last act is to write the exit code into the table,
            // so the answer is there: re-read rather than report the race.
            Ended::LauncherGone => {
                let again = table::reconcile(store)?;
                let c = table::find(&again, want)?;
                let code = c.exit_code;
                Ok((c, code))
            }
            Ended::Bound => Ok((c, None)),
        },
        // ⛔ `Dead` has no exit code and never invents one.
        State::Dead => Ok((c, None)),
        _ => {
            let code = c.exit_code;
            Ok((c, code))
        }
    }
}

/// Remove a container's record and its own directory.
pub fn remove(store: &Store, want: &str, force: bool) -> Result<Container> {
    let table = table::reconcile(store)?;
    let c = table::find(&table, want)?;
    if c.state == State::Running {
        if !force {
            return Err(Error(format!(
                "{} is running. Stop it first, or use -f",
                c.name
            )));
        }
        launcher::signal(store, &c.id, 9).ok();
        let _ = launcher::wait(store, &c.id, 5_000);
    }
    table::update(store, |t| {
        t.containers.retain(|x| x.id != c.id);
        Ok(())
    })?;
    // ⛔ Gated through the same containment check every other unlink in this
    // tree takes, so a crafted id cannot reach outside the store.
    let dir = table::dir(store, &c.id);
    if let Ok(resolved) = podbox_image::contain::within(store.root(), &dir) {
        let _ = std::fs::remove_dir_all(resolved);
    }
    Ok(c)
}

/// Every container, reconciled.
pub fn list(store: &Store, all: bool) -> Result<Vec<Container>> {
    let table = table::reconcile(store)?;
    Ok(table
        .containers
        .into_iter()
        .filter(|c| all || c.state == State::Running)
        .collect())
}

pub fn get(store: &Store, want: &str) -> Result<Container> {
    let table = table::reconcile(store)?;
    table::find(&table, want)
}

/// The container's captured output.
pub fn logs(store: &Store, want: &str) -> Result<Vec<u8>> {
    let c = get(store, want)?;
    let path = table::log_path(store, &c.id);
    match std::fs::read(&path) {
        Ok(v) => Ok(v),
        // ⚠ A container that never wrote anything has no log file, and that is
        // an empty log rather than an error.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error(format!("{}: {e}", path.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_sixty_four_hex_characters_and_two_are_not_equal() {
        let a = new_id();
        let b = new_id();
        assert_eq!(a.len(), 64, "{a}");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "{a}");
        // ⛔ Two containers of one image are two containers. An id derived from
        // content would collide for exactly the case M4's acceptance drives
        // twenty times.
        assert_ne!(a, b);
    }

    #[test]
    fn a_generated_name_is_short_and_derived_from_the_id() {
        let id = new_id();
        let n = generated_name(&id);
        assert!(n.starts_with("podbox_"));
        assert!(id.starts_with(&n["podbox_".len()..]));
    }
}
