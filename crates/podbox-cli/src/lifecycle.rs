//! The lifecycle verbs: milestone M4.
//!
//! [`TODO/milestones.md`](../../../TODO/milestones.md) T-1105,
//! [`TODO/supervise.md`](../../../TODO/supervise.md) T-0601 to T-0607.
//!
//! ⭐ **`create`, `start`, `ps`, `logs`, `stop`, `kill`, `wait`, `rm`, `cp`,
//! and `exec` and `inspect` against a container.** Every one of them reads the
//! container table rather than the filesystem, and every wait in them is on a
//! condition with an upper bound.
//!
//! ⛔ **Nothing here sleeps and nothing polls.** T-0602: the prior art's own
//! capture of this lifecycle contains a failed run because it decided "running"
//! by sleeping three seconds and looking, and M4 is accepted on twenty
//! consecutive passes precisely because one pass proves nothing about a race.

use std::io::Write;

use podbox_image::error::{EXIT_RUNTIME_ERROR, EXIT_USAGE};
use podbox_image::platform::Platform;
use podbox_image::transport::Policy;
use podbox_supervise::table::{Container, State};

use crate::format;

/// docker's default grace period between `SIGTERM` and `SIGKILL`.
const STOP_GRACE_MS: u64 = 10_000;
/// ⛔ `podbox wait`'s bound. `TODO/RULES.md` section 8: a runtime whose audience
/// is automated may not wait unbounded, so this is long rather than absent and
/// reaching it is its own reported outcome.
const WAIT_BOUND_MS: u64 = 3_600_000;

pub const PS_USAGE: &str = "\
usage: podbox ps [-a|--all] [-q|--quiet] [--format T] [--no-trunc]

  Fields: .ID .Names .Image .Command .CreatedAt .Status .State .Ports .Pid

  ⛔ .Ports is always empty and is not an oversight: the payload shares this
    machine's network namespace, so there is nothing to publish.
";

pub const CONTAINER_INSPECT_FIELDS: &[&str] = &[
    "Id",
    "Name",
    "Image",
    "State",
    "Status",
    "Pid",
    "LauncherPid",
    "ExitCode",
    "Created",
    "StartedAt",
    "FinishedAt",
    "RootfsPath",
    "LogPath",
    "Rung",
    "Command",
    "Exec.Mode",
    "Exec.Shares",
    "Contains",
    "Noticed",
];

fn store() -> Result<podbox_image::Store, i32> {
    podbox_image::open_store().map_err(|e| {
        eprintln!("podbox: {e}");
        e.exit_code()
    })
}

fn fail(verb: &str, e: podbox_supervise::Error) -> i32 {
    eprintln!("podbox {verb}: {e}");
    EXIT_RUNTIME_ERROR
}

/// `podbox create [options] <image> [command...]`, and the shared half of
/// `podbox run -d`.
pub fn create(args: &[String]) -> i32 {
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    match crate::run::prepare(args, &s, "create") {
        Ok(p) => {
            match podbox_supervise::create(
                &s,
                p.name.as_deref(),
                &p.image,
                &p.record.manifest_digest,
                &p.rootfs,
                p.argv,
                p.env,
                p.working_dir,
                &p.rung,
            ) {
                Ok(c) => {
                    println!("{}", c.id);
                    0
                }
                Err(e) => fail("create", e),
            }
        }
        Err(code) => code,
    }
}

/// `podbox start <container>...`
pub fn start(args: &[String]) -> i32 {
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    let mut any = false;
    for want in args {
        if want == "-h" || want == "--help" {
            println!("usage: podbox start <container> [container...]");
            return 0;
        }
        if want.starts_with('-') {
            eprintln!("podbox start: unknown option {want:?}");
            return EXIT_USAGE;
        }
        any = true;
        let c = match podbox_supervise::get(&s, want) {
            Ok(c) => c,
            Err(e) => {
                code = fail("start", e);
                continue;
            }
        };
        // ⚠ The image record is needed so the LAUNCHER can hold the image lock
        // for the container's whole life. A lock this process took would go
        // with this process, which exits as soon as the container is up.
        let record = match s.find_one(&c.image) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("podbox start: {e}");
                code = EXIT_RUNTIME_ERROR;
                continue;
            }
        };
        match podbox_supervise::start(&s, want, &record) {
            Ok(c) => println!("{}", c.name),
            Err(e) => code = fail("start", e),
        }
    }
    if !any {
        println!("usage: podbox start <container> [container...]");
        return EXIT_USAGE;
    }
    code
}

/// `podbox ps`
pub fn ps(args: &[String]) -> i32 {
    let mut all = false;
    let mut quiet = false;
    let mut no_trunc = false;
    let mut template: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{PS_USAGE}");
                return 0;
            }
            "-a" | "--all" => all = true,
            "-q" | "--quiet" => quiet = true,
            "--no-trunc" => no_trunc = true,
            "--format" => match it.next() {
                Some(t) => template = Some(t.clone()),
                None => {
                    eprintln!("podbox ps: --format needs a template");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with("--format=") => {
                template = Some(other["--format=".len()..].to_string())
            }
            other => {
                eprintln!("podbox ps: unknown option {other:?}");
                eprint!("{PS_USAGE}");
                return EXIT_USAGE;
            }
        }
    }
    let fields = ps_field_names();
    // ⛔ Checked before anything is read, exactly as `images --format` is: a
    // template validated only inside the loop over results is never checked at
    // all when there are none, and a caller's typo then reads as "no containers".
    if let Some(t) = &template {
        if let Err(bad) = format::check(t, &fields) {
            eprintln!("podbox ps: {bad}");
            return EXIT_USAGE;
        }
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let list = match podbox_supervise::list(&s, all) {
        Ok(l) => l,
        Err(e) => return fail("ps", e),
    };
    if quiet {
        for c in &list {
            println!("{}", if no_trunc { c.id.clone() } else { c.short_id() });
        }
        return 0;
    }
    if let Some(t) = &template {
        for c in &list {
            match format::render(t, &ps_fields(c, no_trunc)) {
                Ok(line) => println!("{line}"),
                Err(bad) => {
                    eprintln!("podbox ps: {bad}");
                    return EXIT_USAGE;
                }
            }
        }
        return 0;
    }
    let mut rows = vec![vec![
        "CONTAINER ID".to_string(),
        "IMAGE".into(),
        "COMMAND".into(),
        "CREATED".into(),
        "STATUS".into(),
        "NAMES".into(),
    ]];
    for c in &list {
        rows.push(vec![
            if no_trunc { c.id.clone() } else { c.short_id() },
            c.image.clone(),
            command_of(c),
            c.created_at.clone(),
            c.status(),
            c.name.clone(),
        ]);
    }
    print!("{}", crate::images::table(&rows));
    0
}

fn command_of(c: &Container) -> String {
    let joined = c.argv.join(" ");
    if joined.len() > 20 {
        format!("{}...", &joined[..17])
    } else {
        joined
    }
}

fn ps_field_names() -> Vec<&'static str> {
    vec![
        "ID",
        "Names",
        "Image",
        "Command",
        "CreatedAt",
        "Status",
        "State",
        "Ports",
        "Pid",
    ]
}

fn ps_fields(c: &Container, no_trunc: bool) -> Vec<(&'static str, String)> {
    vec![
        ("ID", if no_trunc { c.id.clone() } else { c.short_id() }),
        ("Names", c.name.clone()),
        ("Image", c.image.clone()),
        ("Command", c.argv.join(" ")),
        ("CreatedAt", c.created_at.clone()),
        ("Status", c.status()),
        ("State", c.state.word().to_string()),
        // ⛔ Always empty, and the usage says why rather than leaving a reader
        // to wonder whether podbox forgot.
        ("Ports", String::new()),
        (
            "Pid",
            c.pid.map(|p| p.to_string()).unwrap_or_else(|| "0".into()),
        ),
    ]
}

/// `podbox logs <container>`
pub fn logs(args: &[String]) -> i32 {
    let Some(want) = args.first() else {
        println!("usage: podbox logs <container>");
        return EXIT_USAGE;
    };
    if want == "-h" || want == "--help" {
        println!("usage: podbox logs <container>");
        return 0;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    match podbox_supervise::logs(&s, want) {
        Ok(bytes) => {
            // ⚠ The payload's own bytes, to stdout, unaltered. `logs` is the one
            // verb whose stdout is not podbox's.
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(&bytes);
            0
        }
        Err(e) => fail("logs", e),
    }
}

/// `podbox stop [-t N] <container>...`
pub fn stop(args: &[String]) -> i32 {
    let mut grace = STOP_GRACE_MS;
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("usage: podbox stop [-t seconds] <container> [container...]");
                return 0;
            }
            "-t" | "--time" | "--timeout" => match it.next().and_then(|v| v.parse::<u64>().ok()) {
                Some(v) => grace = v * 1000,
                None => {
                    eprintln!("podbox stop: -t takes a number of seconds");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with('-') => {
                eprintln!("podbox stop: unknown option {other:?}");
                return EXIT_USAGE;
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        println!("usage: podbox stop [-t seconds] <container> [container...]");
        return EXIT_USAGE;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        match podbox_supervise::stop(&s, want, grace) {
            Ok((c, killed)) => {
                if killed {
                    // ⛔ Said, never silent. A caller that asked for a graceful
                    // stop and got a SIGKILL has to be able to tell.
                    eprintln!(
                        "podbox stop: {} did not exit within {} s of SIGTERM and was killed",
                        c.name,
                        grace / 1000
                    );
                }
                println!("{}", c.name);
            }
            Err(e) => code = fail("stop", e),
        }
    }
    code
}

/// `podbox kill [-s SIG] <container>...`
pub fn kill(args: &[String]) -> i32 {
    let mut sig = 9;
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("usage: podbox kill [-s SIGNAL] <container> [container...]");
                return 0;
            }
            "-s" | "--signal" => match it.next() {
                Some(v) => match signal_number(v) {
                    Some(n) => sig = n,
                    None => {
                        eprintln!("podbox kill: {v:?} is not a signal podbox knows");
                        return EXIT_USAGE;
                    }
                },
                None => {
                    eprintln!("podbox kill: -s needs a signal");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with('-') => {
                eprintln!("podbox kill: unknown option {other:?}");
                return EXIT_USAGE;
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        println!("usage: podbox kill [-s SIGNAL] <container> [container...]");
        return EXIT_USAGE;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        match podbox_supervise::kill(&s, want, sig) {
            Ok(c) => println!("{}", c.name),
            Err(e) => code = fail("kill", e),
        }
    }
    code
}

/// ⚠ By name or by number, and an unknown one is refused rather than defaulted:
/// sending the wrong signal is not something a caller can notice.
fn signal_number(v: &str) -> Option<i32> {
    if let Ok(n) = v.parse::<i32>() {
        return (n > 0 && n < 65).then_some(n);
    }
    let name = v.trim_start_matches("SIG").to_ascii_uppercase();
    Some(match name.as_str() {
        "HUP" => 1,
        "INT" => 2,
        "QUIT" => 3,
        "KILL" => 9,
        "USR1" => 10,
        "USR2" => 12,
        "TERM" => 15,
        "CONT" => 18,
        "STOP" => 19,
        _ => return None,
    })
}

/// `podbox wait <container>...`
pub fn wait(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("usage: podbox wait <container> [container...]");
        return if args.is_empty() { EXIT_USAGE } else { 0 };
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in args {
        match podbox_supervise::wait(&s, want, WAIT_BOUND_MS) {
            Ok((_, Some(n))) => println!("{n}"),
            Ok((c, None)) => {
                // ⛔ THE BOUND, OR AN UNWATCHED EXIT, AND NEVER A NUMBER. A `0`
                // printed here for a container podbox did not see end is the
                // lie this whole crate is arranged to refuse.
                eprintln!(
                    "podbox wait: {} has no exit code podbox measured: it is {} \
                     (TODO/supervise.md T-0604)",
                    c.name,
                    c.status()
                );
                code = EXIT_RUNTIME_ERROR;
            }
            Err(e) => code = fail("wait", e),
        }
    }
    code
}

/// `podbox rm [-f] <container>...`
pub fn rm(args: &[String]) -> i32 {
    let mut force = false;
    let mut names = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("usage: podbox rm [-f|--force] <container> [container...]");
                return 0;
            }
            "-f" | "--force" => force = true,
            "-v" | "--volumes" => {}
            other if other.starts_with('-') => {
                eprintln!("podbox rm: unknown option {other:?}");
                return EXIT_USAGE;
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        println!("usage: podbox rm [-f|--force] <container> [container...]");
        return EXIT_USAGE;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        match podbox_supervise::remove(&s, want, force) {
            Ok(c) => println!("{}", c.name),
            Err(e) => code = fail("rm", e),
        }
    }
    code
}

/// `podbox cp <src> <dest>`, where one side is `<container>:<path>`.
pub fn cp(args: &[String]) -> i32 {
    if args.len() == 1 && (args[0] == "-h" || args[0] == "--help") {
        println!(
            "usage: podbox cp <container>:<path> <dest>\n       \
             podbox cp <src> <container>:<path>"
        );
        return 0;
    }
    if args.len() != 2 {
        eprintln!("podbox cp: takes exactly two paths, one of them <container>:<path>");
        return EXIT_USAGE;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let (a, b) = (&args[0], &args[1]);
    let (from_container, to_container) = (split(a), split(b));
    match (from_container, to_container) {
        (Some(_), Some(_)) => {
            eprintln!("podbox cp: copying between two containers is not implemented");
            EXIT_RUNTIME_ERROR
        }
        (None, None) => {
            eprintln!("podbox cp: one of the two paths has to be <container>:<path>");
            EXIT_USAGE
        }
        (Some((name, inside)), None) => copy(&s, &name, &inside, std::path::Path::new(b), true),
        (None, Some((name, inside))) => copy(&s, &name, &inside, std::path::Path::new(a), false),
    }
}

/// `name:/path` where the name is not a Windows drive letter or a bare path.
fn split(arg: &str) -> Option<(String, String)> {
    let (name, path) = arg.split_once(':')?;
    if name.is_empty() || path.is_empty() || name.contains('/') {
        return None;
    }
    Some((name.to_string(), path.to_string()))
}

fn copy(
    s: &podbox_image::Store,
    name: &str,
    inside: &str,
    outside: &std::path::Path,
    out_of: bool,
) -> i32 {
    let c = match podbox_supervise::get(s, name) {
        Ok(c) => c,
        Err(e) => return fail("cp", e),
    };
    let root = std::path::Path::new(&c.rootfs);
    let joined = root.join(inside.trim_start_matches('/'));
    // ⛔ THE SAME CONTAINMENT GATE every other path in this tree takes. A
    // container path of `../../etc/shadow` is a request to write outside the
    // rootfs, and `cp` is the verb most likely to be handed one.
    let target = match podbox_image::contain::within(root, &joined) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("podbox cp: {e}");
            return EXIT_RUNTIME_ERROR;
        }
    };
    let (src, dst) = if out_of {
        (target, outside.to_path_buf())
    } else {
        (outside.to_path_buf(), target)
    };
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::copy(&src, &dst) {
        Ok(n) => {
            eprintln!(
                "podbox cp: {} bytes {} -> {}",
                n,
                src.display(),
                dst.display()
            );
            0
        }
        Err(e) => {
            eprintln!("podbox cp: {}: {e}", src.display());
            EXIT_RUNTIME_ERROR
        }
    }
}

/// `podbox inspect` against a container rather than an image.
///
/// ⚠ Tried after the image lookup fails, so a name that is both is an image, as
/// docker resolves it.
pub fn inspect_container(want: &str, template: Option<&str>) -> Option<i32> {
    let s = podbox_image::open_store().ok()?;
    let c = podbox_supervise::get(&s, want).ok()?;
    let fields = container_fields(&s, &c);
    match template {
        Some(t) => match format::render(t, &fields) {
            Ok(line) => {
                println!("{line}");
                Some(0)
            }
            Err(bad) => {
                eprintln!("podbox inspect: {bad}");
                Some(EXIT_USAGE)
            }
        },
        None => {
            println!("{}", container_json(&s, &c));
            Some(0)
        }
    }
}

fn container_fields(s: &podbox_image::Store, c: &Container) -> Vec<(&'static str, String)> {
    vec![
        ("Id", c.id.clone()),
        ("Name", c.name.clone()),
        ("Image", c.image.clone()),
        ("State", c.state.word().to_string()),
        ("Status", c.status()),
        (
            "Pid",
            c.pid.map(|p| p.to_string()).unwrap_or_else(|| "0".into()),
        ),
        (
            "LauncherPid",
            c.launcher_pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "0".into()),
        ),
        // ⛔ A dash where there is no code, never a zero. docs/AGENTS.md
        // absolute 3, and `wait` refuses for the same reason.
        (
            "ExitCode",
            c.exit_code
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".into()),
        ),
        ("Created", c.created_at.clone()),
        (
            "StartedAt",
            c.started_at.clone().unwrap_or_else(|| "-".into()),
        ),
        (
            "FinishedAt",
            c.finished_at.clone().unwrap_or_else(|| "-".into()),
        ),
        ("RootfsPath", c.rootfs.clone()),
        (
            "LogPath",
            podbox_supervise::table::log_path(s, &c.id)
                .display()
                .to_string(),
        ),
        ("Rung", c.rung.clone()),
        ("Command", c.argv.join(" ")),
        ("Exec.Mode", crate::images::EXEC_MODE.to_string()),
        ("Exec.Shares", crate::images::EXEC_SHARES.to_string()),
        // ⛔ SAID, NOT IMPLIED. TODO/supervise.md T-0601: a pidfd addresses one
        // process. podbox has no PID namespace, so a grandchild that reparents
        // is outside its reach, and a caller reads this rather than assuming
        // docker's containment.
        (
            "Contains",
            "the payload process only; podbox has no PID namespace, so a grandchild \
             that reparents is outside its reach"
                .to_string(),
        ),
        ("Noticed", c.noticed.clone().unwrap_or_else(|| "-".into())),
    ]
}

fn container_json(s: &podbox_image::Store, c: &Container) -> String {
    let fields = container_fields(s, c);
    let mut o = serde_json::Map::new();
    for (k, v) in &fields {
        // ⚠ The nested pair is nested here too, so a caller reading the
        // document and one reading `--format` do not find two shapes.
        if let Some(rest) = k.strip_prefix("Exec.") {
            let e = o
                .entry("Exec".to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(m) = e.as_object_mut() {
                m.insert(rest.to_string(), serde_json::Value::String(v.clone()));
            }
            continue;
        }
        o.insert(k.to_string(), serde_json::Value::String(v.clone()));
    }
    serde_json::Value::Array(vec![serde_json::Value::Object(o)]).to_string()
}

/// The image `run -d` and `create` need, and everything derived from it.
pub struct Prepared {
    pub record: podbox_image::Record,
    pub image: String,
    pub rootfs: String,
    pub argv: Vec<String>,
    pub env: Vec<String>,
    pub working_dir: String,
    pub name: Option<String>,
    pub rung: String,
    /// The banner the caller prints, built once so `run` and `run -d` say the
    /// same thing about the same machine.
    pub banner: String,
    pub detach: bool,
    pub rm: bool,
}

/// Resolve a container reference to the rootfs `exec` re-enters.
///
/// ⚠ A container name first, then an image reference, because `podbox exec` on a
/// running container is the common case and an image of the same name is the
/// unusual one.
pub fn rootfs_of(s: &podbox_image::Store, want: &str) -> Option<(String, State)> {
    let c = podbox_supervise::get(s, want).ok()?;
    Some((c.rootfs, c.state))
}

/// Shared by `run` and `create`: everything a container needs before it exists.
#[allow(clippy::too_many_arguments)]
pub fn platform_and_policy(
    verb: &str,
    platform: Option<&str>,
    insecure: &[String],
    tls_verify: Option<bool>,
) -> Result<(Platform, Policy), i32> {
    let p = Platform::wanted(platform).map_err(|e| {
        eprintln!("podbox {verb}: {e}");
        e.exit_code()
    })?;
    let pol = Policy::resolve(insecure, tls_verify).map_err(|e| {
        eprintln!("podbox {verb}: {e}");
        e.exit_code()
    })?;
    Ok((p, pol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_container_path_is_split_from_a_plain_one() {
        assert_eq!(
            split("web:/etc/hosts"),
            Some(("web".into(), "/etc/hosts".into()))
        );
        assert_eq!(split("/etc/hosts"), None);
        // ⚠ A path with a colon in it is not a container reference.
        assert_eq!(split("/tmp/a:b"), None);
        assert_eq!(split("web:"), None);
    }

    #[test]
    fn a_signal_is_taken_by_name_or_number_and_never_defaulted() {
        assert_eq!(signal_number("TERM"), Some(15));
        assert_eq!(signal_number("SIGKILL"), Some(9));
        assert_eq!(signal_number("9"), Some(9));
        // ⛔ Refused rather than defaulted: sending the wrong signal is not
        // something a caller can notice afterwards.
        assert_eq!(signal_number("NOPE"), None);
        assert_eq!(signal_number("0"), None);
        assert_eq!(signal_number("999"), None);
    }

    /// ⛔ Every declared field is built and every built field is declared.
    #[test]
    fn the_declared_container_fields_are_the_built_ones() {
        let d = std::env::temp_dir().join(format!("podbox-lc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let s = podbox_image::Store::open(&d).unwrap();
        let c = Container {
            id: "a".repeat(64),
            name: "n".into(),
            image: "img".into(),
            manifest_digest: "sha256:0".into(),
            rootfs: "/tmp".into(),
            argv: vec!["true".into()],
            env: Vec::new(),
            working_dir: "/".into(),
            created_at: "now".into(),
            started_at: None,
            finished_at: None,
            state: State::Created,
            pid: None,
            launcher_pid: None,
            exit_code: None,
            noticed: None,
            rung: "chroot".into(),
        };
        let built: Vec<&str> = container_fields(&s, &c).iter().map(|(k, _)| *k).collect();
        assert_eq!(built, CONTAINER_INSPECT_FIELDS);
        // ⛔ And a container with no exit code renders a dash, never a zero.
        let f = container_fields(&s, &c);
        assert_eq!(
            f.iter().find(|(k, _)| *k == "ExitCode").unwrap().1,
            "-".to_string()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_ps_fields_are_the_ones_the_template_check_allows() {
        let c = Container {
            id: "b".repeat(64),
            name: "n".into(),
            image: "img".into(),
            manifest_digest: "sha256:0".into(),
            rootfs: "/tmp".into(),
            argv: vec!["sleep".into(), "1".into()],
            env: Vec::new(),
            working_dir: "/".into(),
            created_at: "now".into(),
            started_at: None,
            finished_at: None,
            state: State::Running,
            pid: Some(7),
            launcher_pid: Some(6),
            exit_code: None,
            noticed: None,
            rung: "chroot".into(),
        };
        let built: Vec<&str> = ps_fields(&c, false).iter().map(|(k, _)| *k).collect();
        assert_eq!(built, ps_field_names());
        assert!(format::check("{{.Status}}", &ps_field_names()).is_ok());
        assert!(format::check("{{.Nope}}", &ps_field_names()).is_err());
    }
}
