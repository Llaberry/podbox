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
