//! What the probe prints, and on which channel.
//!
//! `TODO/probe.md` T-0108 and T-0110, and `TOOL.md` sections 6.8 and 6.9.
//!
//! ⛔ **The rung goes to stdout and the evidence to stderr.** A payload's
//! stdout is data to whatever consumes it, and a banner on it corrupts every
//! pipeline. The same split `pathshim` uses at
//! `references/compforge__pathshim/tree/src/main.rs:134-138`.
//!
//! ⛔ **No output may imply namespaces, cgroups or devices exist when they do
//! not.** Every field below is derived from a row that ran, and a row that did
//! not run prints as a skip with its reason rather than as an absence.

use crate::identity::Identity;
use crate::mounts::Mounts;
use crate::select::{banner_cell, Provides, Rung, Selection, BANNER_ROWS};
use crate::verdict::Verdict;
use crate::{json, Findings};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Every probe row, in the reference harness's own format, so a podbox run and
/// a `verification/probe` run diff against each other line by line.
pub fn rows(f: &Findings) -> String {
    let mut out = String::new();
    for (name, o) in &f.rows {
        out.push_str(&o.row(name));
        out.push('\n');
    }
    out
}

/// `TOOL.md` section 6.8's banner. Two lines of mode, then the probe verdicts
/// that decided it.
pub fn banner(f: &Findings, sel: &Selection) -> String {
    let p = Provides::of(sel.rung, f);
    let mut out = format!(
        "podbox {VERSION}: mode={} (namespaces: {}; mounts: {}; devices: {};\n\
         ownership: {}; network: {}; pids: {})\n",
        sel.rung.word(),
        p.namespaces,
        p.mounts,
        p.devices,
        p.ownership,
        p.network,
        p.pids
    );
    let cells: Vec<String> = BANNER_ROWS.iter().map(|n| banner_cell(f, n)).collect();
    out.push_str("probes: ");
    out.push_str(&cells[..cells.len().min(4)].join(" "));
    out.push('\n');
    if cells.len() > 4 {
        out.push_str("        ");
        out.push_str(&cells[4..].join(" "));
        out.push('\n');
    }
    if !sel.rung.must_never_claim().is_empty() {
        out.push_str(&format!(
            "this mode does NOT provide: {}\n",
            sel.rung.must_never_claim()
        ));
    }
    // ⛔ TODO/enter.md T-0506 point 5. Where an emulator produced these rows,
    // the banner says so HERE, beside the rung, rather than further down: the
    // rung is the line a caller acts on, and a rung measured by qemu is a
    // statement about qemu.
    if let Some(note) = crate::interp::detect().note() {
        out.push_str(&note);
        out.push('\n');
    }
    out
}

/// The full stderr block: the banner, then what the diagnostic of `TOOL.md`
/// section 6.9 needs to name the mechanism and the remedy rather than an errno
/// on its own.
pub fn evidence(f: &Findings, sel: &Selection) -> String {
    let mut out = banner(f, sel);

    out.push_str("\nidentity\n");
    out.push_str(&identity_block(&f.identity));

    let m = Mounts::of(f);
    out.push_str("\nmounts, as four verdicts and not one\n");
    for (half, o) in m.rows() {
        out.push_str(&format!("  {half:<10} {}\n", short(o)));
    }
    if let Some(note) = m.capability_note() {
        out.push_str(&format!("  ⭐ {note}\n"));
    }

    out.push_str("\nwritable, by writing\n");
    for w in &f.writable {
        let space = match (&w.space, w.space_error) {
            (Some(s), _) => format!(
                "{} blocks of {} B free, {} inodes free",
                s.blocks_free, s.block_size, s.inodes_free
            ),
            (None, Some(e)) => format!("statfs: {} ({})", e.name(), e.0),
            (None, None) => "statfs: not attempted".to_string(),
        };
        out.push_str(&format!(
            "  {:<26} {:<8} {space}  [{}]\n",
            w.path,
            short(&w.outcome),
            w.source
        ));
    }

    out.push_str("\ncontrols for the bogus-argument discriminator\n");
    for name in crate::select::control_rows() {
        match f.get(name) {
            Some(o) => out.push_str(&format!("  {}\n", o.row(name))),
            None => out.push_str(&format!("  {name:<34} (not probed)\n")),
        }
    }
    if sel.controls_answered {
        out.push_str("  ⭐ the discriminator is still separating a filter from a policy\n");
    } else {
        for note in &sel.control_notes {
            out.push_str(&format!("  ⛔ {note}\n"));
        }
        out.push_str(
            "  ⛔ a probe whose control has stopped answering has stopped\n     \
             discriminating. Read the mechanism attributions below as unconfirmed.\n",
        );
    }

    if !sel.rejected.is_empty() {
        out.push_str("\nrungs considered and not reached\n");
    }
    for r in &sel.rejected {
        out.push_str(&format!(
            "  {:<12} needs {}\n",
            r.rung.word(),
            r.requirement
        ));
        out.push_str(&format!("  {:<12}   got  {}\n", "", r.evidence));
    }

    let skipped: Vec<&(&str, crate::verdict::Outcome)> = f
        .rows
        .iter()
        .filter(|(_, o)| o.verdict == Verdict::Skip)
        .collect();
    if !skipped.is_empty() {
        out.push_str(
            "\ncould not run. ⛔ None of these is a denial, and none of them was\n\
             counted as one when the rung above was chosen.\n",
        );
        for (name, o) in skipped {
            out.push_str(&format!("  {name}\n      {}\n", o.reason));
        }
    }

    out.push_str(&format!(
        "\nprobe children were re-executed from {}\n",
        f.self_exe
    ));
    out
}

fn short(o: &crate::verdict::Outcome) -> String {
    match (o.verdict, o.errno) {
        (Verdict::Ok, _) => "ok".to_string(),
        (Verdict::Denied, Some(e)) => format!("denied {}", e.name()),
        (Verdict::Denied, None) => "denied".to_string(),
        (Verdict::Skip, Some(e)) => format!("skip ({})", e.name()),
        (Verdict::Skip, None) => "skip".to_string(),
    }
}

fn identity_block(id: &Identity) -> String {
    let mut out = format!("  uid={} gid={} groups={:?}\n", id.uid, id.gid, id.groups);
    for (label, v) in [
        ("/proc/self/uid_map", &id.uid_map),
        ("/proc/self/gid_map", &id.gid_map),
        ("/proc/self/setgroups", &id.setgroups),
    ] {
        match v {
            Some(s) => out.push_str(&format!("  {label}: {s}\n")),
            None => out.push_str(&format!("  {label}: unreadable\n")),
        }
    }
    for (label, v) in [
        ("CapEff", &id.cap_eff),
        ("Seccomp", &id.seccomp),
        ("Seccomp_filters", &id.seccomp_filters),
        ("NoNewPrivs", &id.no_new_privs),
        ("Threads", &id.threads),
    ] {
        match v {
            Some(s) => out.push_str(&format!("  {label}: {s}\n")),
            None => out.push_str(&format!("  {label}: unreadable\n")),
        }
    }
    if let Some(map) = &id.uid_map {
        if crate::identity::is_single_id_map(map) {
            out.push_str(
                "  ⭐ the uid map is a single range of one id. That is the fact that\n     \
                 explains every EINVAL from chown(2) and setuid(2) here: the id is\n     \
                 not mapped, so no capability makes it valid.\n",
            );
        }
    }
    if id.groups.contains(&65534) {
        out.push_str(
            "  ⚠ gid 65534 is overflowgid, which getgroups(2) returns for a group\n     \
             with no mapping. It is not evidence of a supplementary group.\n",
        );
    }
    for why in &id.unreadable {
        out.push_str(&format!("  ⚠ not read: {why}\n"));
    }
    out
}

/// `--json`: the same findings, for a harness to diff against
/// `experiments/results/`.
pub fn document(f: &Findings, sel: &Selection) -> String {
    let mut s = String::new();
    let mut o = json::Obj::new(&mut s);
    o.str("podbox", VERSION);
    o.str("rung", sel.rung.word());
    o.str("rung_must_never_claim", sel.rung.must_never_claim());
    // ⚠ The rung and only the rung: `controls_answered` is beside it and is a
    // statement about the diagnostic. See `Selection::exit_code`.
    o.bool("strict_ok", sel.meets(Selection::STRICT_FLOOR));
    o.bool("controls_answered", sel.controls_answered);
    o.str("self_exe", &f.self_exe);

    // ⛔ TODO/enter.md T-0506 point 5: WHICH INSTRUMENT ANSWERED, beside the
    // rung rather than at the end, because a harness that reads `rung` and not
    // this has read qemu's answer as the machine's.
    // ⚠ `emulated: false` is the absence of evidence and says so in `checked`;
    // it is not a claim that this is a bare machine.
    let interp = crate::interp::detect();
    o.obj("measured_by", |m| {
        m.bool("emulated", interp.is_emulated());
        m.str("interpreter", &interp.key());
        match &interp {
            crate::interp::Interpreter::Emulated { evidence, .. } => {
                m.str("evidence", evidence);
            }
            crate::interp::Interpreter::NoEvidence { checked } => {
                m.arr("checked", |a| {
                    for c in checked {
                        a.str(c);
                    }
                });
            }
        }
    });

    o.arr("control_notes", |a| {
        for n in &sel.control_notes {
            a.str(n);
        }
    });

    // ⭐ TODO/probe.md T-0102's shape: the control's answer, by name.
    o.obj("controls", |c| {
        for name in crate::select::control_rows() {
            let key = name.split_once('(').map(|(k, _)| k).unwrap_or(name);
            match f.get(name) {
                Some(x) if x.verdict == Verdict::Denied => {
                    c.opt_str(key, x.errno_name().as_deref());
                }
                Some(x) if x.verdict == Verdict::Ok => {
                    c.str(key, "ok");
                }
                // ⛔ A control that could not run is null, never the errno of
                // whatever stopped it: that would read as an answer.
                _ => {
                    c.null(key);
                }
            }
        }
    });

    // ⭐ TODO/enter.md T-0503's `has("ptmx")`. Derived from the two rows that
    // ran, never a second pair of calls: `usable` is the OPEN, not the stat,
    // because a device node that exists and cannot be opened is exactly the
    // degraded `-t` the entry refuses.
    let ptmx_stat = f.get("stat(/dev/ptmx)");
    let ptmx_open = f.get("open(/dev/ptmx, O_RDWR)");
    o.obj("ptmx", |x| {
        x.str("path", crate::probes::PTMX);
        x.bool(
            "present",
            ptmx_stat.map(|s| s.verdict == Verdict::Ok) == Some(true),
        );
        x.bool(
            "usable",
            ptmx_open.map(|s| s.verdict == Verdict::Ok) == Some(true),
        );
        x.opt_str("stat_verdict", ptmx_stat.map(|s| s.verdict.word()));
        x.opt_str("stat_detail", ptmx_stat.map(|s| s.reason.as_str()));
        x.opt_str(
            "stat_errno_name",
            ptmx_stat.and_then(|s| s.errno_name()).as_deref(),
        );
        x.opt_str("open_verdict", ptmx_open.map(|s| s.verdict.word()));
        x.opt_str(
            "open_errno_name",
            ptmx_open.and_then(|s| s.errno_name()).as_deref(),
        );
    });

    let m = Mounts::of(f);
    o.obj("mounts", |mo| {
        for (half, out) in m.rows() {
            mo.str(half, out.verdict.word());
        }
        mo.str("summary", &m.summary());
        mo.opt_str("capability_note", m.capability_note());
    });

    o.obj("identity", |i| {
        i.num("uid", f.identity.uid);
        i.num("gid", f.identity.gid);
        i.arr("groups", |a| {
            for g in &f.identity.groups {
                a.num(*g as i64);
            }
        });
        i.opt_str("uid_map", f.identity.uid_map.as_deref());
        i.opt_str("gid_map", f.identity.gid_map.as_deref());
        i.opt_str("setgroups", f.identity.setgroups.as_deref());
        i.opt_str("cap_eff", f.identity.cap_eff.as_deref());
        i.opt_str("cap_prm", f.identity.cap_prm.as_deref());
        i.opt_str("cap_bnd", f.identity.cap_bnd.as_deref());
        i.opt_str("seccomp", f.identity.seccomp.as_deref());
        i.opt_str("seccomp_filters", f.identity.seccomp_filters.as_deref());
        i.opt_str("no_new_privs", f.identity.no_new_privs.as_deref());
        i.opt_str("threads", f.identity.threads.as_deref());
        i.arr("unreadable", |a| {
            for u in &f.identity.unreadable {
                a.str(u);
            }
        });
    });

    o.arr("writable", |a| {
        for w in &f.writable {
            a.obj(|e| {
                e.str("path", &w.path);
                e.str("source", &w.source);
                e.bool("writable", w.writable());
                e.str("verdict", w.outcome.verdict.word());
                e.opt_num("errno", w.outcome.errno.map(|x| x.0 as i64));
                e.opt_str("errno_name", w.outcome.errno_name().as_deref());
                e.opt_unum("blocks_free", w.space.as_ref().map(|s| s.blocks_free));
                e.opt_num("block_size", w.space.as_ref().map(|s| s.block_size));
                e.opt_unum("inodes_free", w.space.as_ref().map(|s| s.inodes_free));
                e.opt_str("reason", none_if_empty(&w.outcome.reason));
            });
        }
    });

    o.arr("probes", |a| {
        for (name, out) in &f.rows {
            a.obj(|e| {
                e.str("name", name);
                e.str("verdict", out.verdict.word());
                e.opt_num("errno", out.errno.map(|x| x.0 as i64));
                e.opt_str("errno_name", out.errno_name().as_deref());
                e.opt_str("reason", none_if_empty(&out.reason));
                e.str(
                    "group",
                    match crate::probes::find(name).map(|p| p.group) {
                        Some(crate::probes::Group::Attribution) => "attribution",
                        _ => "census",
                    },
                );
            });
        }
    });

    // ⭐ TODO/probe.md T-0111. The cache stores this document verbatim, so the
    // key it is validated against travels inside it and there is no second
    // serializer to drift against the first.
    //
    // ⚠ Five of the seven components are also in the `identity` block above.
    // `docs/conventions/forbidden-patterns.md` forbids a value in two places
    // WITH NO CHECK THAT THEY AGREE, and the check is
    // `the_cache_key_agrees_with_the_identity_block_it_is_derived_from` below.
    let key = crate::identity::confinement_key(&f.identity);
    o.obj("cache_key", |k| {
        for (name, value) in key.components() {
            k.opt_str(name, value);
        }
        k.bool("complete", key.is_complete());
    });

    o.arr("rejected", |a| {
        for r in &sel.rejected {
            a.obj(|e| {
                e.str("rung", r.rung.word());
                e.str("requirement", r.requirement);
                e.str("evidence", &r.evidence);
            });
        }
    });

    o.end();
    s.push('\n');
    s
}

fn none_if_empty(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// `Rung` is re-exported here so a caller printing the stdout word does not
/// have to reach into `select`.
pub fn word(rung: Rung) -> &'static str {
    rung.word()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The document, taken from a real run of the shipping path.
    fn one_document() -> String {
        let f = crate::run();
        let sel = Selection::choose(&f);
        document(&f, &sel)
    }

    /// A value of `key` at the top level of a flat JSON object, without a
    /// parser. ⚠ Only sound for the shapes asserted below, which are all
    /// `"key":"value"` on one line, and it is test scaffolding rather than
    /// something shipped.
    fn field(doc: &str, key: &str) -> Option<String> {
        let at = doc.find(&format!("\"{key}\":\""))? + key.len() + 4;
        let rest = &doc[at..];
        Some(rest[..rest.find('"')?].to_string())
    }

    #[test]
    fn the_cache_key_agrees_with_the_identity_block_it_is_derived_from() {
        // ⛔ docs/conventions/forbidden-patterns.md: a value in two places with
        // no check that they agree drifts, and the copy a reader trusts is the
        // wrong one. This is that check. TODO/probe.md T-0111.
        let doc = one_document();
        let id = crate::identity::read();
        let key = crate::identity::confinement_key(&id);
        for (name, value) in key.components() {
            if let Some(v) = value {
                assert!(
                    doc.contains(&format!("\"{name}\":")),
                    "the document has no {name}"
                );
                if matches!(
                    name,
                    "uid_map" | "gid_map" | "setgroups" | "seccomp" | "seccomp_filters"
                ) {
                    assert_eq!(
                        id_field(&id, name).as_deref(),
                        Some(v),
                        "cache_key.{name} and identity.{name} disagree"
                    );
                }
            }
        }
    }

    fn id_field(id: &Identity, name: &str) -> Option<String> {
        match name {
            "uid_map" => id.uid_map.clone(),
            "gid_map" => id.gid_map.clone(),
            "setgroups" => id.setgroups.clone(),
            "seccomp" => id.seccomp.clone(),
            "seccomp_filters" => id.seccomp_filters.clone(),
            _ => None,
        }
    }

    #[test]
    fn the_document_carries_a_complete_cache_key_on_this_machine() {
        let doc = one_document();
        assert!(doc.contains("\"cache_key\":{"), "{doc}");
        assert!(doc.contains("\"complete\":true"), "{doc}");
        let mnt = field(&doc, "mnt_ns").expect("mnt_ns in the document");
        assert!(mnt.starts_with("mnt:["), "{mnt}");
    }

    #[test]
    fn two_documents_from_one_machine_agree_on_the_rung_and_the_key() {
        // ⚠ The rung and the key, not the whole document: the writable probe
        // reports free blocks, which move between two runs on a live machine.
        let (a, b) = (one_document(), one_document());
        assert_eq!(field(&a, "rung"), field(&b, "rung"));
        assert_eq!(field(&a, "mnt_ns"), field(&b, "mnt_ns"));
        assert_eq!(field(&a, "boot_id"), field(&b, "boot_id"));
    }
}
