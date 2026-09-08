//! `TODO/probe.md` T-0105: read the ID maps directly rather than inferring
//! them.
//!
//! A dozen probes infer what one read answers, and the inference is wrong in
//! the case that matters: `groups=0,65534` looks like a supplementary group
//! and is `overflowgid`, which is what `getgroups(2)` returns for a group with
//! no mapping.
//!
//! ⚠ A full `CapEff` proves nothing on its own. A process that is root in a new
//! user namespace receives the complete set by construction, whatever its
//! parent held.

use crate::sys;

#[derive(Default)]
pub struct Identity {
    pub uid: i64,
    pub gid: i64,
    pub groups: Vec<i32>,
    /// The three files, verbatim and whitespace-collapsed. `None` where the
    /// file could not be read, with the reason in [`Identity::unreadable`].
    pub uid_map: Option<String>,
    pub gid_map: Option<String>,
    pub setgroups: Option<String>,
    /// From `/proc/self/status`, by name.
    pub cap_eff: Option<String>,
    pub cap_prm: Option<String>,
    pub cap_bnd: Option<String>,
    pub seccomp: Option<String>,
    pub seccomp_filters: Option<String>,
    pub no_new_privs: Option<String>,
    pub threads: Option<String>,
    /// Every read that did not happen, and why. ⛔ An absent value is reported
    /// as absent, never as a zero or an empty string that reads like one.
    pub unreadable: Vec<String>,
}

/// True when the map is a single range covering exactly one id, which is the
/// fact that explains every `EINVAL` from `chown` and `setuid` on this class of
/// runtime. `TOOL.md` section 6.9 quotes it in the diagnostic for that reason.
pub fn is_single_id_map(map: &str) -> bool {
    let mut ranges = 0;
    for line in map.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 3 {
            continue;
        }
        ranges += 1;
        if f[2] != "1" {
            return false;
        }
    }
    ranges == 1
}

fn read_trimmed(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map(|s| {
            s.lines()
                .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .map_err(|e| format!("{path}: {e}"))
}

pub fn read() -> Identity {
    // getuid(2) and getgid(2) are documented always to succeed, so their raw
    // returns are the values and there is no error branch to write.
    let mut id = Identity {
        uid: unsafe { sys::syscall6(sys::SYS_GETUID, 0, 0, 0, 0, 0, 0) },
        gid: unsafe { sys::syscall6(sys::SYS_GETGID, 0, 0, 0, 0, 0, 0) },
        ..Default::default()
    };
    match sys::getgroups() {
        Ok(mut g) => {
            // getgroups(2) makes no ordering guarantee, and two runs that
            // differ only in order are two runs somebody has to read twice.
            g.sort_unstable();
            id.groups = g;
        }
        Err(e) => id
            .unreadable
            .push(format!("getgroups(2): {} ({})", e.name(), e.0)),
    }

    for (path, slot) in [
        ("/proc/self/uid_map", &mut id.uid_map),
        ("/proc/self/gid_map", &mut id.gid_map),
        ("/proc/self/setgroups", &mut id.setgroups),
    ] {
        match read_trimmed(path) {
            Ok(v) => *slot = Some(v),
            Err(why) => id.unreadable.push(why),
        }
    }

    match std::fs::read_to_string("/proc/self/status") {
        Ok(text) => {
            for line in text.lines() {
                let Some((key, value)) = line.split_once(':') else {
                    continue;
                };
                let value = value.trim().to_string();
                // ⚠ Selected by NAME. docs/conventions/code.md: anything
                // consuming structured output selects by name, never position.
                let slot = match key {
                    "CapEff" => &mut id.cap_eff,
                    "CapPrm" => &mut id.cap_prm,
                    "CapBnd" => &mut id.cap_bnd,
                    "Seccomp" => &mut id.seccomp,
                    "Seccomp_filters" => &mut id.seccomp_filters,
                    "NoNewPrivs" => &mut id.no_new_privs,
                    "Threads" => &mut id.threads,
                    _ => continue,
                };
                *slot = Some(value);
            }
        }
        Err(e) => id.unreadable.push(format!("/proc/self/status: {e}")),
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_range_of_one_id_is_recognised() {
        // The target's map, `references/Azathothas__container-research/tree/verification/real/identity.txt`.
        assert!(is_single_id_map("0 1000 1"));
        assert!(is_single_id_map("         0       1000          1"));
    }

    #[test]
    fn a_full_map_and_a_multi_range_map_are_not() {
        assert!(!is_single_id_map("0 0 4294967295"));
        assert!(!is_single_id_map("0 1000 1\n1 100000 65536"));
        assert!(!is_single_id_map(""));
    }

    #[test]
    fn this_process_can_read_its_own_maps() {
        // The production default of the seam, not an injected double:
        // docs/conventions/code.md requires the shipping branch be tested.
        let id = read();
        assert!(id.uid >= 0);
        assert!(id.cap_eff.is_some(), "unreadable: {:?}", id.unreadable);
    }
}
