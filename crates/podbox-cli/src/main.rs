//! TOOL.md section 6.8: the docker and podman argument surface, the parity table, exit codes.
//!
//! Milestones M0 ([`TODO/milestones.md`](../../../TODO/milestones.md) T-1101)
//! and M1 (T-1102) implement `probe`, `pull`, `images`, `rmi`, `tag`,
//! `image prune` and `inspect`. Every other verb is unimplemented and says so
//! on stderr with exit 125, which is docker's code for "the runtime could not
//! run the command". The work is in
//! [`TODO/cli.md`](../../../TODO/cli.md).
//!
//! ⚠ The full verb and flag parity table is [`TODO/cli.md`](../../../TODO/cli.md)
//! T-0801 and docker's own exit codes are T-0802, both of which land with M3.
//! Until then this file's contract is the one
//! [`TODO/probe.md`](../../../TODO/probe.md) T-0110 settled: invalid input
//! exits 2, and a runtime failure exits 125.
#![forbid(unsafe_op_in_unsafe_fn)]

mod exec;
mod format;
mod images;
mod lifecycle;
mod names;
mod parity;
mod run;
mod system;

use std::io::Write;

use podbox_probe::report;
use podbox_probe::select::{Selection, Strictness};

/// docker's exit code for a daemon-side failure to run the command.
const EXIT_RUNTIME_ERROR: i32 = 125;
/// `pathshim`'s contract, adopted at
/// `references/compforge__pathshim/tree/README.md:65`: invalid input exits 2.
const EXIT_USAGE: i32 = 2;

const USAGE: &str = "\
usage: podbox <command> [options]

  run          extract an image if needed and run a command inside it
  exec         run a command in an already extracted image. ⛔ A FRESH CHROOT,
               sharing only the filesystem, never a namespace entry
  probe        report what this machine permits, and the rung podbox selects
  pull         fetch an image into the content-addressed store
  extract      unpack a pulled image's layers into a rootfs
  images       list what the store holds
  rmi          remove images, and every blob no image reaches
  tag          point a second name at one manifest digest
  image        ls | rm | prune | tag
  inspect      print one image's record
  create       write a container record, and start nothing
  start        start a created container, and return when its payload is running
  ps           list containers, from podbox's own table and never from /proc
  logs         a container's captured output
  stop         SIGTERM, then SIGKILL after a bounded grace
  kill         send one signal to a container's payload
  wait         block until a container ends, and print its exit code
  rm           remove a container
  cp           copy one file into or out of a container's rootfs
  system info  what this podbox is, and the verb and flag parity table AS DATA
  system install-names
               install `docker` and `podman` as symlinks to this binary
  version      print the version

  Every other docker verb is named in that table with the reason podbox does
  not have it, and says so with exit 125 rather than being silently ignored.
";

const PROBE_USAGE: &str = "\
usage: podbox probe [--json | --rows] [--strict] [--cached]

  (no flag)   print the selected rung on stdout and the evidence on stderr
  --json      print the findings as one JSON document on stdout
  --rows      print every probe row on stdout, in the format
              verification/probe emits, so two runs diff line by line
  --strict    exit non-zero when the selected rung is below `namespace`, or
              when a control of the bogus-argument discriminator could not
              answer. A caller gates on this without parsing anything.
  --cached    the hot path: serve $store/probe.json when its key still holds,
              and measure and rewrite it when it does not. Without this flag
              podbox probe measures and touches no store, because a diagnostic
              that can answer from a file is not a diagnostic.

  --probe-child <name>
              internal. Runs one named probe in this process and reports it on
              stdout and in the exit status. podbox re-executes itself with
              this for every probe, because a successful unshare, chroot or
              setuid mutates the prober.
";

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().collect();

    // ⛔ First, before the ordinary argument surface. A probe child must never
    // fall through into a verb, and nothing here may print before it does.
    if argv.get(1).map(String::as_str) == Some(podbox_probe::CHILD_FLAG) {
        let code = match argv.get(2) {
            Some(name) => podbox_probe::run_child(name),
            None => {
                eprintln!("podbox: {} needs a probe name", podbox_probe::CHILD_FLAG);
                EXIT_USAGE
            }
        };
        return exit(code);
    }

    let argv0 = argv.first().cloned().unwrap_or_else(|| "podbox".into());
    let rest = if argv.len() > 2 { &argv[2..] } else { &[] };
    match argv.get(1).map(String::as_str) {
        Some("run") => exit(run::run(rest)),
        Some("exec") => exit(exec::exec(rest)),
        Some("cp") => exit(lifecycle::cp(rest)),
        Some("rm") => exit(lifecycle::rm(rest)),
        Some("wait") => exit(lifecycle::wait(rest)),
        Some("kill") => exit(lifecycle::kill(rest)),
        Some("stop") => exit(lifecycle::stop(rest)),
        Some("logs") => exit(lifecycle::logs(rest)),
        Some("ps") => exit(lifecycle::ps(rest)),
        Some("start") => exit(lifecycle::start(rest)),
        Some("create") => exit(lifecycle::create(rest)),
        Some("probe") => exit(probe(rest)),
        Some("pull") => exit(images::pull(rest)),
        Some("extract") => exit(images::extract(rest)),
        Some("images") => exit(images::images(rest)),
        Some("rmi") => exit(images::rmi(rest)),
        Some("tag") => exit(images::tag(rest)),
        Some("inspect") => exit(images::inspect(rest)),
        Some("image") => exit(image_group(rest)),
        Some("system") => exit(system::system(rest)),
        Some("info") => exit(system::info(rest)),
        Some("version") | Some("--version") | Some("-v") => {
            println!("podbox {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some("-h") | Some("--help") | Some("help") => {
            print!("{USAGE}");
            std::process::ExitCode::SUCCESS
        }
        other => {
            let mut err = std::io::stderr().lock();
            let name = other.unwrap_or("");
            // ⛔ TODO/cli.md T-0801. The parity table answers, so a docker verb
            // podbox does not have gets the REASON it does not, from the one
            // place that records it, rather than a milestone number that ages.
            match parity::verb(name) {
                Some(row) => {
                    let _ = writeln!(err, "podbox: {name}: {}", row.note);
                    let _ = writeln!(
                        err,
                        "podbox: `podbox system info --format '{{{{json .Parity}}}}'` is \
                         every verb and flag podbox has and every one it does not"
                    );
                }
                None => {
                    let _ = writeln!(
                        err,
                        "podbox: {}: podbox has no such command, and neither does the \
                         parity table (TOOL.md section 6.8)",
                        if name.is_empty() {
                            "no command given"
                        } else {
                            name
                        }
                    );
                    let _ = write!(err, "{USAGE}");
                }
            }
            let _ = writeln!(err, "podbox: invoked as {argv0}");
            exit(EXIT_RUNTIME_ERROR)
        }
    }
}

/// docker's `image` sub-command group.
fn image_group(args: &[String]) -> i32 {
    let rest = if args.len() > 1 { &args[1..] } else { &[] };
    match args.first().map(String::as_str) {
        Some("ls") | Some("list") => images::images(rest),
        Some("rm") | Some("remove") => images::rmi(rest),
        Some("prune") => images::prune(rest),
        Some("tag") => images::tag(rest),
        Some("inspect") => images::inspect(rest),
        Some("pull") => images::pull(rest),
        Some("extract") => images::extract(rest),
        Some("-h") | Some("--help") | None => {
            println!("usage: podbox image ls | rm | prune | tag | inspect | pull");
            0
        }
        Some(other) => {
            eprintln!("podbox image: {other:?}: not implemented yet");
            EXIT_RUNTIME_ERROR
        }
    }
}

fn probe(args: &[String]) -> i32 {
    let mut json = false;
    let mut rows = false;
    let mut cached = false;
    let mut strictness = Strictness::Warn;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "--rows" => rows = true,
            "--strict" => strictness = Strictness::Refuse,
            "--cached" => cached = true,
            "-h" | "--help" => {
                print!("{PROBE_USAGE}");
                return 0;
            }
            other => {
                eprintln!("podbox probe: unknown option {other:?}");
                eprint!("{PROBE_USAGE}");
                return EXIT_USAGE;
            }
        }
    }
    if json && rows {
        eprintln!("podbox probe: --json and --rows both write stdout; pick one");
        return EXIT_USAGE;
    }

    // ⭐ TODO/probe.md T-0111. `--cached` is the hot path `run` and `exec` take
    // at M3; `--rows` is a per-probe rendering the cached document does not
    // carry, so the two are refused together rather than one silently winning.
    if cached && rows {
        eprintln!(
            "podbox probe: --cached serves the stored --json document, which \
             carries no per-probe rows; pick one"
        );
        return EXIT_USAGE;
    }
    if cached {
        return cached_probe(json, strictness);
    }

    let findings = podbox_probe::run();
    let selection = Selection::choose(&findings);

    // ⛔ The rung goes to stdout, the evidence to stderr. A payload's stdout is
    // data to whatever consumes it.
    let mut err = std::io::stderr().lock();
    if json {
        print!("{}", report::document(&findings, &selection));
    } else if rows {
        print!("{}", report::rows(&findings));
    } else {
        println!("{}", selection.rung.word());
        let _ = write!(err, "{}", report::evidence(&findings, &selection));
    }

    let code = selection.exit_code(strictness);
    if code != 0 {
        let _ = writeln!(
            err,
            "podbox probe: --strict, and this machine is {} rather than {}{}",
            selection.rung.word(),
            Selection::STRICT_FLOOR.word(),
            if selection.controls_answered {
                ""
            } else {
                ", and a control of the bogus-argument discriminator could not answer"
            }
        );
    }
    code
}

/// `podbox probe --cached`.
///
/// ⛔ Where the cache is served, the evidence says so on stderr. A rung read
/// from a file and a rung measured now are different sentences, and printing
/// the first as though it were the second is the failure T-0111 exists to stop.
fn cached_probe(json: bool, strictness: Strictness) -> i32 {
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("podbox probe: {e}");
            return e.exit_code();
        }
    };
    let probe = podbox_image::probe_cache::resolve(&store);
    let mut err = std::io::stderr().lock();
    match &probe.source {
        podbox_image::probe_cache::Source::Cache => {
            let _ = writeln!(
                err,
                "podbox probe: served from {}, whose key still matches this \
                 machine's confinement",
                probe.path.display()
            );
        }
        podbox_image::probe_cache::Source::Measured(why) => {
            let _ = writeln!(err, "podbox probe: measured now, because {why}");
        }
    }
    if let Some(why) = &probe.not_written {
        let _ = writeln!(err, "podbox probe: ⚠ the answer was not cached: {why}");
    }

    let Some(rung) = probe.rung() else {
        let _ = writeln!(
            err,
            "podbox probe: the document at {} carries no rung",
            probe.path.display()
        );
        return EXIT_RUNTIME_ERROR;
    };
    if json {
        print!("{}", probe.document);
    } else {
        println!("{rung}");
    }

    // ⚠ `--strict` on the cached path reads `strict_ok` out of the document
    // rather than re-deriving it: `Selection` is not reconstructed from JSON,
    // and a second implementation of the rung rules is exactly the copy-paste
    // docs/conventions/code.md forbids.
    if matches!(strictness, Strictness::Refuse) {
        let ok = probe.document.contains("\"strict_ok\":true");
        if !ok {
            let _ = writeln!(
                err,
                "podbox probe: --strict, and this machine is {rung} rather than {}",
                Selection::STRICT_FLOOR.word()
            );
            return 1;
        }
    }
    0
}

fn exit(code: i32) -> std::process::ExitCode {
    // ⚠ An exit status carries eight bits. Anything wider would be truncated
    // into a different code, so it is clamped to one that cannot be confused
    // with a verdict.
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(EXIT_RUNTIME_ERROR as u8))
}
