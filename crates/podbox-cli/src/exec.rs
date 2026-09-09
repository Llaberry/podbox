//! `podbox exec`: [`TODO/enter.md`](../../../TODO/enter.md) T-0505.
//!
//! ⛔ **`docker exec` enters the container's namespaces. podbox has none to
//! enter.** Its `exec` re-runs the `TOOL.md` section 6.5 sequence against the
//! same rootfs, so it shares the filesystem tree with whatever else is in that
//! rootfs and **nothing else**: not the process table, not `/proc`, not
//! signals, not the original process's environment. A caller that assumes
//! otherwise gets a process that cannot see any of them.
//!
//! ⭐ That difference is said in three places a caller can reach, and the three
//! agree because they read one pair of constants: the banner on every run
//! ([`degradation`]), `podbox inspect --format '{{.Exec.Shares}}'`
//! ([`crate::images::EXEC_SHARES`]), and the `Degraded` row for the verb in the
//! parity table.
//!
//! ⚠ **The target is an image reference today, and a container name at M4.**
//! podbox has no containers yet ([`TODO/supervise.md`](../../../TODO/supervise.md)
//! T-1105), and the mechanism does not change when it does: a container name
//! resolves to the same rootfs and enters it the same way.

use std::io::Write;

use podbox_enter::{Fds, Plan, RootDir};
use podbox_image::error::{EXIT_CLI_ERROR, EXIT_FLAG_ERROR};
use podbox_image::platform::Platform;

pub const EXEC_USAGE: &str = "\
usage: podbox exec [options] <image> <command> [arg...]

  -e, --env K=V    set an environment variable. Repeatable; a later one wins
  -w, --workdir D  working directory inside the container
  --platform P     which platform of a multi-platform image to enter
  --add-host N:IP  add a name to the container's /etc/hosts. Repeatable
  --no-source-fixup
                   leave the image's package sources exactly as extracted, and
                   undo a rewrite an earlier run made (TODO/complete.md T-0411)
  --no-host-cas    as in run: leave the image's trust store alone
  --strict         ⛔ refuse to re-enter at all where anything about this
                   invocation is Degraded or Stub (TODO/cli.md T-0804)
  -t, --tty        ⛔ REFUSED BY NAME where /dev/ptmx is unusable, rather
                   than silently degraded (TODO/enter.md T-0503)

  ⛔ This is a FRESH CHROOT re-entry, not an entry into a running container.
    It shares the filesystem tree and nothing else. `podbox inspect --format
    '{{.Exec.Mode}}'` says the same thing to a program.

  ⛔ The image must already be extracted. `podbox exec` never pulls and never
    extracts: there would be nothing to re-enter, and a verb that creates what
    it claims to attach to is the lie TOOL.md section 4.1 forbids.
";

/// The one sentence that makes the degradation true rather than implied.
///
/// ⛔ Built from the same constants `inspect` reports, so the banner and the
/// machine-readable field cannot say different things.
pub fn degradation() -> String {
    format!(
        "podbox exec: this is a {} re-entry and not an entry into a running \
         container. It shares {} with anything else in this rootfs and nothing \
         else: not the process table, not /proc, not signals, not the original \
         process's environment (TODO/enter.md T-0505)\n",
        crate::images::EXEC_MODE,
        crate::images::EXEC_SHARES,
    )
}

#[derive(Debug)]
struct Opts {
    env: Vec<String>,
    workdir: Option<String>,
    platform: Option<String>,
    tty: bool,
    image: Option<String>,
    command: Vec<String>,
    ask: crate::complete::Ask,
}

/// ⛔ Parsing stops at the image name, exactly as `run`'s does: everything
/// after it is the payload's, dashes and all.
fn parse(args: &[String]) -> std::result::Result<Opts, i32> {
    let mut o = Opts {
        env: Vec::new(),
        workdir: None,
        platform: None,
        tty: false,
        image: None,
        command: Vec::new(),
        ask: crate::complete::Ask::default(),
    };
    let mut expecting: Option<&'static str> = None;
    for a in args {
        if let Some(flag) = expecting.take() {
            match flag {
                "-e" => o.env.push(a.clone()),
                "-w" => o.workdir = Some(a.clone()),
                "--add-host" => crate::complete::add_host(&mut o.ask, "exec", a)?,
                _ => o.platform = Some(a.clone()),
            }
            continue;
        }
        if o.image.is_some() {
            o.command.push(a.clone());
            continue;
        }
        // ⛔ TODO/cli.md T-0801. The table decides, exactly as it does for
        // `run`: an unlisted flag never reaches an arm, and a `None` row is
        // refused with its own reason.
        if a.starts_with('-') {
            crate::parity::admit("exec", a, EXEC_USAGE)?;
            o.ask
                .seen_flags
                .push(a.split('=').next().unwrap_or(a).to_string());
        }
        match a.as_str() {
            "-h" | "--help" => {
                print!("{EXEC_USAGE}");
                return Err(0);
            }
            "-t" | "--tty" => o.tty = true,
            "-i" | "--interactive" => {
                // ⚠ Accepted and a no-op, as in `run`: podbox does not detach
                // stdin, so it is already interactive when the caller's is.
            }
            "-e" | "--env" => expecting = Some("-e"),
            "-w" | "--workdir" => expecting = Some("-w"),
            "--platform" => expecting = Some("--platform"),
            "--add-host" => expecting = Some("--add-host"),
            other if other.starts_with("--add-host=") => {
                crate::complete::add_host(&mut o.ask, "exec", &other[11..])?
            }
            "--no-source-fixup" => o.ask.no_source_fixup = true,
            "--no-host-cas" => o.ask.no_host_cas = true,
            "--strict" => o.ask.strict = true,
            other if other.starts_with("--env=") => o.env.push(other[6..].to_string()),
            other if other.starts_with("--workdir=") => o.workdir = Some(other[10..].to_string()),
            other if other.starts_with("--platform=") => o.platform = Some(other[11..].to_string()),
            other if other.starts_with('-') => {
                // ⛔ Unreachable through the table above; an assertion, not a
                // fallback. See `run`'s own arm for why.
                return Err(crate::parity::no_arm("exec", other));
            }
            other => o.image = Some(other.to_string()),
        }
    }
    if let Some(flag) = expecting {
        eprintln!("podbox exec: {flag} needs a value");
        return Err(EXIT_FLAG_ERROR);
    }
    if o.image.is_none() {
        eprint!("{EXEC_USAGE}");
        return Err(EXIT_CLI_ERROR);
    }
    // ⛔ docker's rule, and podbox's for the same reason: `exec` has no default
    // command. An image's Cmd is what `run` starts, not what a second entry
    // re-runs, and silently re-running it is a process the caller did not ask
    // for.
    if o.command.is_empty() {
        eprintln!("podbox exec: a command is required. `exec` never falls back to the image's Cmd");
        eprint!("{EXEC_USAGE}");
        return Err(EXIT_CLI_ERROR);
    }
    Ok(o)
}

/// Enter an existing rootfs. ⭐ ONE PATH for a container and for an image: the
/// only difference is where the rootfs came from, and duplicating the entry
/// would be a second implementation of the thing this verb exists to be.
fn enter(
    target: &str,
    rootfs: &str,
    state: podbox_supervise::table::State,
    o: &Opts,
    store: &podbox_image::Store,
) -> i32 {
    use podbox_supervise::table::State;
    if state == State::Created {
        eprintln!(
            "podbox exec: {target} has been created and never started, so its rootfs              is there and nothing is running in it. ⚠ podbox will enter it anyway,              because a fresh chroot shares only the filesystem and needs nothing to              be running (TODO/enter.md T-0505)"
        );
    }
    // ⚠ The container's own environment is not inherited: T-0505's whole
    // subject is that this shares the filesystem and NOTHING else, and silently
    // copying the original's environment would be the implication it refuses.
    let env = podbox_enter::Plan::env_for(&[], &o.env);
    let path_dirs = podbox_enter::Plan::path_from(&env);
    let working_dir = o.workdir.clone().unwrap_or_else(|| "/".to_string());
    let findings = podbox_probe::run();
    let selection = podbox_probe::select::Selection::choose(&findings);
    let entered = podbox_enter::ENTERED_RUNG;
    let mut banner = podbox_probe::report::entry_banner(&findings, &selection, entered);
    if let Some(note) = crate::names::alias_note() {
        banner.push_str(&note);
    }
    banner.push_str(&degradation());
    // ⭐ M5. `exec` completes the rootfs exactly as `run` does, and for the same
    // reason: a fresh chroot re-entry is a fresh payload, and the `/dev/null` a
    // previous one turned into a file is still a file.
    let mut ask = o.ask.clone();
    ask.container_name = Some(target.to_string());
    let completion = match crate::complete::prepare("exec", rootfs, &ask, &mut banner) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let mut err = std::io::stderr().lock();
    if let Err(code) =
        crate::complete::strict_refusal("exec", &ask, entered.word(), &completion, &mut err)
    {
        return code;
    }
    if crate::complete::banner_quiet(store) {
        banner.clear();
    }
    if o.tty && !ptmx_usable(&findings) {
        let _ = write!(err, "{banner}");
        let _ = writeln!(err, "{TTY_REFUSAL}");
        return podbox_enter::EXIT_RUNTIME_ERROR;
    }
    let plan = Plan {
        argv: o.command.clone(),
        env,
        working_dir,
        fds: Fds::default(),
        banner,
        path_dirs,
    };
    let root = match RootDir::open(rootfs) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "podbox exec: {e}");
            return e.exit_code();
        }
    };
    match podbox_enter::run(&root, &plan, &mut err) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(err, "podbox exec: {e}");
            e.exit_code()
        }
    }
}

/// ⛔ `Ok` and nothing else. `Skip` means the row never ran, and TODO/probe.md
/// T-0109 rule 1 is that a skip may not read as a denial or as a pass: a pty
/// podbox could not measure is not one it may promise.
fn ptmx_usable(findings: &podbox_probe::Findings) -> bool {
    findings.rows.iter().any(|(n, out)| {
        n.starts_with("open(/dev/ptmx") && matches!(out.verdict, podbox_probe::verdict::Verdict::Ok)
    })
}

const TTY_REFUSAL: &str = "podbox exec: -t was asked for and /dev/ptmx is not usable on this \
machine, so podbox cannot allocate a pty. It refuses rather than running without one and \
letting the payload discover it (TODO/enter.md T-0503)";

pub fn exec(args: &[String]) -> i32 {
    let o = match parse(args) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let image = o.image.clone().expect("checked in parse");

    let platform = match Platform::wanted(o.platform.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("podbox exec: {e}");
            return e.exit_code();
        }
    };
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("podbox exec: {e}");
            return e.exit_code();
        }
    };

    // ⭐ M4. A CONTAINER NAME FIRST, then an image reference. `docker exec` takes
    // a container and podbox's takes either, because until M4 there were no
    // containers and an image was the only thing to re-enter. ⚠ The mechanism is
    // the same either way: a container's rootfs is a directory in the store and
    // so is an image's.
    if let Some((rootfs, state)) = crate::lifecycle::rootfs_of(&store, &image) {
        return enter(&image, &rootfs, state, &o, &store);
    }

    // ⛔ Never pulls. `exec` re-enters something that is already there, so a
    // reference the store does not hold is a refusal that names `run`, not a
    // fetch the caller did not ask for.
    let found = store.find_for(&image, Some(&platform)).unwrap_or_default();
    let others = found.other_platforms.clone();
    let Some(record) = found.one() else {
        if others.is_empty() {
            eprintln!(
                "podbox exec: {image} is not in the store. `podbox exec` never pulls: \
                 `podbox run {image} ...` is what puts it there"
            );
        } else {
            eprintln!(
                "podbox exec: the store holds {image} for {}, and {platform} was asked \
                 for. `podbox exec` never pulls",
                others.join(", ")
            );
        }
        return podbox_image::error::EXIT_RUNTIME_ERROR;
    };

    // ⛔ The lock BEFORE the extraction check, so a concurrent `rmi` cannot
    // delete the rootfs between podbox deciding it is there and entering it.
    // TODO/image.md T-0204, the same ordering `run` takes.
    let held = match store.hold(&record) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("podbox exec: {e}");
            return podbox_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    if !podbox_extract::is_extracted(&store, &record.manifest_digest) {
        eprintln!(
            "podbox exec: {image} is in the store and has never been extracted, so \
             there is no rootfs to re-enter. `podbox run {image} ...` or `podbox \
             extract {image}` is what puts one there"
        );
        return podbox_image::error::EXIT_RUNTIME_ERROR;
    }
    let (rootfs, _) = podbox_extract::paths(&store, &record.manifest_digest);
    let rootfs = rootfs.to_string_lossy().to_string();

    // --------------------------------------------------------------- the plan
    let cfg = match crate::run::config_of(&store, &record) {
        Ok(c) => c,
        Err(code) => return code,
    };
    // ⛔ The image's Entrypoint is NOT prepended. docker's `exec` runs the
    // command given and nothing around it, and an entrypoint that wraps a
    // second entry is a process the caller did not write.
    let argv = o.command.clone();
    let env = Plan::env_for(&cfg.config.env, &o.env);
    let path_dirs = Plan::path_from(&env);
    let working_dir = o
        .workdir
        .clone()
        .unwrap_or_else(|| cfg.config.working_dir.clone());

    // ------------------------------------------------------------- the banner
    let findings = podbox_probe::run();
    let selection = podbox_probe::select::Selection::choose(&findings);
    let entered = podbox_enter::ENTERED_RUNG;
    let mut banner = podbox_probe::report::entry_banner(&findings, &selection, entered);
    // ⭐ TODO/cli.md T-0803. Where podbox was reached under somebody else's
    // name, the banner says which name was used and that this is podbox.
    // Taking the name is the product requirement; taking it silently is what
    // TOOL.md section 4.1 forbids.
    if let Some(note) = crate::names::alias_note() {
        banner.push_str(&note);
    }
    banner.push_str(&degradation());
    // ⭐ M5. The same completion the container path takes, from the same
    // function: two entry paths that complete a rootfs differently would be two
    // answers to one question, and the one nobody exercises is the one that
    // diverges.
    let mut ask = o.ask.clone();
    ask.container_name = Some(image.clone());
    let completion = match crate::complete::prepare("exec", &rootfs, &ask, &mut banner) {
        Ok(r) => r,
        Err(code) => return code,
    };

    let mut err = std::io::stderr().lock();
    if let Err(code) =
        crate::complete::strict_refusal("exec", &ask, entered.word(), &completion, &mut err)
    {
        return code;
    }
    if crate::complete::banner_quiet(&store) {
        banner.clear();
    }
    if o.tty {
        // ⛔ T-0503, and the same rule as `run`: `Ok` and nothing else, because
        // a skip is not a pass.
        let usable = findings.rows.iter().any(|(n, out)| {
            n.starts_with("open(/dev/ptmx")
                && matches!(out.verdict, podbox_probe::verdict::Verdict::Ok)
        });
        if !usable {
            let _ = write!(err, "{banner}");
            let _ = writeln!(
                err,
                "podbox exec: -t was asked for and /dev/ptmx is not usable on this \
                 machine, so podbox cannot allocate a pty. It refuses rather than \
                 running without one and letting the payload discover it \
                 (TODO/enter.md T-0503)"
            );
            return podbox_enter::EXIT_RUNTIME_ERROR;
        }
    }

    let plan = Plan {
        argv,
        env,
        working_dir,
        fds: Fds::default(),
        banner,
        path_dirs,
    };

    // -------------------------------------------------------------- the entry
    let root = match RootDir::open(&rootfs) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "podbox exec: {e}");
            return e.exit_code();
        }
    };
    if let Err(e) = held.hand_to_payload() {
        let _ = writeln!(err, "podbox exec: {e}");
        return podbox_image::error::EXIT_RUNTIME_ERROR;
    }
    match podbox_enter::run(&root, &plan, &mut err) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(err, "podbox exec: {e}");
            e.exit_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parsing_stops_at_the_image_name() {
        let o = parse(&v(&["-e", "A=1", "alpine", "ls", "-l", "-e"])).unwrap();
        assert_eq!(o.env, v(&["A=1"]));
        assert_eq!(o.image.as_deref(), Some("alpine"));
        assert_eq!(o.command, v(&["ls", "-l", "-e"]));
    }

    /// ⛔ The difference from `run`, and it is deliberate rather than an
    /// omission: an image's Cmd is what `run` starts.
    #[test]
    fn exec_has_no_default_command() {
        assert_eq!(parse(&v(&["alpine"])).unwrap_err(), EXIT_CLI_ERROR);
        assert!(parse(&v(&["alpine", "true"])).is_ok());
    }

    /// ⭐ TODO/cli.md T-0801. Every flag the table ADMITS for this verb reaches
    /// an arm of this parser. The other direction is held by construction: an
    /// argument starting with `-` goes through `parity::admit` first, so an arm
    /// for a flag with no row is unreachable rather than a second surface.
    #[test]
    fn every_flag_the_table_admits_is_handled_by_this_parser() {
        for r in crate::parity::TABLE
            .iter()
            .filter(|r| r.verb == "exec" && r.flag.is_some())
        {
            if r.status == crate::parity::Status::None {
                // ⚠ A `None` row is refused BY `admit`, so reaching an arm is
                // exactly what it must not do. Asserted the other way round.
                let got = parse(&v(&[
                    r.flag.unwrap().split(',').next().unwrap().trim(),
                    "img",
                    "true",
                ]));
                assert_eq!(
                    got.unwrap_err(),
                    EXIT_FLAG_ERROR,
                    "{:?} was not refused",
                    r.flag
                );
                continue;
            }
            for spelling in r.flag.unwrap().split(',') {
                let f = spelling.trim();
                if f == "-h" || f == "--help" {
                    assert_eq!(parse(&v(&[f])).unwrap_err(), 0, "{f} did not print usage");
                    continue;
                }
                // ⚠ ASSERTED ON THE EXIT CODE, not on a shape parsing. A
                // valued flag needs a value, a boolean one does not, and one
                // with a closed set of values (`--pull`) rejects any value this
                // test could invent: all three are legitimate and only one of
                // them parses. What no legitimate arm ever returns is
                // EXIT_RUNTIME_ERROR, which is the fallback arm's own code and
                // means exactly "the table admits this flag and nothing
                // implements it".
                // ⚠ The assertion is `parity::no_arm`'s panic. See the same
                // test in `run.rs` for why the code comparison it replaced
                // stopped being able to fail.
                for shape in [v(&[f, "V", "img", "true"]), v(&[f, "img", "true"])] {
                    let _ = parse(&shape);
                }
            }
        }
    }

    #[test]
    fn a_flag_needing_a_value_does_not_swallow_the_image() {
        assert_eq!(parse(&v(&["--platform"])).unwrap_err(), EXIT_FLAG_ERROR);
        assert_eq!(parse(&v(&["-w"])).unwrap_err(), EXIT_FLAG_ERROR);
    }

    /// ⭐ The banner and the machine-readable field are one pair of constants,
    /// so a caller reading either gets the same answer.
    #[test]
    fn the_banner_says_what_inspect_reports() {
        let d = degradation();
        assert!(d.contains(crate::images::EXEC_MODE), "{d}");
        assert!(d.contains(crate::images::EXEC_SHARES), "{d}");
        assert!(d.contains("T-0505"), "{d}");
    }
}
