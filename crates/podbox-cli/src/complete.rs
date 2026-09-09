//! Where the CLI meets [`podbox_complete`], and where
//! [`TODO/cli.md`](../../../TODO/cli.md) T-0804's one switch is enforced.
//!
//! ⭐ **`--strict` turns every degradation into a refusal**, so a caller that
//! needs real semantics demands them in one flag instead of parsing the banner.
//! Three things feed it, and all three are read from something that was
//! measured rather than declared:
//!
//! 1. **the flags the caller passed**, against the parity table's own status
//!    for each. A `Degraded` or `Stub` row is a difference from docker that
//!    this invocation actually incurs;
//! 2. **the rung podbox selected**, from the probe. Below `namespace` the
//!    payload is in a `chroot` sharing this machine's process table, network,
//!    IPC and mounts;
//! 3. **the completion layer's own report**, one row per fixup, from
//!    [`podbox_complete::Report::degradations`].
//!
//! ⛔ A `None` row is already fatal without `--strict`, and has been since
//! T-0801 made the table binding: `parity::admit` refuses it in the parser. So
//! `--strict` is about the two statuses that otherwise let a run proceed, and
//! about the run itself.

use std::io::Write;

use podbox_image::error::{EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};

/// What the caller asked the completion layer for.
#[derive(Debug, Clone, Default)]
pub struct Ask {
    pub container_name: Option<String>,
    pub add_hosts: Vec<String>,
    /// `--no-source-fixup`. T-0411.
    pub no_source_fixup: bool,
    /// `--no-host-cas`. T-0407.
    pub no_host_cas: bool,
    /// `--no-steps`. T-0412.
    pub no_steps: bool,
    /// `--strict`. T-0804.
    pub strict: bool,
    /// Every flag this invocation actually passed, in the caller's spelling, so
    /// `--strict` can look each one up. ⚠ Collected by the parser rather than
    /// re-derived: a flag that took a value is one argument here and two on the
    /// command line, and re-splitting is a second parser.
    pub seen_flags: Vec<String>,
}

/// `<store>/config`, and the one key T-0804 defines.
///
/// ⭐ **Suppressible by config, never by the command line.** `ruri` allows
/// `--disable-warnings` to silence its degradation notices, which is the
/// `sandlock` failure mode with a flag in front of it. podbox's equivalent is a
/// file a machine's operator sets once, so a single run cannot hide what it is.
pub const CONFIG_FILE: &str = "config";
pub const BANNER_KEY: &str = "banner";

/// Is the banner suppressed on this machine? ⛔ Default no, always.
pub fn banner_quiet(store: &podbox_image::Store) -> bool {
    let p = store.root().join(CONFIG_FILE);
    let Ok(text) = std::fs::read_to_string(&p) else {
        return false;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == BANNER_KEY {
                return matches!(v.trim(), "quiet" | "off" | "false" | "0");
            }
        }
    }
    false
}

/// Run the completion layer over `rootfs`, fold its report into `banner`, and
/// apply `--strict`.
///
/// ⛔ **The completion layer never fails the run on its own.** A fixup podbox
/// could not apply is a `Failed` row and a banner line; the caller turns that
/// into a refusal with `--strict` and not otherwise, because a rootfs podbox
/// could not write a resolver into is one the payload may still be able to use.
pub fn prepare(
    verb: &str,
    rootfs: &str,
    ask: &Ask,
    banner: &mut String,
) -> Result<podbox_complete::Report, i32> {
    let opts = podbox_complete::Options {
        container_name: ask.container_name.clone(),
        add_hosts: ask.add_hosts.clone(),
        source_fixup: !ask.no_source_fixup,
        host_cas: !ask.no_host_cas,
        steps: !ask.no_steps,
        ..podbox_complete::Options::default()
    };
    let report = match podbox_complete::complete(rootfs, &opts) {
        Ok(r) => r,
        Err(e) => {
            // ⛔ Even this is not fatal without --strict: the rootfs is there
            // and the payload may not need what podbox could not do.
            banner.push_str(&format!(
                "podbox: complete: ⛔ the completion layer could not run at all: {e}. \
                 The payload is entering a rootfs podbox did not prepare\n"
            ));
            if ask.strict {
                eprint!("{banner}");
                eprintln!("podbox {verb}: --strict, and the completion layer could not run");
                return Err(EXIT_RUNTIME_ERROR);
            }
            return Ok(podbox_complete::Report::default());
        }
    };
    banner.push_str(&report.banner());
    Ok(report)
}

/// T-0804's refusal, over all three inputs.
///
/// ⚠ Called after the banner is built and before the payload starts, so a
/// caller that is refused still gets the whole account of why.
pub fn strict_refusal(
    verb: &str,
    ask: &Ask,
    rung: &str,
    report: &podbox_complete::Report,
    err: &mut dyn Write,
) -> Result<(), i32> {
    if !ask.strict {
        return Ok(());
    }
    let mut reasons: Vec<String> = Vec::new();

    // 1. the flags this invocation passed.
    for f in &ask.seen_flags {
        let Some(row) = crate::parity::flag(verb, f) else {
            continue;
        };
        if matches!(
            row.status,
            crate::parity::Status::Degraded | crate::parity::Status::Stub
        ) {
            reasons.push(format!(
                "{} is {} in the parity table: {}",
                row.flag.unwrap_or(f),
                row.status.word(),
                row.note
            ));
        }
    }

    // 2. the rung. ⛔ The floor is `Selection`'s own, not a word repeated here:
    // `podbox probe --strict` gates on the same constant, and two spellings of
    // one floor is the drift T-1206 is about.
    let floor = podbox_probe::select::Selection::STRICT_FLOOR.word();
    if rung != floor {
        reasons.push(format!(
            "the selected rung is `{rung}` and not `{floor}`, so the payload shares \
             this machine's process table, network, IPC and mount namespaces"
        ));
    }

    // 3. the completion layer.
    for f in report.degradations() {
        reasons.push(format!(
            "{} {} ({}): {}",
            f.action.word(),
            if f.path.is_empty() { "-" } else { &f.path },
            f.entry,
            f.detail
        ));
    }

    // 4. ⭐ T-0412's steps, and they are counted BEFORE any of them runs. podbox
    // executing a command inside somebody else's image that the caller did not
    // write is exactly the kind of difference from docker `--strict` exists to
    // refuse, and refusing after running one would be podbox acting and then
    // declining to have acted.
    for s in &report.steps {
        reasons.push(format!(
            "podbox would run `{}` ({}) inside this image before the payload: {}",
            s.argv.join(" "),
            s.entry,
            s.why
        ));
    }

    if reasons.is_empty() {
        return Ok(());
    }
    let _ = writeln!(
        err,
        "podbox {verb}: --strict, and this run is degraded in {} way(s). podbox \
         refuses rather than running and letting the payload discover them \
         (TODO/cli.md T-0804):",
        reasons.len()
    );
    for r in &reasons {
        let _ = writeln!(err, "  - {r}");
    }
    Err(EXIT_RUNTIME_ERROR)
}

/// The bound one step gets.
///
/// ⛔ **A bound rather than a wait**, because `docs/AGENTS.md` makes that a
/// requirement of podbox and not only of the agent working on it: a step is a
/// program from somebody else's image and podbox has no idea what it does.
/// ⚠ Five minutes because `pacman-key --populate` builds a keyring with `gpg`
/// and `openssl rehash` reads every file in a directory of six hundred; both
/// are seconds here and neither has a documented worst case.
pub const STEP_TIMEOUT_MS: i64 = 300_000;

/// ⭐ **T-0412. Run the report's steps INSIDE the rootfs, before the payload.**
///
/// The completion layer runs on the host and every fixup it can make is a
/// write. Two are not -- `pacman-key --init` runs `gpg` in the rootfs, and a
/// hash-indexed CApath is indexed by a program that reads the certificates --
/// so the layer returns an argv and this runs it.
///
/// ⛔ **The banner has already named every one of these**, which is why this is
/// the caller's job rather than the library's: the announcement has to precede
/// the command, and only the caller knows what else it is about to print.
///
/// ⛔ **A step that fails is a `Failed` row and never a failed run.** The
/// payload may not need what the step would have provided, and refusing here
/// would make podbox less useful than the bare `chroot` it replaces. `--strict`
/// is how a caller turns it into a refusal, and it refuses before any step runs.
///
/// ⚠ The step's stdout is redirected to podbox's STDERR. T-1104: the payload
/// owns stdout, and a step's output on it would corrupt every pipeline
/// `podbox run <image> cmd | consumer` is in.
pub fn run_steps(
    verb: &str,
    rootfs: &str,
    env: &[String],
    report: &mut podbox_complete::Report,
    quiet: bool,
    err: &mut dyn Write,
) {
    if report.steps.is_empty() {
        return;
    }
    let steps = report.steps.clone();
    let root = match podbox_enter::RootDir::open(rootfs) {
        Ok(r) => r,
        Err(e) => {
            report.fixups.push(failed_step(
                steps[0].entry,
                steps[0].id,
                format!("podbox could not open the rootfs to run its steps in: {e}"),
            ));
            let _ = writeln!(err, "podbox {verb}: complete: {e}");
            return;
        }
    };
    for (i, s) in steps.iter().enumerate() {
        let started = std::time::Instant::now();
        let outcome = one_step(&root, s, env);
        let took = started.elapsed().as_secs_f64();
        let (line, failure) = match outcome {
            Ok(podbox_enter::Bounded::Exited(0)) => (
                format!(
                    "podbox: complete: step {} of {}: `{}` exited 0 in {took:.1} s",
                    i + 1,
                    steps.len(),
                    s.argv.join(" ")
                ),
                None,
            ),
            Ok(podbox_enter::Bounded::Exited(c)) => (
                format!(
                    "podbox: complete: step {} of {}: `{}` exited {c} in {took:.1} s. \
                     ⚠ podbox does NOT fail the run for it: the payload may not need \
                     what it would have done",
                    i + 1,
                    steps.len(),
                    s.argv.join(" ")
                ),
                Some(format!("`{}` exited {c}", s.argv.join(" "))),
            ),
            Ok(podbox_enter::Bounded::TimedOut { after_ms }) => (
                format!(
                    "podbox: complete: step {} of {}: `{}` was still running after \
                     {} s and podbox killed it",
                    i + 1,
                    steps.len(),
                    s.argv.join(" "),
                    after_ms / 1000
                ),
                Some(format!(
                    "`{}` did not finish within {} s and was killed",
                    s.argv.join(" "),
                    after_ms / 1000
                )),
            ),
            Err(e) => (
                format!(
                    "podbox: complete: step {} of {}: `{}` could not be started: {e}",
                    i + 1,
                    steps.len(),
                    s.argv.join(" ")
                ),
                Some(format!("`{}` could not be started: {e}", s.argv.join(" "))),
            ),
        };
        if !quiet {
            let _ = writeln!(err, "{line}");
        }
        report.fixups.push(match failure {
            None => podbox_complete::Fixup {
                entry: s.entry,
                id: s.id,
                path: String::new(),
                action: podbox_complete::Action::Ran,
                detail: format!("`{}` exited 0. {}", s.argv.join(" "), s.why),
                degraded: false,
            },
            Some(why) => failed_step(s.entry, s.id, why),
        });
    }
}

fn failed_step(entry: &'static str, id: &'static str, why: String) -> podbox_complete::Fixup {
    podbox_complete::Fixup {
        entry,
        id,
        path: String::new(),
        action: podbox_complete::Action::Failed,
        detail: why,
        degraded: true,
    }
}

/// One step, entered exactly as the payload is.
///
/// ⛔ The same `podbox_enter` sequence and not a second one: a step that
/// resolved its program in the parent, or entered by a path rather than by the
/// descriptor T-0504 holds, would be a quieter entry path with none of the
/// guarantees the loud one has.
fn one_step(
    root: &podbox_enter::RootDir,
    s: &podbox_complete::Step,
    env: &[String],
) -> Result<podbox_enter::Bounded, podbox_enter::Error> {
    // ⛔ The step's stdout becomes podbox's stderr, and it is DUPLICATED first:
    // handing `(1, 2)` to the child's `dup2`-then-close loop would close the
    // child's own stderr with it.
    let mirror = podbox_probe::sys::dup_cloexec(2)
        .map_err(|e| podbox_enter::Error::Runtime(format!("dup of stderr: {}", e.name())))?;
    let plan = podbox_enter::Plan {
        argv: s.argv.clone(),
        env: env.to_vec(),
        working_dir: "/".to_string(),
        fds: podbox_enter::Fds {
            pass: vec![(1, mirror)],
        },
        // ⚠ Empty: the banner named this step before podbox got here.
        banner: String::new(),
        path_dirs: Vec::new(),
    };
    let mut sink = std::io::sink();
    let spawned = podbox_enter::spawn(root, &plan, &mut sink);
    // ⚠ The parent's copy goes here whatever happened: the child got its own.
    let _ = podbox_probe::sys::close(mirror);
    spawned?.wait_bounded(STEP_TIMEOUT_MS)
}

/// Parse `--add-host name:ip` and friends out of the shared argument surface.
///
/// ⚠ Returned as an error code rather than a panic so the caller's own usage
/// text is what the user sees.
pub fn add_host(ask: &mut Ask, verb: &str, value: &str) -> Result<(), i32> {
    if !value.contains(':') {
        eprintln!("podbox {verb}: --add-host takes name:ip, not {value:?}");
        return Err(EXIT_FLAG_ERROR);
    }
    ask.add_hosts.push(value.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_with_degradation() -> podbox_complete::Report {
        let mut r = podbox_complete::Report::default();
        r.fixups.push(podbox_complete::Fixup {
            entry: "T-0401",
            id: "dev-shim",
            path: "dev/null".into(),
            action: podbox_complete::Action::Created,
            detail: "a regular file".into(),
            degraded: true,
        });
        r
    }

    /// ⛔ Without `--strict` a degraded run is a run. The banner is the report,
    /// and refusing by default would make podbox useless on the machines it
    /// exists for.
    #[test]
    fn without_strict_nothing_is_refused() {
        let ask = Ask::default();
        let mut out = Vec::new();
        assert!(
            strict_refusal("run", &ask, "chroot", &report_with_degradation(), &mut out).is_ok()
        );
        assert!(out.is_empty());
    }

    /// ⭐ With `--strict`, all three inputs are named in one refusal rather
    /// than the first one found.
    #[test]
    fn strict_names_every_reason_at_once() {
        let ask = Ask {
            strict: true,
            seen_flags: vec!["-i".into(), "--rm".into()],
            ..Ask::default()
        };
        let mut out = Vec::new();
        let e = strict_refusal("run", &ask, "chroot", &report_with_degradation(), &mut out)
            .unwrap_err();
        assert_eq!(e, EXIT_RUNTIME_ERROR);
        let s = String::from_utf8(out).unwrap();
        // the Stub flag
        assert!(s.contains("-i, --interactive"), "{s}");
        // the rung
        assert!(s.contains("`chroot`"), "{s}");
        // the fixup
        assert!(s.contains("dev/null"), "{s}");
        // ⚠ and NOT the Native one.
        assert!(!s.contains("--rm is"), "{s}");
        assert!(s.contains("3 way(s)"), "{s}");
    }

    /// ⭐ T-0412. A step is a reason on its own, and it is counted BEFORE any of
    /// them runs: podbox executing a command inside somebody else's image that
    /// the caller did not write is a difference from docker, and refusing after
    /// running one would be podbox acting and then declining to have acted.
    #[test]
    fn strict_refuses_a_run_whose_only_difference_is_a_step() {
        let mut report = podbox_complete::Report::default();
        report.steps.push(podbox_complete::Step {
            entry: "T-0412",
            id: "ca-hash-dir",
            argv: vec![
                "/usr/bin/openssl".into(),
                "rehash".into(),
                "/etc/ssl/certs".into(),
            ],
            why: "libzypp reads this directory and no CAfile at all".into(),
        });
        let ask = Ask {
            strict: true,
            ..Ask::default()
        };
        let floor = podbox_probe::select::Selection::STRICT_FLOOR.word();
        let mut out = Vec::new();
        let e = strict_refusal("run", &ask, floor, &report, &mut out).unwrap_err();
        assert_eq!(e, EXIT_RUNTIME_ERROR);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("/usr/bin/openssl rehash /etc/ssl/certs"), "{s}");
        assert!(s.contains("1 way(s)"), "{s}");
    }

    /// ⚠ A run with nothing degraded passes `--strict`, or the flag would be a
    /// refusal rather than a gate.
    #[test]
    fn strict_passes_a_run_with_no_degradation() {
        let ask = Ask {
            strict: true,
            seen_flags: vec!["--rm".into()],
            ..Ask::default()
        };
        let floor = podbox_probe::select::Selection::STRICT_FLOOR.word();
        let mut out = Vec::new();
        assert!(strict_refusal(
            "run",
            &ask,
            floor,
            &podbox_complete::Report::default(),
            &mut out
        )
        .is_ok());
    }

    #[test]
    fn the_banner_is_never_quiet_by_default() {
        let d = std::env::temp_dir().join(format!("podbox-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::env::set_var("PODBOX_STORE", &d);
        let s = podbox_image::open_store().unwrap();
        assert!(!banner_quiet(&s));
        std::fs::write(s.root().join(CONFIG_FILE), "# a comment\nbanner = quiet\n").unwrap();
        assert!(banner_quiet(&s));
        std::env::remove_var("PODBOX_STORE");
        let _ = std::fs::remove_dir_all(&d);
    }
}
