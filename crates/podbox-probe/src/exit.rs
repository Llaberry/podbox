//! ⭐ **docker's exit codes, in one file, measured rather than read.**
//!
//! [`TODO/cli.md`](../../../TODO/cli.md) T-0802: an automated caller reads the
//! exit code before it reads anything else, so parity here is worth more than a
//! scheme podbox thinks is clearer. A runtime that returns its own codes breaks
//! every script that branches on docker's.
//!
//! ⛔ **This module is the only place these numbers are written.** They were in
//! three files before T-0802 -- `podbox-image::error`, `podbox-cli::main` and
//! `podbox-enter` -- and the copies had already diverged: the usage code was
//! corrected against a measurement in one of them and stayed at the old value
//! in another, so two verbs of one binary disagreed about what a flag error is.
//! Check 20 of `scripts/check-todo.py` holds this file as the only declaration.
//!
//! ⭐ **The discriminator between 125 and 1 is not obvious and it is the whole
//! finding.** Measured against docker 29.3.1 by
//! `experiments/330-exit-codes.sh`, which runs both binaries on the same host:
//!
//! | what | docker | why |
//! | --- | --- | --- |
//! | `run --badflag`, `--pull=bogus`, `--memory=notasize` | **125** | the FLAG PARSER refused it |
//! | `images --format '{{.Nope}}'`, `rmi no-such-image`, `run` with no image | **1** | the verb refused it after the flags parsed |
//! | `run` could not run the command at all | **125** | |
//! | the command was found and is not invocable | **126** | |
//! | the command was not found | **127** | |
//! | anything else | the payload's own status, and `128+N` for a signal | |
//! | the bare name with no arguments | **0**, with the help on stdout | |
//! | a verb neither tool has | **1** | |

/// docker's code for "the runtime could not run the command", and for anything
/// a verb's **flag parser** refused. ⚠ One number for two meanings because
/// docker uses one number for both, not because they are the same thing;
/// [`EXIT_FLAG_ERROR`] is the other name so a call site says which it is.
pub const EXIT_RUNTIME_ERROR: i32 = 125;

/// A flag error: an unknown option, a missing value, a value the parser will
/// not take, or a flag the parity table refuses.
pub const EXIT_FLAG_ERROR: i32 = 125;

/// Everything the CLI refuses **after** the flags parsed: an invalid reference,
/// a `--format` template no verb can answer, a name that resolves to nothing, a
/// required argument that is not there, a verb neither tool has.
pub const EXIT_CLI_ERROR: i32 = 1;

/// The command was found and could not be invoked.
pub const EXIT_CANNOT_INVOKE: i32 = 126;

/// The command was not found.
pub const EXIT_NOT_FOUND: i32 = 127;

/// ⭐ **THE TABLE, AS DATA**, for the same reason the verb and flag parity
/// table is: a caller decides from it, and a measurement asserts against it
/// rather than against a number somebody typed into a shell script.
///
/// `podbox system info --format '{{json .ExitCodes}}'` prints this, and
/// `experiments/330-exit-codes.sh` drives both podbox and docker through every
/// case below and asserts they agree.
///
/// ⛔ Two clauses of `experiments/320-cli-contract.sh` had `[ "$rc" -eq 2 ]`
/// written into them and went red the day T-0802 measured docker and moved the
/// number. They read it from here now.
pub const CASES: &[(&str, i32, &str)] = &[
    (
        "flag-error",
        EXIT_FLAG_ERROR,
        "a verb's flag parser refused: an unknown option, a missing value, a value \
         it will not take, or a flag the parity table refuses",
    ),
    (
        "cli-error",
        EXIT_CLI_ERROR,
        "the verb refused after the flags parsed: an invalid reference, a template \
         no verb can answer, a name that resolves to nothing, a missing argument, \
         or a verb neither podbox nor docker has",
    ),
    (
        "runtime-error",
        EXIT_RUNTIME_ERROR,
        "podbox could not run the command at all",
    ),
    (
        "cannot-invoke",
        EXIT_CANNOT_INVOKE,
        "the command was found and could not be invoked",
    ),
    ("not-found", EXIT_NOT_FOUND, "the command was not found"),
    (
        "no-arguments",
        0,
        "the bare name with no arguments prints the help and succeeds",
    ),
];

/// One case's code, or `None` where nothing names it.
pub fn code(case: &str) -> Option<i32> {
    CASES.iter().find(|(k, ..)| *k == case).map(|(_, c, _)| *c)
}

/// The table as one JSON document.
pub fn json() -> String {
    let mut s = String::from("[");
    for (i, (case, code, what)) in CASES.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // ⚠ Hand-serialised, and the only escaping that matters is the prose's.
        let what = what.replace('\\', "\\\\").replace('"', "\\\"");
        s.push_str(&format!(
            r#"{{"case":"{case}","code":{code},"what":"{what}"}}"#
        ));
    }
    s.push(']');
    s
}

/// The status a signalled payload reports, which is a shell's convention and
/// docker's. ⚠ Measured: `docker kill` on a running container makes `docker
/// wait` print 137, which is `128 + SIGKILL`.
pub fn from_signal(sig: i32) -> i32 {
    128 + sig
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ 137 is the number `experiments/330-exit-codes.sh` reads out of
    /// `docker wait` after a `docker kill`, and it is the one podbox has to
    /// produce for the same event.
    #[test]
    fn a_sigkilled_payload_is_137() {
        assert_eq!(from_signal(9), 137);
        assert_eq!(from_signal(15), 143);
    }

    /// ⛔ Every case is in the table, so a caller and a measurement read the
    /// same list. A constant with no row is one nobody can assert against.
    #[test]
    fn every_code_has_a_row_and_every_row_parses() {
        for c in [
            EXIT_FLAG_ERROR,
            EXIT_CLI_ERROR,
            EXIT_RUNTIME_ERROR,
            EXIT_CANNOT_INVOKE,
            EXIT_NOT_FOUND,
        ] {
            assert!(
                CASES.iter().any(|(_, code, _)| *code == c),
                "{c} has no row"
            );
        }
        assert_eq!(code("flag-error"), Some(125));
        assert_eq!(code("cli-error"), Some(1));
        assert_eq!(code("nope"), None);
        let doc = json();
        assert!(doc.starts_with('['), "{doc}");
        assert!(doc.contains(r#""case":"not-found","code":127"#), "{doc}");
    }

    /// ⚠ An assertion that the two names for 125 have not been given different
    /// values by an edit that looked local.
    #[test]
    fn the_two_names_for_125_are_one_number() {
        assert_eq!(EXIT_RUNTIME_ERROR, EXIT_FLAG_ERROR);
        assert_ne!(EXIT_CLI_ERROR, EXIT_FLAG_ERROR);
    }
}
