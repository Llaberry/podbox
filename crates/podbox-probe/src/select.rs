//! `TODO/probe.md` T-0107: mode selection, and one switch that turns every
//! degradation into a refusal.
//!
//! ⛔ **A weaker mode must never satisfy a stronger request.** A user who
//! believes they have namespaces when they have a `chroot` is worse off than
//! one who is told the truth, and an automated user cannot notice the
//! difference the way a human skimming a log might.
//!
//! The rung is derived from the probe results, never from a privilege that
//! usually implies them. Two mechanisms are taken from the corpus and neither
//! project's architecture is:
//!
//! - `references/RuriOSS__ruri/tree/src/include/ruri.h:237-248`.
//!   `ruri_warn_on_error` is non-fatal by default and, under
//!   `ruri_flag(force_panic)`, warns and then exits at the same site. **One
//!   switch converts every degradation into a refusal.** [`Strictness`] is
//!   that switch. ⚠ `ruri`'s `show` argument is
//!   `!ruri_flag(disable_warnings)`, so a flag can silence its degradation
//!   notices; podbox suppresses the banner by configuration and never by
//!   default, because a silenced degradation is `sandlock`'s failure mode with
//!   a flag in front of it.
//! - `references/indigo-dc__udocker/tree/udocker/engine/execmode.py:43-56`.
//!   `get_mode()`'s precedence is force, then the per-container file, then a
//!   config override, then a per-architecture default, then `DEFAULT`.
//!   [`Selection::choose`] is the same precedence with podbox's sources.

use crate::probes::Group;
use crate::verdict::{Outcome, Verdict};
use crate::Findings;

/// `TOOL.md` section 4.1's ladder, strongest first. ⛔ The order is the
/// comparison: `Rung::Namespace < Rung::Chroot` is what "a weaker mode must
/// never satisfy a stronger request" is enforced with.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Rung {
    Namespace,
    Supervise,
    Chroot,
    Interpose,
    Unsupported,
}

impl Rung {
    pub fn word(self) -> &'static str {
        match self {
            Rung::Namespace => "namespace",
            Rung::Supervise => "supervise",
            Rung::Chroot => "chroot",
            Rung::Interpose => "interpose",
            Rung::Unsupported => "unsupported",
        }
    }

    /// What the rung must never claim, per `TOOL.md` section 4.1's last column.
    pub fn must_never_claim(self) -> &'static str {
        match self {
            Rung::Namespace => "",
            Rung::Supervise => {
                "anything whose arguments it cannot read; on this runtime, exec remapping"
            }
            Rung::Chroot => "process, network, IPC or mount isolation",
            Rung::Interpose => "any security property whatsoever",
            Rung::Unsupported => "",
        }
    }
}

/// ⭐ `ruri`'s inversion, as one switch. Under [`Strictness::Refuse`] a rung
/// below the requested floor is an error rather than a notice.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Report the rung achieved and continue. The banner still names every
    /// degradation; it is never silent.
    Warn,
    /// Refuse anything below the floor.
    Refuse,
}

/// Why a rung was not reached. One per rung considered and rejected, so the
/// diagnostic names the unmet requirement rather than the conclusion.
pub struct Rejection {
    pub rung: Rung,
    pub requirement: &'static str,
    pub evidence: String,
}

pub struct Selection {
    pub rung: Rung,
    pub rejected: Vec<Rejection>,
    /// ⭐ True only when every control in the attribution set answered. A
    /// probe whose control has stopped answering has stopped discriminating,
    /// and `TODO/probe.md` T-0102 rules that it must say so rather than
    /// reporting a mechanism.
    pub controls_answered: bool,
    pub control_notes: Vec<String>,
}

/// The verdict of one row, by name, or `None` when the set carries no such row.
fn row<'a>(f: &'a Findings, name: &str) -> Option<&'a Outcome> {
    f.rows.iter().find(|(n, _)| *n == name).map(|(_, o)| o)
}

fn is(f: &Findings, name: &str, want: Verdict) -> bool {
    row(f, name).map(|o| o.verdict == want) == Some(true)
}

fn ok(f: &Findings, name: &str) -> bool {
    is(f, name, Verdict::Ok)
}

/// How a row reads in a rejection: the verdict and, for a denial, its errno.
fn evidence(f: &Findings, name: &str) -> String {
    match row(f, name) {
        None => format!("{name}=(not probed)"),
        Some(o) => match (o.verdict, o.errno) {
            (Verdict::Ok, _) => format!("{name}=ok"),
            (Verdict::Denied, Some(e)) => format!("{name}={}", e.name()),
            (Verdict::Denied, None) => format!("{name}=denied"),
            (Verdict::Skip, _) => format!("{name}=skip"),
        },
    }
}

fn evidence_of(f: &Findings, names: &[&str]) -> String {
    names
        .iter()
        .map(|n| evidence(f, n))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Selection {
    /// Derive the rung. ⛔ Each rung's requirement is the **operation**, never
    /// a privilege that usually implies it: `TOOL.md` section 6.1 rule 1.
    pub fn choose(f: &Findings) -> Selection {
        let mut rejected = Vec::new();

        // -- namespace: namespace creation AND mounts AND ID maps -----------
        //
        // "mounts" is the attach half and not the create half. A machine where
        // fsmount succeeds and move_mount is denied has detached mounts it
        // cannot use, which is TODO/probe.md T-0103's whole point.
        let ns_created = ok(f, "unshare(CLONE_NEWNS)") || ok(f, "clone(CLONE_NEWNS)");
        let mounts = ok(f, "mount(tmpfs,/mnt)") || ok(f, "mount(tmpfs,/mnt) in clone(NEWNS)");
        if ns_created && mounts {
            return Selection {
                rung: Rung::Namespace,
                rejected,
                ..Selection::controls(f)
            };
        }
        rejected.push(Rejection {
            rung: Rung::Namespace,
            requirement: "namespace creation and mounts and ID maps for what was asked",
            evidence: evidence_of(
                f,
                &[
                    "unshare(CLONE_NEWNS)",
                    "clone(CLONE_NEWNS)",
                    "mount(tmpfs,/mnt)",
                    "mount(tmpfs,/mnt) in clone(NEWNS)",
                ],
            ),
        });

        // -- supervise: three legs, and the tier is refused if any is missing
        //
        // ⛔ TODO/supervise.md T-0606: never fall back per call. The third leg
        // is not in `TOOL.md` section 4.1's row and is established by
        // `references/multikernel__sandlock` issue #27: the only known fix for
        // seccomp-notify's TOCTOU race needs PTRACE_SEIZE on every thread of
        // every process in the sandbox before a Continue, so a runtime where
        // ptrace is filtered has no way to make the tier race-safe.
        let listener = ok(f, "seccomp(NEW_LISTENER)");
        let arg_channel = ok(f, "open /proc/self/mem O_RDONLY");
        let race_safe = ok(f, "ptrace(PTRACE_TRACEME)");
        if listener && arg_channel && race_safe {
            return Selection {
                rung: Rung::Supervise,
                rejected,
                ..Selection::controls(f)
            };
        }
        rejected.push(Rejection {
            rung: Rung::Supervise,
            requirement: "a notification listener, a channel to read the child's syscall \
                          arguments, and ptrace to make a Continue race-safe",
            evidence: evidence_of(
                f,
                &[
                    "seccomp(NEW_LISTENER)",
                    "open /proc/self/mem O_RDONLY",
                    "ptrace(PTRACE_TRACEME)",
                ],
            ),
        });

        // -- chroot ---------------------------------------------------------
        if ok(f, "chroot(/tmp)") {
            return Selection {
                rung: Rung::Chroot,
                rejected,
                ..Selection::controls(f)
            };
        }
        rejected.push(Rejection {
            rung: Rung::Chroot,
            requirement: "CAP_SYS_CHROOT, a prepared rootfs, payload syscalls permitted",
            evidence: evidence_of(f, &["chroot(/tmp)"]),
        });

        // -- interpose ------------------------------------------------------
        //
        // The rung needs a dynamically linked payload and sufficient libc
        // coverage, neither of which exists until there is a payload. What the
        // probe can establish is that this machine can execute one at all.
        if ok(f, "exec(/tmp/execprobe)") || ok(f, "memfd_create+exec") {
            return Selection {
                rung: Rung::Interpose,
                rejected,
                ..Selection::controls(f)
            };
        }
        rejected.push(Rejection {
            rung: Rung::Interpose,
            requirement: "a payload this machine can execute at all",
            evidence: evidence_of(f, &["exec(/tmp/execprobe)", "memfd_create+exec"]),
        });

        Selection {
            rung: Rung::Unsupported,
            rejected,
            ..Selection::controls(f)
        }
    }

    /// ⭐ TODO/probe.md T-0102's discipline, evaluated over the controls that
    /// travelled in this same run.
    fn controls(f: &Findings) -> Selection {
        let mut notes = Vec::new();
        let mut answered = true;
        for (name, want) in [
            ("pidfd_getfd(-1,-1) [control]", crate::sys::EBADF),
            ("kcmp(-1,-1,...) [control]", crate::sys::ESRCH),
        ] {
            match row(f, name) {
                None => {
                    answered = false;
                    notes.push(format!("{name} did not run"));
                }
                Some(o) if o.verdict == Verdict::Skip => {
                    answered = false;
                    notes.push(format!("{name} could not answer: {}", o.reason));
                }
                Some(o) if o.errno == Some(want) => {}
                Some(o) => {
                    answered = false;
                    notes.push(format!(
                        "{name} answered {} where the discriminator needs {}; \
                         it has stopped separating a filter from a policy",
                        o.errno.map(|e| e.name()).unwrap_or_else(|| "ok".into()),
                        want.name()
                    ));
                }
            }
        }
        Selection {
            rung: Rung::Unsupported,
            rejected: Vec::new(),
            controls_answered: answered,
            control_notes: notes,
        }
    }

    /// The floor `--strict` holds to: nothing below `namespace` satisfies a
    /// request that asked for isolation.
    pub const STRICT_FLOOR: Rung = Rung::Namespace;

    pub fn meets(&self, floor: Rung) -> bool {
        self.rung <= floor
    }

    /// Exit code for `podbox probe`. ⛔ `TODO/probe.md` T-0110: 0 always for a
    /// plain run, because the probe ran and answered; non-zero under `--strict`
    /// when the rung is below the floor, so a caller can gate without parsing.
    ///
    /// ⚠ **This code carries the rung and only the rung.**
    /// [`Selection::controls_answered`] is a separate statement about the
    /// diagnostic and it does not move this number. Folding the two together
    /// would make `--strict` fail on every kernel built without
    /// `CONFIG_CHECKPOINT_RESTORE`, where `kcmp(2)` is absent and podbox is
    /// otherwise fully capable, and a caller could no longer tell the two
    /// apart from one exit code. `controls_answered` is in the JSON and the
    /// shortfall is on stderr on every run.
    pub fn exit_code(&self, strictness: Strictness) -> i32 {
        match strictness {
            Strictness::Warn => 0,
            Strictness::Refuse if self.meets(Self::STRICT_FLOOR) => 0,
            Strictness::Refuse => 1,
        }
    }
}

/// What the selected rung provides, derived from the probes rather than
/// asserted. `TOOL.md` section 6.8's banner is these six fields.
pub struct Provides {
    pub namespaces: String,
    pub mounts: String,
    pub devices: String,
    pub ownership: String,
    pub network: String,
    pub pids: String,
}

impl Provides {
    pub fn of(rung: Rung, f: &Findings) -> Provides {
        let namespaces = if rung == Rung::Namespace {
            "as configured".to_string()
        } else if ok(f, "clone(CLONE_NEWUTS)") && ok(f, "sethostname(in NEWUTS)") {
            "uts-only".to_string()
        } else {
            "none".to_string()
        };
        // ⛔ What the RUNG provides, not what the machine permits. Only the
        // `namespace` rung sets a mount topology up, so at every rung below it
        // podbox mounts nothing, however attachable this machine's mounts turn
        // out to be. `TOOL.md` section 4.1: report the mode you achieved. The
        // machine's own four verdicts are in the evidence block beneath, where
        // they are a measurement rather than a claim about the mode.
        let mounts = if rung == Rung::Namespace {
            crate::mounts::Mounts::of(f).summary()
        } else {
            "none".to_string()
        };
        let devices = if ok(f, "mknod(chr 1:3 /tmp/nodprobe)") {
            "real".to_string()
        } else {
            "shimmed".to_string()
        };
        let ownership = if ok(f, "chown(f,0,42)") {
            "real".to_string()
        } else {
            "virtualized+sidecar".to_string()
        };
        // ⛔ Not measured, and stated as what the rung does rather than as a
        // reading. Only the `namespace` rung creates a network or pid
        // namespace, so at every other rung both are the caller's.
        let shared = |r: Rung| {
            if r == Rung::Namespace {
                "as configured".to_string()
            } else {
                "host-shared".to_string()
            }
        };
        Provides {
            namespaces,
            mounts,
            devices,
            ownership,
            network: shared(rung),
            pids: shared(rung),
        }
    }
}

/// The probe rows the banner's `probes:` line quotes, in `TOOL.md`
/// section 6.8's order.
pub const BANNER_ROWS: &[&str] = &[
    "clone(CLONE_NEWNS)",
    "mount(tmpfs,/mnt)",
    "fsmount(tmpfs)",
    "move_mount(-> /tmp/mm-probe)",
    "clone(CLONE_NEWUTS)",
    "sethostname(in NEWUTS)",
    "chroot(/tmp)",
    "ptrace(PTRACE_TRACEME)",
];

/// `clone(CLONE_NEWNS)=ok`, `mount=EPERM`, `chroot=skip`: the banner's
/// shorthand, which is the verdict for `ok` and the errno name otherwise.
pub fn banner_cell(f: &Findings, name: &str) -> String {
    let short = name
        .strip_prefix("clone(CLONE_")
        .and_then(|r| r.strip_suffix(')'))
        .map(|r| format!("clone({r})"))
        .unwrap_or_else(|| name.split(['(', ' ']).next().unwrap_or(name).to_string());
    match row(f, name) {
        None => format!("{short}=(not probed)"),
        Some(o) => match (o.verdict, o.errno) {
            (Verdict::Ok, _) => format!("{short}=ok"),
            (Verdict::Denied, Some(e)) => format!("{short}={}", e.name()),
            (Verdict::Denied, None) => format!("{short}=denied"),
            (Verdict::Skip, Some(e)) => format!("{short}=skip({})", e.name()),
            (Verdict::Skip, None) => format!("{short}=skip"),
        },
    }
}

/// Every attribution row that has a control, for the report.
pub fn control_rows() -> impl Iterator<Item = &'static str> {
    crate::probes::PROBES
        .iter()
        .filter(|p| p.group == Group::Attribution && p.name.contains("[control]"))
        .map(|p| p.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys;
    use crate::verdict::Outcome;

    fn findings(rows: &[(&'static str, Outcome)]) -> Findings {
        Findings {
            rows: rows.to_vec(),
            ..Findings::empty()
        }
    }

    fn target_shape() -> Findings {
        // The reconstruction of TOOL.md section 3: namespaces creatable by
        // clone, every mount attach denied, chroot permitted, ptrace filtered.
        findings(&[
            ("unshare(CLONE_NEWNS)", Outcome::denied(sys::EPERM)),
            ("clone(CLONE_NEWNS)", Outcome::ok()),
            ("mount(tmpfs,/mnt)", Outcome::denied(sys::EPERM)),
            (
                "mount(tmpfs,/mnt) in clone(NEWNS)",
                Outcome::denied(sys::EPERM),
            ),
            ("seccomp(NEW_LISTENER)", Outcome::ok()),
            ("open /proc/self/mem O_RDONLY", Outcome::ok()),
            ("ptrace(PTRACE_TRACEME)", Outcome::denied(sys::EPERM)),
            ("chroot(/tmp)", Outcome::ok()),
            ("pidfd_getfd(-1,-1) [control]", Outcome::denied(sys::EBADF)),
            ("kcmp(-1,-1,...) [control]", Outcome::denied(sys::ESRCH)),
        ])
    }

    #[test]
    fn the_target_shape_selects_chroot() {
        assert_eq!(Selection::choose(&target_shape()).rung, Rung::Chroot);
    }

    #[test]
    fn an_unconfined_host_selects_namespace() {
        let f = findings(&[
            ("unshare(CLONE_NEWNS)", Outcome::ok()),
            ("clone(CLONE_NEWNS)", Outcome::ok()),
            ("mount(tmpfs,/mnt)", Outcome::ok()),
            ("chroot(/tmp)", Outcome::ok()),
            ("pidfd_getfd(-1,-1) [control]", Outcome::denied(sys::EBADF)),
            ("kcmp(-1,-1,...) [control]", Outcome::denied(sys::ESRCH)),
        ]);
        assert_eq!(Selection::choose(&f).rung, Rung::Namespace);
    }

    #[test]
    fn a_listener_without_ptrace_does_not_reach_supervise() {
        // TODO/supervise.md T-0606: the tier is refused when any leg is
        // missing, never entered and degraded per call.
        let s = Selection::choose(&target_shape());
        assert_eq!(s.rung, Rung::Chroot);
        let why = s
            .rejected
            .iter()
            .find(|r| r.rung == Rung::Supervise)
            .unwrap();
        assert!(
            why.evidence.contains("ptrace(PTRACE_TRACEME)=EPERM"),
            "{}",
            why.evidence
        );
    }

    #[test]
    fn a_weaker_rung_never_satisfies_the_strict_floor() {
        let s = Selection::choose(&target_shape());
        assert!(!s.meets(Selection::STRICT_FLOOR));
        assert_eq!(s.exit_code(Strictness::Refuse), 1);
        assert_eq!(s.exit_code(Strictness::Warn), 0);
    }

    #[test]
    fn a_control_that_cannot_answer_is_reported_apart_from_the_rung() {
        // ⭐ A kernel without CONFIG_CHECKPOINT_RESTORE: kcmp(2) answers
        // ENOSYS, which is the control saying it is not available rather than
        // the discriminator working. That is a statement about the diagnostic
        // and not about the rung, so it is reported and it does not move the
        // exit code: see `Selection::exit_code`.
        let mut rows = vec![
            ("unshare(CLONE_NEWNS)", Outcome::ok()),
            ("mount(tmpfs,/mnt)", Outcome::ok()),
            ("pidfd_getfd(-1,-1) [control]", Outcome::denied(sys::EBADF)),
            (
                "kcmp(-1,-1,...) [control]",
                Outcome::skip(Some(sys::ENOSYS), "no kcmp here"),
            ),
        ];
        let s = Selection::choose(&findings(&rows));
        assert_eq!(s.rung, Rung::Namespace);
        assert!(!s.controls_answered);
        assert_eq!(s.control_notes.len(), 1);
        assert_eq!(s.exit_code(Strictness::Refuse), 0);
        rows[3] = ("kcmp(-1,-1,...) [control]", Outcome::denied(sys::ESRCH));
        let s = Selection::choose(&findings(&rows));
        assert!(s.controls_answered);
        assert!(s.control_notes.is_empty());
    }

    #[test]
    fn a_control_answering_the_wrong_errno_is_caught_as_well_as_a_missing_one() {
        // ⛔ The failure this exists for is subtler than absence: a control
        // that answers something else has stopped separating a filter from a
        // policy, and the rows around it are no longer attributions.
        let f = findings(&[
            ("chroot(/tmp)", Outcome::ok()),
            ("pidfd_getfd(-1,-1) [control]", Outcome::denied(sys::EPERM)),
            ("kcmp(-1,-1,...) [control]", Outcome::denied(sys::ESRCH)),
        ]);
        let s = Selection::choose(&f);
        assert!(!s.controls_answered);
        assert!(
            s.control_notes[0].contains("stopped separating"),
            "{:?}",
            s.control_notes
        );
    }

    #[test]
    fn the_banner_reports_what_the_rung_provides_and_not_what_the_machine_permits() {
        // ⛔ The reconstruction of 2026-09-08 has no Landlock, so move_mount
        // attaches there and the machine's own mount summary reads `full`. The
        // selected rung is still `chroot`, which mounts nothing, and a banner
        // saying otherwise is the exact claim TOOL.md section 4.1 forbids.
        let f = findings(&[
            ("unshare(CLONE_NEWNS)", Outcome::denied(sys::EPERM)),
            ("clone(CLONE_NEWNS)", Outcome::ok()),
            ("mount(tmpfs,/mnt)", Outcome::denied(sys::EPERM)),
            ("chroot(/tmp)", Outcome::ok()),
            ("ptrace(PTRACE_TRACEME)", Outcome::denied(sys::EPERM)),
            ("fsopen(tmpfs)", Outcome::ok()),
            ("fsmount(tmpfs)", Outcome::ok()),
            ("move_mount(-> /tmp/mm-probe)", Outcome::ok()),
            ("openat(detached tmpfs, O_DIRECTORY)", Outcome::ok()),
        ]);
        let s = Selection::choose(&f);
        assert_eq!(s.rung, Rung::Chroot);
        assert_eq!(crate::mounts::Mounts::of(&f).summary(), "full");
        assert_eq!(Provides::of(s.rung, &f).mounts, "none");
        assert_eq!(Provides::of(s.rung, &f).network, "host-shared");
    }

    #[test]
    fn the_chroot_rung_says_what_it_must_never_claim() {
        assert!(Rung::Chroot.must_never_claim().contains("network"));
    }

    #[test]
    fn the_banner_cell_names_the_errno_rather_than_the_word_denied() {
        let f = target_shape();
        assert_eq!(banner_cell(&f, "clone(CLONE_NEWNS)"), "clone(NEWNS)=ok");
        assert_eq!(banner_cell(&f, "ptrace(PTRACE_TRACEME)"), "ptrace=EPERM");
    }
}
