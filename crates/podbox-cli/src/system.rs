//! `podbox system info`, and `podbox info` as docker spells it.
//!
//! [`TODO/cli.md`](../../../TODO/cli.md) T-0801.
//!
//! ⭐ **This verb exists so the parity table is reachable from a program.** An
//! agent that knows docker needs to be able to ask what this podbox will and
//! will not do BEFORE it runs anything, and the answer has to be data:
//!
//! ```sh
//! podbox system info --format '{{json .Parity}}' | jq -r '.[] | select(.status=="None") | .verb'
//! ```
//!
//! ⛔ **podbox has no daemon**, so docker's server half of `info` is not
//! withheld and not faked: what stands in its place is the rung this machine
//! permits, measured, and what that rung does not provide.

use crate::{format, parity};
use podbox_image::error::EXIT_USAGE;

pub const SYSTEM_USAGE: &str = "\
usage: podbox system info [--format T]
       podbox system install-names [--dir D] [--force]
       podbox info [--format T]

  Fields: .Parity .ParityRows .Version .Rung .StrictOk .Store .Platform
          .Kernel .Verbs .VerbsNative .VerbsDegraded .VerbsStub .VerbsNone

  --format T   {{.Field}} placeholders, plus `{{json .Field}}`. .Parity is
               already a JSON document, so both spellings print it.

  ⛔ podbox has no daemon, so there is no client/server split to report. The
    rung this machine permits stands where docker prints a server version,
    and it is measured rather than assumed.

  ⚠ .Parity is the verb and flag parity table (TOOL.md section 6.8) as data:
    one object per row, with `verb`, `flag`, `status` and `note`. `status` is
    one of Native, Degraded, Stub, None and there is no fifth.
";

/// Fields whose value is already a JSON document, so `{{json .X}}` prints it
/// rather than quoting it. ⚠ A subset of [`FIELDS`], never a second list.
const DOCUMENTS: &[&str] = &["Parity"];

pub const FIELDS: &[&str] = &[
    "Parity",
    "ParityRows",
    "Version",
    "Rung",
    "StrictOk",
    "Store",
    "Platform",
    "Kernel",
    "Verbs",
    "VerbsNative",
    "VerbsDegraded",
    "VerbsStub",
    "VerbsNone",
];

/// `podbox system <sub>`.
pub fn system(args: &[String]) -> i32 {
    let rest = if args.len() > 1 { &args[1..] } else { &[] };
    match args.first().map(String::as_str) {
        Some("info") => info(rest),
        Some("install-names") => crate::names::install(rest),
        Some("-h") | Some("--help") | None => {
            print!("{SYSTEM_USAGE}");
            0
        }
        Some(other) => {
            // ⛔ The table answers, not this match arm. `podbox system df` gets
            // the reason `system` is Degraded rather than a bare "no".
            eprintln!(
                "podbox system: {other:?} is not implemented. `podbox system info` is, \
                 and `podbox system info --format '{{{{json .Parity}}}}'` lists every \
                 verb podbox has and every one it does not"
            );
            podbox_image::error::EXIT_RUNTIME_ERROR
        }
    }
}

pub fn info(args: &[String]) -> i32 {
    let mut template: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{SYSTEM_USAGE}");
                return 0;
            }
            "-f" | "--format" => match it.next() {
                Some(t) => template = Some(t.clone()),
                None => {
                    eprintln!("podbox system info: --format needs a template");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with("--format=") => {
                template = Some(other["--format=".len()..].to_string())
            }
            other => {
                eprintln!("podbox system info: unknown option {other:?}");
                eprint!("{SYSTEM_USAGE}");
                return EXIT_USAGE;
            }
        }
    }
    // ⛔ The caller's own input is checked before anything about the machine is
    // measured, so a typo is not paid for with a probe.
    if let Some(t) = &template {
        if let Err(bad) = format::check_with_documents(t, FIELDS, DOCUMENTS) {
            eprintln!("podbox system info: {bad}");
            return EXIT_USAGE;
        }
    }

    let fields = collect();
    match template {
        Some(t) => match format::render_with_documents(&t, &fields, DOCUMENTS) {
            Ok(line) => {
                println!("{line}");
                0
            }
            Err(bad) => {
                eprintln!("podbox system info: {bad}");
                EXIT_USAGE
            }
        },
        None => {
            print!("{}", human(&fields));
            0
        }
    }
}

/// Everything `info` can report, measured once.
///
/// ⚠ The measuring is HERE and the assembling is in [`fields_from`], so a test
/// can assert the field names without running 50 probe children of whatever
/// binary it happens to be. `podbox_probe::run` re-execs argv[0], and under
/// `cargo test` argv[0] is the test harness.
fn collect() -> Vec<(&'static str, String)> {
    let findings = podbox_probe::run();
    let selection = podbox_probe::select::Selection::choose(&findings);
    let store = podbox_image::open_store()
        .map(|s| s.root().display().to_string())
        .unwrap_or_else(|e| format!("unavailable: {e}"));
    fields_from(
        selection.rung.word(),
        selection.exit_code(podbox_probe::select::Strictness::Refuse) == 0,
        store,
    )
}

fn fields_from(rung: &str, strict_ok: bool, store: String) -> Vec<(&'static str, String)> {
    let count = |s: parity::Status| {
        parity::TABLE
            .iter()
            .filter(|r| r.flag.is_none() && r.status == s)
            .count()
            .to_string()
    };
    vec![
        ("Parity", parity::json()),
        ("ParityRows", parity::TABLE.len().to_string()),
        ("Version", env!("CARGO_PKG_VERSION").to_string()),
        ("Rung", rung.to_string()),
        ("StrictOk", strict_ok.to_string()),
        ("Store", store),
        (
            "Platform",
            podbox_image::platform::Platform::host().to_string(),
        ),
        ("Kernel", kernel_release()),
        (
            "Verbs",
            parity::TABLE
                .iter()
                .filter(|r| r.flag.is_none())
                .count()
                .to_string(),
        ),
        ("VerbsNative", count(parity::Status::Native)),
        ("VerbsDegraded", count(parity::Status::Degraded)),
        ("VerbsStub", count(parity::Status::Stub)),
        ("VerbsNone", count(parity::Status::None)),
    ]
}

/// ⚠ Read from `/proc`, and a dash where it cannot be read. docs/AGENTS.md
/// absolute 3: a dash where a value is unknown, never a plausible-looking one.
fn kernel_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "-".into())
}

fn pick(fields: &[(&str, String)], key: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn human(fields: &[(&str, String)]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "podbox {}\n  rung          {}   (--strict would {})\n  platform      {}\n  \
         kernel        {}\n  store         {}\n\n",
        pick(fields, "Version"),
        pick(fields, "Rung"),
        if pick(fields, "StrictOk") == "true" {
            "pass"
        } else {
            "refuse"
        },
        pick(fields, "Platform"),
        pick(fields, "Kernel"),
        pick(fields, "Store"),
    ));
    out.push_str(&format!(
        "parity (TOOL.md section 6.8): {} rows, {} verbs\n  \
         Native {}   Degraded {}   Stub {}   None {}\n\n",
        pick(fields, "ParityRows"),
        pick(fields, "Verbs"),
        pick(fields, "VerbsNative"),
        pick(fields, "VerbsDegraded"),
        pick(fields, "VerbsStub"),
        pick(fields, "VerbsNone"),
    ));
    out.push_str(&parity::text());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ Every declared field is built, and every built field is declared. A
    /// name in one list and not the other is a field that `--format` accepts
    /// and renders blank, which is the wrong answer that looks right.
    #[test]
    fn the_declared_fields_are_the_built_ones() {
        let built: Vec<&str> = fields_from("chroot", false, "/nowhere".into())
            .iter()
            .map(|(k, _)| *k)
            .collect();
        assert_eq!(built, FIELDS);
        for d in DOCUMENTS {
            assert!(FIELDS.contains(d), "{d} is a document and not a field");
        }
    }

    /// ⭐ T-0801's `Prove`, without a shell: the template it names renders the
    /// table, and what comes out parses as an array of the right shape.
    #[test]
    fn the_acceptance_template_renders_the_whole_table() {
        let fields = vec![("Parity", parity::json())];
        let out = format::render_with_documents("{{json .Parity}}", &fields, DOCUMENTS).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = doc.as_array().unwrap();
        assert!(arr.len() >= 60, "{} rows", arr.len());
        assert!(arr.iter().all(|r| {
            matches!(
                r["status"].as_str(),
                Some("Native") | Some("Degraded") | Some("Stub") | Some("None")
            )
        }));
    }
}
