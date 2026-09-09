//! `podbox run`: milestone M3.
//!
//! [`TODO/milestones.md`](../../../TODO/milestones.md) T-1104,
//! [`TODO/enter.md`](../../../TODO/enter.md) T-0501 to T-0506,
//! [`TODO/cli.md`](../../../TODO/cli.md) T-0802.
//!
//! ⭐ **This verb is the product requirement the whole specification exists
//! for**: an agent that knows `docker` needs zero new knowledge. So its
//! argument surface, its output channels and its exit codes are docker's, and
//! where podbox cannot honour something it says so in one line rather than
//! pretending.

use std::io::Write;

use podbox_enter::{binfmt, Fds, Plan, RootDir};
use podbox_image::error::{EXIT_CLI_ERROR, EXIT_FLAG_ERROR};
use podbox_image::platform::Platform;
use podbox_image::transport::Policy;

pub const RUN_USAGE: &str = "\
usage: podbox run [options] <image> [command] [arg...]

  -d, --detach     start the container and print its id, and do not wait
  --name NAME      a name for the container. ⚠ Refused if one already has it
  --rm             remove the extracted rootfs when the payload exits
  -e, --env K=V    set an environment variable. Repeatable; a later one wins
  -w, --workdir D  working directory inside the container
  --entrypoint P   replace the image's entrypoint. ⚠ As docker: this also
                   drops the image's Cmd, because those were that
                   entrypoint's default arguments
  --add-host N:IP  add a name to the container's /etc/hosts. Repeatable
  --no-source-fixup
                   leave the image's package sources exactly as extracted.
                   ⚠ podbox rewrites http:// to https:// for a mirror that
                   answers over HTTPS, because tcp/80 HANGS on the runtimes
                   podbox targets. This turns that off, and UNDOES a rewrite
                   an earlier container made in the same shared rootfs
  --no-host-cas    do not append this machine's announced CA bundle
                   ($SSL_CERT_FILE, $CURL_CA_BUNDLE, $REQUESTS_CA_BUNDLE) to
                   the image's own trust store. ⚠ Where this machine
                   intercepts TLS, an https package source then fails to
                   verify inside the container, exactly as it does under
                   docker
  --no-steps       ⛔ do not run any COMMAND inside the rootfs before the
                   payload. Two fixups cannot be made from outside the
                   chroot -- `pacman-key --init` for an empty keyring, and
                   `openssl rehash` for a hash-indexed CA directory, which is
                   the only trust store libzypp reads -- and podbox names
                   each on the banner before it runs it. This refuses them
                   all, and the fixup log then says what the caller gave up
  --strict         ⛔ refuse to run at all where anything about this
                   invocation is Degraded or Stub: a flag, the selected rung,
                   or a fixup the completion layer had to make
  --platform P     which platform of a multi-platform image to run
  --pull WHEN      never | missing (default) | always
  --insecure-registry HOST, --tls-verify=B
                   as `podbox pull`; used only when something must be fetched
  -t, --tty        ⛔ REFUSED BY NAME where /dev/ptmx is unusable, rather
                   than silently degraded (TODO/enter.md T-0503)

  ⛔ podbox run enters a CHROOT, not a container. It shares this machine's
    process table, network, IPC and mount namespaces with the payload. The
    banner on stderr says so on every run and names what the selected rung
    must never claim.

  ⚠ The payload owns stdout. Every word podbox prints goes to stderr, so
    `podbox run <image> cmd | consumer` gives the consumer the payload's
    bytes and nothing else.

  ⚠ The exit code is the payload's own, and a signalled payload is 128+signal.
    125 is podbox failing to run the command, 126 the command found and not
    invocable, 127 not found. Those are docker's.
";

#[derive(Debug)]
struct Opts {
    rm: bool,
    detach: bool,
    name: Option<String>,
    env: Vec<String>,
    workdir: Option<String>,
    entrypoint: Option<String>,
    platform: Option<String>,
    pull: String,
    insecure: Vec<String>,
    tls_verify: Option<bool>,
    tty: bool,
    image: Option<String>,
    command: Vec<String>,
    /// M5 and T-0804. ⚠ Carried in one struct so `run`, `create` and the
    /// launcher cannot each grow their own copy of the same three answers.
    ask: crate::complete::Ask,
}

/// ⛔ Parsing stops at the image name: everything after it is the payload's.
/// `podbox run alpine ls -l` must pass `-l` to `ls` and not read it as podbox's,
/// which is docker's rule and the one thing a caller cannot work around.
fn parse(args: &[String]) -> std::result::Result<Opts, i32> {
    let mut o = Opts {
        rm: false,
        detach: false,
        name: None,
        env: Vec::new(),
        workdir: None,
        entrypoint: None,
        platform: None,
        pull: "missing".into(),
        insecure: Vec::new(),
        tls_verify: None,
        tty: false,
        image: None,
        command: Vec::new(),
        ask: crate::complete::Ask::default(),
    };
    let mut expecting: Option<&'static str> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(flag) = expecting.take() {
            match flag {
                "-e" => o.env.push(a.clone()),
                "-w" => o.workdir = Some(a.clone()),
                "--entrypoint" => o.entrypoint = Some(a.clone()),
                "--platform" => o.platform = Some(a.clone()),
                "--pull" => o.pull = a.clone(),
                "--name" => o.name = Some(a.clone()),
                "--add-host" => crate::complete::add_host(&mut o.ask, "run", a)?,
                _ => o.insecure.push(a.clone()),
            }
            i += 1;
            continue;
        }
        if o.image.is_some() {
            // ⛔ Past the image name. Everything from here is the payload's,
            // dashes and all.
            o.command.push(a.clone());
            i += 1;
            continue;
        }
        // ⛔ TODO/cli.md T-0801. THE TABLE DECIDES, and the match arms below
        // only implement what it already admitted. A flag with no row cannot
        // reach an arm, so an arm for an unlisted flag is unreachable rather
        // than a second, quieter surface; and a `None` row is refused here with
        // its own reason instead of reading as an unknown option.
        if a.starts_with('-') {
            crate::parity::admit("run", a, RUN_USAGE)?;
            // ⭐ T-0804 reads this. Recorded HERE, at the one place every flag
            // passes through, so a flag added to a match arm and forgotten in a
            // second list cannot exist.
            o.ask
                .seen_flags
                .push(a.split('=').next().unwrap_or(a).to_string());
        }
        match a.as_str() {
            "-h" | "--help" => {
                print!("{RUN_USAGE}");
                return Err(0);
            }
            "--rm" => o.rm = true,
            "-d" | "--detach" => o.detach = true,
            "--name" => expecting = Some("--name"),
            other if other.starts_with("--name=") => o.name = Some(other[7..].to_string()),
            "-t" | "--tty" => o.tty = true,
            "-i" | "--interactive" => {
                // ⚠ Accepted and a no-op, deliberately: podbox does not detach
                // stdin, so it is already interactive when the caller's is.
                // TODO/cli.md T-0801 calls this Stub and the banner lists it.
            }
            "-e" | "--env" => expecting = Some("-e"),
            "-w" | "--workdir" => expecting = Some("-w"),
            "--entrypoint" => expecting = Some("--entrypoint"),
            "--platform" => expecting = Some("--platform"),
            "--pull" => expecting = Some("--pull"),
            "--insecure-registry" => expecting = Some("--insecure-registry"),
            "--tls-verify" => o.tls_verify = Some(true),
            "--add-host" => expecting = Some("--add-host"),
            other if other.starts_with("--add-host=") => {
                crate::complete::add_host(&mut o.ask, "run", &other[11..])?
            }
            "--no-source-fixup" => o.ask.no_source_fixup = true,
            "--no-host-cas" => o.ask.no_host_cas = true,
            "--no-steps" => o.ask.no_steps = true,
            "--strict" => o.ask.strict = true,
            other if other.starts_with("--env=") => o.env.push(other[6..].to_string()),
            other if other.starts_with("--workdir=") => o.workdir = Some(other[10..].to_string()),
            other if other.starts_with("--entrypoint=") => {
                o.entrypoint = Some(other[13..].to_string())
            }
            other if other.starts_with("--platform=") => o.platform = Some(other[11..].to_string()),
            other if other.starts_with("--pull=") => o.pull = other[7..].to_string(),
            other if other.starts_with("--insecure-registry=") => {
                o.insecure.push(other[20..].to_string())
            }
            other if other.starts_with("--tls-verify=") => match &other[13..] {
                "true" | "1" => o.tls_verify = Some(true),
                "false" | "0" => o.tls_verify = Some(false),
                v => {
                    eprintln!("podbox run: --tls-verify takes true or false, not {v:?}");
                    return Err(EXIT_FLAG_ERROR);
                }
            },
            other if other.starts_with('-') => {
                // ⛔ Unreachable through the table above, and it is here as an
                // assertion rather than as a fallback: the table admitted this
                // flag and this parser has no arm for it, which is a defect in
                // podbox and is reported as one.
                return Err(crate::parity::no_arm("run", other));
            }
            other => o.image = Some(other.to_string()),
        }
        i += 1;
    }
    if let Some(flag) = expecting {
        eprintln!("podbox run: {flag} needs a value");
        return Err(EXIT_FLAG_ERROR);
    }
    if o.image.is_none() {
        eprint!("{RUN_USAGE}");
        return Err(EXIT_CLI_ERROR);
    }
    if !matches!(o.pull.as_str(), "never" | "missing" | "always") {
        eprintln!(
            "podbox run: --pull takes never, missing or always, not {:?}",
            o.pull
        );
        return Err(EXIT_FLAG_ERROR);
    }
    Ok(o)
}

pub fn run(args: &[String]) -> i32 {
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("podbox run: {e}");
            return e.exit_code();
        }
    };
    let p = match prepare(args, &store, "run") {
        Ok(p) => p,
        Err(code) => return code,
    };
    let mut err = std::io::stderr().lock();

    // ⭐ M4. `-d` hands the container to a detached launcher and returns as soon
    // as its payload is running, which is a condition rather than a duration:
    // TODO/supervise.md T-0602.
    if p.detach {
        // ⚠ The banner was printed by `prepare`, which had to print it before
        // running T-0412's steps inside the rootfs.
        drop(err);
        let c = match podbox_supervise::create(
            &store,
            p.name.as_deref(),
            &p.image,
            &p.record.manifest_digest,
            &p.rootfs,
            p.argv.clone(),
            p.env.clone(),
            p.working_dir.clone(),
            &p.rung,
            p.completion.clone(),
            p.completion_degraded,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("podbox run: {e}");
                return podbox_image::error::EXIT_RUNTIME_ERROR;
            }
        };
        return match podbox_supervise::start(&store, &c.id, &p.record) {
            Ok(c) => {
                println!("{}", c.id);
                0
            }
            Err(e) => {
                eprintln!("podbox run: {e}");
                // ⚠ The record is removed again: a container that could not be
                // started is not one `ps` should list as created, and leaving
                // it would also hold its name against a retry.
                let _ = podbox_supervise::remove(&store, &c.id, true);
                podbox_image::error::EXIT_RUNTIME_ERROR
            }
        };
    }

    // ---------------------------------------------------- the foreground entry
    let held = match store.hold(&p.record) {
        Ok(h) => h,
        Err(e) => {
            let _ = writeln!(err, "podbox run: {e}");
            return podbox_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    let root = match RootDir::open(&p.rootfs) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "podbox run: {e}");
            return e.exit_code();
        }
    };
    let plan = Plan {
        argv: p.argv.clone(),
        env: p.env.clone(),
        working_dir: p.working_dir.clone(),
        fds: Fds::default(),
        // ⚠ Empty, and that is the same reason the launcher's is: `prepare`
        // printed the banner, because T-0412's steps run inside the rootfs
        // after it and every one of them has to be named before it runs.
        banner: String::new(),
        path_dirs: Plan::path_from(&p.env),
    };
    // ⭐ T-0204 and T-0211. The lock is handed to the payload immediately
    // before the fork that leads to its exec, and to nothing else.
    if let Err(e) = held.hand_to_payload() {
        let _ = writeln!(err, "podbox run: {e}");
        return podbox_image::error::EXIT_RUNTIME_ERROR;
    }
    let code = match podbox_enter::run(&root, &plan, &mut err) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(err, "podbox run: {e}");
            e.exit_code()
        }
    };
    drop(err);

    if p.rm {
        // ⚠ The lock is dropped first: `remove_extracted` deletes the tree the
        // lock exists to protect, and holding it while deleting would be podbox
        // refusing its own request.
        drop(held);
        if let Err(e) = podbox_extract::remove_extracted(&store, &p.record.manifest_digest) {
            eprintln!("podbox run: --rm could not remove the rootfs: {e}");
        }
    }
    code
}

/// Everything `run` and `create` both need: the image in the store, extracted,
/// its plan resolved and the rung selected, with nothing entered yet.
///
/// ⛔ ONE PATH for both verbs. `create` that resolved its own image would be a
/// second implementation of the hardest half of `run`, and the one nobody
/// exercises is the one that diverges.
pub(crate) fn prepare(
    args: &[String],
    store: &podbox_image::Store,
    verb: &str,
) -> std::result::Result<crate::lifecycle::Prepared, i32> {
    let o = parse(args)?;
    let image = o.image.clone().expect("checked in parse");

    let (platform, policy) = crate::lifecycle::platform_and_policy(
        verb,
        o.platform.as_deref(),
        &o.insecure,
        o.tls_verify,
    )?;

    // ------------------------------------------------------------- the image
    let record = acquire(store, &image, &platform, &policy, &o.pull)?;

    // ⛔ The lock BEFORE the extraction check, so a concurrent `rmi` cannot
    // delete the rootfs between podbox deciding it is there and entering it.
    // TODO/image.md T-0204. ⚠ Dropped at the end of this function: the caller
    // takes its own, for the process that actually enters.
    let held = store.hold(&record).map_err(|e| {
        eprintln!("podbox {verb}: {e}");
        podbox_image::error::EXIT_RUNTIME_ERROR
    })?;

    if !podbox_extract::is_extracted(store, &record.manifest_digest) {
        extract_now(store, &record)?;
    }
    let (rootfs, _) = podbox_extract::paths(store, &record.manifest_digest);
    let rootfs = rootfs.to_string_lossy().to_string();

    // ---------------------------------------------------- the foreign platform
    let host = Platform::host();
    let support = binfmt::support_for(&record.architecture, &host.arch);
    let mut err = std::io::stderr().lock();
    match &support {
        binfmt::Support::Native => {}
        binfmt::Support::Interpreter(r) => {
            // ⭐ The F flag: the kernel holds the interpreter open, so it works
            // inside the chroot with nothing copied in.
            let _ = writeln!(
                err,
                "podbox: this image is {}/{} and this machine is {}. Running it \
                 through the registered interpreter {} (binfmt flag F, so it is \
                 reachable inside the chroot)",
                record.os, record.architecture, host, r.interpreter
            );
        }
        binfmt::Support::InterpreterNeedsCopyIn(r) => {
            // ⛔ podbox is about to WRITE A FILE INTO SOMEBODY ELSE'S IMAGE.
            // T-0506: the honesty rules have no exception for a helpful edit,
            // and the payload can see this file.
            match binfmt::copy_interpreter_in(&rootfs, r) {
                Ok(dest) => {
                    // ⛔ podbox has written a file into somebody else's image.
                    // T-0506: the honesty rules have no exception for a helpful
                    // edit, and the payload can see that file.
                    let _ = writeln!(
                        err,
                        "podbox: this image is {}/{} and this machine is {}. Its \
                         interpreter {} is registered WITHOUT the binfmt F flag, so \
                         the kernel opens it by path at exec time and that path is \
                         inside the container. ⛔ podbox has COPIED it to {} inside \
                         the image's own rootfs; the payload can see that file.",
                        record.os,
                        record.architecture,
                        host,
                        r.interpreter,
                        dest.display()
                    );
                }
                Err(e) => {
                    let _ = writeln!(
                        err,
                        "podbox run: this image is {}/{} and this machine is {}. Its \
                         interpreter {} lacks the binfmt F flag and could not be \
                         copied into the rootfs: {e}",
                        record.os, record.architecture, host, r.interpreter
                    );
                    return Err(podbox_enter::EXIT_RUNTIME_ERROR);
                }
            }
        }
        binfmt::Support::None { why } => {
            // ⛔ Refused with everything a caller needs, never an `Exec format
            // error` from the kernel with nothing attached to it.
            let _ = writeln!(
                err,
                "podbox {verb}: {image} is {}/{} and this machine is {}. podbox \
                 cannot execute it: {why}",
                record.os, record.architecture, host
            );
            return Err(podbox_enter::EXIT_RUNTIME_ERROR);
        }
    }

    // --------------------------------------------------------------- the plan
    let cfg = config_of(store, &record)?;
    let argv = Plan::argv_for(
        cfg.config.entrypoint.as_deref(),
        cfg.config.cmd.as_deref(),
        &o.command,
        o.entrypoint.as_deref(),
    );
    if argv.is_empty() {
        eprintln!(
            "podbox {verb}: {image} declares neither an Entrypoint nor a Cmd, and no \
             command was given. There is nothing to run"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    let env = Plan::env_for(&cfg.config.env, &o.env);
    let path_dirs = Plan::path_from(&env);
    let working_dir = o
        .workdir
        .clone()
        .unwrap_or_else(|| cfg.config.working_dir.clone());

    // ------------------------------------------------------------- the banner
    // ⭐ TODO/probe.md T-0107 and T-0108's remaining halves: the rung is
    // selected here, for a real entry, and the banner names it and what it must
    // never claim.
    let findings = podbox_probe::run();
    let selection = podbox_probe::select::Selection::choose(&findings);
    // ⭐ T-0804 rule 4. The banner is built from the rung podbox ENTERS with,
    // not the one the machine would permit: `podbox_enter::ENTERED_RUNG` is the
    // sequence that crate implements, and it is one constant so the banner, the
    // container record and `--strict` cannot disagree.
    let entered = podbox_enter::ENTERED_RUNG;
    let mut banner = podbox_probe::report::entry_banner(&findings, &selection, entered);
    // ⭐ TODO/cli.md T-0803. Where podbox was reached under somebody else's
    // name, the banner says which name was used and that this is podbox.
    // Taking the name is the product requirement; taking it silently is what
    // TOOL.md section 4.1 forbids.
    if let Some(note) = crate::names::alias_note() {
        banner.push_str(&note);
    }
    // ⭐ M5. The completion layer runs HERE: after the rootfs exists and the
    // image lock is held, and before anything is entered. Its report is part of
    // the banner, because every one of these is an edit podbox made inside
    // somebody else's image and the payload can see it.
    let mut ask = o.ask.clone();
    ask.container_name = o.name.clone();
    let mut completion = crate::complete::prepare(verb, &rootfs, &ask, &mut banner)?;
    if !cfg.config.user.is_empty() {
        // ⚠ Read and REPORTED, never applied. podbox cannot setuid to an id
        // this machine does not map, which is the wall the project is about.
        banner.push_str(&format!(
            "podbox: the image asks to run as user {:?}; podbox runs as uid 0 \
             and does not change to it\n",
            cfg.config.user
        ));
    }
    // ⭐ T-0804 rule 1, applied HERE and nowhere else, so `run`, `run -d`,
    // `create` and the launcher cannot disagree about whether this machine
    // prints it. ⛔ It moved ahead of every refusal below on purpose: a refused
    // caller used to get the reasons with no banner under `--strict`, and
    // `create` printed neither, while T-0412's steps mean podbox may now run a
    // command inside the image, which rule 3 says is named before it runs.
    let quiet = crate::complete::banner_quiet(store);
    if !quiet {
        let _ = write!(err, "{banner}");
    }
    if o.tty {
        // ⛔ T-0503: refused BY NAME rather than silently degraded.
        // ⛔ `Ok` and nothing else. `Skip` means the row never ran, and
        // TODO/probe.md T-0109 rule 1 is that a skip may not read as either a
        // denial or a pass: a pty podbox could not measure is not one it may
        // promise.
        let usable = findings.rows.iter().any(|(n, out)| {
            n.starts_with("open(/dev/ptmx")
                && matches!(out.verdict, podbox_probe::verdict::Verdict::Ok)
        });
        if !usable {
            let _ = writeln!(
                err,
                "podbox run: -t was asked for and /dev/ptmx is not usable on this \
                 machine, so podbox cannot allocate a pty. It refuses rather than \
                 running without one and letting the payload discover it \
                 (TODO/enter.md T-0503)"
            );
            return Err(podbox_enter::EXIT_RUNTIME_ERROR);
        }
    }
    // ⭐ T-0804, and it is the LAST thing before the run is committed to, so a
    // refused caller still gets the whole account of why on stderr. ⛔ The
    // refusal prints even where the banner is suppressed: the config switch
    // silences a notice, never a refusal.
    crate::complete::strict_refusal(verb, &ask, entered.word(), &completion, &mut err)?;
    // ⭐ T-0412. The commands the completion layer could not run from the host,
    // run here: after the banner named them, after `--strict` had its chance to
    // refuse them, and before anything of the payload's exists.
    crate::complete::run_steps(verb, &rootfs, &env, &mut completion, quiet, &mut err);
    let _ = path_dirs;
    // ⚠ The lock this function took goes here. It existed to keep the rootfs
    // from being deleted between the extraction check and now; the process that
    // ENTERS takes its own, and for `-d` that is the launcher rather than this.
    drop(held);
    Ok(crate::lifecycle::Prepared {
        record,
        image,
        rootfs,
        argv,
        env,
        working_dir,
        name: o.name.clone(),
        rung: entered.word().to_string(),
        detach: o.detach,
        rm: o.rm,
        completion: completion
            .fixups
            .iter()
            .filter(|f| f.action.changed() || f.action == podbox_complete::Action::Failed)
            .map(|f| {
                format!(
                    "{} {} ({}, {})",
                    f.action.word(),
                    if f.path.is_empty() { "-" } else { &f.path },
                    f.id,
                    f.entry
                )
            })
            .collect(),
        completion_degraded: completion.degradations().len(),
    })
}

/// Make sure the store holds the image, honouring `--pull`.
fn acquire(
    store: &podbox_image::Store,
    image: &str,
    platform: &Platform,
    policy: &Policy,
    pull: &str,
) -> std::result::Result<podbox_image::Record, i32> {
    let found = store.find_for(image, Some(platform)).unwrap_or_default();
    let others = found.other_platforms.clone();
    let have = found.one();
    match (pull, have) {
        ("never", Some(r)) => Ok(r),
        ("never", None) => {
            // ⛔ Two different sentences for two different situations. "held,
            // for another platform" sends the caller to --platform; "not held"
            // sends them to a pull. Collapsing them into "no such image" is the
            // message a caller cannot act on.
            if others.is_empty() {
                eprintln!(
                    "podbox run: {image} is not in the store and --pull never was \
                     given, so podbox will not fetch it"
                );
            } else {
                eprintln!(
                    "podbox run: the store holds {image} for {}, and {platform} was \
                     asked for. --pull never was given, so podbox will not fetch \
                     the one that was asked for",
                    others.join(", ")
                );
            }
            Err(podbox_image::error::EXIT_RUNTIME_ERROR)
        }
        ("missing", Some(r)) => Ok(r),
        _ => {
            // ⛔ The transcript goes to STDERR here, unlike `podbox pull` where
            // it is the output. The payload owns stdout.
            let mut err = std::io::stderr().lock();
            match podbox_image::pull::pull(store, image, platform, policy, &mut err) {
                Ok(done) => Ok(done.record),
                Err(e) => {
                    let _ = writeln!(err, "podbox run: {e}");
                    Err(e.exit_code())
                }
            }
        }
    }
}

fn extract_now(
    store: &podbox_image::Store,
    record: &podbox_image::Record,
) -> std::result::Result<(), i32> {
    let d = podbox_image::digest::Digest::parse(&record.manifest_digest).map_err(|e| {
        eprintln!("podbox run: {e}");
        e.exit_code()
    })?;
    let bytes = store.read_blob(&d).map_err(|e| {
        eprintln!("podbox run: {e}");
        e.exit_code()
    })?;
    let manifest: podbox_image::oci::Manifest = serde_json::from_slice(&bytes).map_err(|e| {
        eprintln!("podbox run: the manifest does not parse: {e}");
        podbox_image::error::EXIT_RUNTIME_ERROR
    })?;
    let mut err = std::io::stderr().lock();
    podbox_extract::extract(store, &manifest, &record.manifest_digest, &mut err).map_err(|e| {
        let _ = writeln!(err, "podbox run: {e}");
        podbox_image::error::EXIT_RUNTIME_ERROR
    })?;
    Ok(())
}

/// The image's config blob.
///
/// ⚠ `pub(crate)` because `exec` reads the same blob for the same reason, and a
/// second copy of it is the copy that drifts: `docs/conventions/code.md`.
pub(crate) fn config_of(
    store: &podbox_image::Store,
    record: &podbox_image::Record,
) -> std::result::Result<podbox_image::oci::Config, i32> {
    let d = podbox_image::digest::Digest::parse(&record.config_digest).map_err(|e| {
        eprintln!("podbox run: {e}");
        e.exit_code()
    })?;
    let bytes = store.read_blob(&d).map_err(|e| {
        eprintln!("podbox run: {e}");
        e.exit_code()
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        eprintln!("podbox run: the image config does not parse: {e}");
        podbox_image::error::EXIT_RUNTIME_ERROR
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_string()).collect()
    }

    /// ⛔ The rule a caller cannot work around if podbox gets it wrong.
    #[test]
    fn parsing_stops_at_the_image_name() {
        let o = parse(&v(&["--rm", "alpine", "ls", "-l", "--rm"])).unwrap();
        assert!(o.rm);
        assert_eq!(o.image.as_deref(), Some("alpine"));
        // ⛔ `-l` and the SECOND `--rm` belong to `ls`, not to podbox.
        assert_eq!(o.command, v(&["ls", "-l", "--rm"]));
    }

    #[test]
    fn a_flag_needing_a_value_does_not_swallow_the_image() {
        assert_eq!(parse(&v(&["--platform"])).unwrap_err(), EXIT_FLAG_ERROR);
        assert_eq!(parse(&v(&["-e"])).unwrap_err(), EXIT_FLAG_ERROR);
    }

    #[test]
    fn both_spellings_of_every_valued_flag_reach_the_same_place() {
        let a = parse(&v(&["-e", "A=1", "-w", "/w", "--entrypoint", "/e", "img"])).unwrap();
        let b = parse(&v(&["--env=A=1", "--workdir=/w", "--entrypoint=/e", "img"])).unwrap();
        assert_eq!(a.env, b.env);
        assert_eq!(a.workdir, b.workdir);
        assert_eq!(a.entrypoint, b.entrypoint);
        assert_eq!(a.image, b.image);
    }

    #[test]
    fn an_unknown_pull_mode_is_invalid_input_and_not_a_silent_default() {
        assert_eq!(
            parse(&v(&["--pull", "sometimes", "img"])).unwrap_err(),
            EXIT_FLAG_ERROR
        );
        assert_eq!(
            parse(&v(&["--pull", "always", "img"])).unwrap().pull,
            "always"
        );
        assert_eq!(parse(&v(&["img"])).unwrap().pull, "missing");
    }

    /// ⭐ TODO/cli.md T-0801. Every flag the table ADMITS for this verb reaches
    /// an arm of this parser. The other direction is held by construction: an
    /// argument starting with `-` goes through `parity::admit` first, so an arm
    /// for a flag with no row is unreachable rather than a second surface.
    #[test]
    fn every_flag_the_table_admits_is_handled_by_this_parser() {
        for r in crate::parity::TABLE
            .iter()
            .filter(|r| r.verb == "run" && r.flag.is_some())
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
                // ⚠ THE ASSERTION IS `parity::no_arm`'s PANIC, not a code
                // comparison. It used to compare against EXIT_RUNTIME_ERROR,
                // which was the fallback arm's own code and distinguishable
                // from a legitimate refusal's 2; T-0802 measured docker and
                // made a flag error 125 as well, so that comparison started
                // asserting `125 != 125` and could no longer fail. Both shapes
                // are tried because a valued flag needs a value, a boolean one
                // does not, and one with a closed set of values (`--pull`)
                // refuses any value this test could invent: all three are
                // legitimate, and only reaching the fallback arm is not.
                for shape in [v(&[f, "V", "img", "true"]), v(&[f, "img", "true"])] {
                    let _ = parse(&shape);
                }
            }
        }
    }

    #[test]
    /// ⚠ docker's own code for a required argument that is not there is 1 and
    /// not 125: measured, `docker run` with no image exits 1. T-0802.
    fn no_image_is_a_cli_error() {
        assert_eq!(parse(&v(&["--rm"])).unwrap_err(), EXIT_CLI_ERROR);
    }
}
