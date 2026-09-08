//! TOOL.md section 6.8: the docker and podman argument surface, the parity table, exit codes.
//!
//! Milestone M0 ([`TODO/milestones.md`](../../../TODO/milestones.md) T-1101)
//! implements `probe` and nothing else. Every other verb is unimplemented and
//! says so on stderr with exit 125, which is docker's code for "the runtime
//! could not run the command". The work is in
//! [`TODO/cli.md`](../../../TODO/cli.md).
#![forbid(unsafe_op_in_unsafe_fn)]

use std::io::Write;

use podbox_probe::report;
use podbox_probe::select::{Selection, Strictness};

/// docker's exit code for a daemon-side failure to run the command.
const EXIT_RUNTIME_ERROR: i32 = 125;
/// `pathshim`'s contract, adopted at
/// `references/compforge__pathshim/tree/README.md:65`: invalid input exits 2.
const EXIT_USAGE: i32 = 2;

const PROBE_USAGE: &str = "\
usage: podbox probe [--json | --rows] [--strict]

  (no flag)   print the selected rung on stdout and the evidence on stderr
  --json      print the findings as one JSON document on stdout
  --rows      print every probe row on stdout, in the format
              verification/probe emits, so two runs diff line by line
  --strict    exit non-zero when the selected rung is below `namespace`, or
              when a control of the bogus-argument discriminator could not
              answer. A caller gates on this without parsing anything.

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
    match argv.get(1).map(String::as_str) {
        Some("probe") => exit(probe(&argv[2..])),
        Some("version") | Some("--version") | Some("-v") => {
            println!("podbox {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        other => {
            let mut err = std::io::stderr().lock();
            let _ = writeln!(
                err,
                "podbox: {}: not implemented yet (milestone M0 implements `probe` \
                 and nothing else; TOOL.md section 5)",
                other.unwrap_or("no command given")
            );
            let _ = writeln!(err, "podbox: invoked as {argv0}");
            exit(EXIT_RUNTIME_ERROR)
        }
    }
}

fn probe(args: &[String]) -> i32 {
    let mut json = false;
    let mut rows = false;
    let mut strictness = Strictness::Warn;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "--rows" => rows = true,
            "--strict" => strictness = Strictness::Refuse,
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

fn exit(code: i32) -> std::process::ExitCode {
    // ⚠ An exit status carries eight bits. Anything wider would be truncated
    // into a different code, so it is clamped to one that cannot be confused
    // with a verdict.
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(EXIT_RUNTIME_ERROR as u8))
}
