//! TOOL.md §6.8: the docker and podman argument surface, the parity table, exit codes.
//!
//! Skeleton. Every verb is unimplemented and says so on stderr with exit 125,
//! which is docker's code for "the runtime could not run the command". The
//! work is in `TODO/cli.md`.
#![forbid(unsafe_op_in_unsafe_fn)]

use std::io::Write;

/// docker's exit code for a daemon-side failure to run the command.
const EXIT_RUNTIME_ERROR: i32 = 125;

fn main() -> std::process::ExitCode {
    let argv0 = std::env::args().next().unwrap_or_else(|| "podbox".into());
    let verb = std::env::args().nth(1);
    let mut err = std::io::stderr().lock();
    match verb.as_deref() {
        Some("version") | Some("--version") | Some("-v") => {
            println!("podbox {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        other => {
            let _ = writeln!(
                err,
                "podbox: {}: not implemented yet (skeleton; TOOL.md §5 milestone M-1)",
                other.unwrap_or("no command given")
            );
            let _ = writeln!(err, "podbox: invoked as {argv0}");
            std::process::ExitCode::from(EXIT_RUNTIME_ERROR as u8)
        }
    }
}
