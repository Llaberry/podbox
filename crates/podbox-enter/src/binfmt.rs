//! Whether this machine can execute a foreign-architecture payload, and how.
//!
//! [`TODO/enter.md`](../../../TODO/enter.md) T-0506.
//!
//! ⭐ **This is the mechanism docker and podman use, read rather than assumed.**
//! `binfmt_misc` lets the kernel hand an ELF it cannot execute to a registered
//! interpreter, which for a foreign architecture is `qemu-user`. podbox reads
//! the registrations out of `/proc/sys/fs/binfmt_misc` and reports what it
//! found; it never registers one, because registering is a machine-wide change
//! and podbox is not the machine's owner.
//!
//! ⛔ **The `F` flag is the whole question for a chroot.** Without it the
//! kernel opens the interpreter **by path at exec time**, which is a path inside
//! the new root, so a chroot with no qemu in it fails. With it the interpreter
//! is opened when the registration is made and held, so it works inside any
//! chroot. Measured on 2026-09-09, `experiments/results/multiarch.txt` clause 5:
//! an `F` registration ran an aarch64 binary in a bare chroot containing
//! nothing but that binary.

use std::path::{Path, PathBuf};

/// Where the kernel exposes its registrations.
pub const BINFMT_DIR: &str = "/proc/sys/fs/binfmt_misc";

/// One registration, as podbox reads it.
#[derive(Debug, Clone)]
pub struct Registration {
    pub name: String,
    pub interpreter: String,
    pub enabled: bool,
    /// ⭐ The `F` flag: the interpreter is opened at registration time and held
    /// open, so it is reachable from inside a chroot that does not contain it.
    pub fix_binary: bool,
    /// The ELF `e_machine` the magic selects on, where podbox could read one.
    pub machine: Option<u16>,
}

/// What podbox can say about running `want` on this host.
#[derive(Debug, Clone)]
pub enum Support {
    /// The payload is this machine's own architecture.
    Native,
    /// A registered interpreter carrying `F`: it works inside the chroot.
    Interpreter(Registration),
    /// A registered interpreter WITHOUT `F`. ⚠ It works only if the interpreter
    /// is reachable by its own path inside the rootfs, which is the copy-in.
    InterpreterNeedsCopyIn(Registration),
    /// Nothing is registered for this architecture.
    None { why: String },
}

/// The ELF machine number for an OCI architecture name.
///
/// ⚠ These are the ELF specification's `EM_*` values, and they are what a
/// `binfmt_misc` magic actually matches on. Mapping an OCI name to one is the
/// only way to answer "is there an interpreter for THIS image" rather than
/// "is there any interpreter at all".
pub fn machine_for(arch: &str) -> Option<u16> {
    Some(match arch {
        "amd64" => 0x3e,
        "386" => 0x03,
        "arm64" => 0xb7,
        "arm" => 0x28,
        "riscv64" => 0xf3,
        "ppc64le" | "ppc64" => 0x15,
        "s390x" => 0x16,
        "loong64" => 0x102,
        "mips64le" | "mips64" => 0x08,
        _ => return None,
    })
}

fn parse_one(path: &Path) -> Option<Registration> {
    let text = std::fs::read_to_string(path).ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let mut interpreter = String::new();
    let mut enabled = false;
    let mut flags = String::new();
    let mut magic = String::new();
    let mut offset: usize = 0;
    for line in text.lines() {
        let line = line.trim();
        if line == "enabled" {
            enabled = true;
        } else if let Some(v) = line.strip_prefix("interpreter ") {
            interpreter = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("flags:") {
            flags = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("magic ") {
            magic = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("offset ") {
            offset = v.trim().parse().unwrap_or(0);
        }
    }
    // ⚠ `e_machine` is a little-endian u16 at byte 18 of an ELF header, so it
    // is hex characters 36 and 37 of the magic when the registration starts at
    // offset 0. A magic shorter than that does not select on the architecture
    // at all, which is worth knowing rather than guessing at.
    let machine = {
        let start = 18usize.checked_sub(offset).map(|b| b * 2);
        match start {
            Some(s) if magic.len() >= s + 4 => {
                let lo = u8::from_str_radix(&magic[s..s + 2], 16).ok();
                let hi = u8::from_str_radix(&magic[s + 2..s + 4], 16).ok();
                match (lo, hi) {
                    (Some(l), Some(h)) => Some(u16::from(l) | (u16::from(h) << 8)),
                    _ => None,
                }
            }
            _ => None,
        }
    };
    Some(Registration {
        name,
        interpreter,
        enabled,
        fix_binary: flags.contains('F'),
        machine,
    })
}

/// Every enabled registration this machine carries.
pub fn registrations() -> Vec<Registration> {
    let dir = match std::fs::read_dir(BINFMT_DIR) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for e in dir.flatten() {
        let name = e.file_name();
        // ⚠ `register` and `status` are the control files, not registrations.
        if name == "register" || name == "status" {
            continue;
        }
        if let Some(r) = parse_one(&e.path()) {
            if r.enabled {
                out.push(r);
            }
        }
    }
    out
}

/// Whether this machine can run `arch`, and how.
///
/// ⛔ Never a guess. Where nothing is registered the answer carries **why**, so
/// the refusal names the remedy rather than the symptom.
pub fn support_for(arch: &str, host_arch: &str) -> Support {
    if arch == host_arch {
        return Support::Native;
    }
    let want = match machine_for(arch) {
        Some(m) => m,
        None => {
            return Support::None {
                why: format!(
                    "podbox has no ELF machine number for the architecture {arch:?}, so it \
                     cannot tell whether an interpreter is registered for it"
                ),
            }
        }
    };
    if !Path::new(BINFMT_DIR).is_dir() {
        return Support::None {
            why: format!(
                "{BINFMT_DIR} is not mounted, so this kernel has no interpreter \
                 registrations at all. `mount -t binfmt_misc none {BINFMT_DIR}` \
                 exposes them where the kernel was built with CONFIG_BINFMT_MISC"
            ),
        };
    }
    let all = registrations();
    for r in &all {
        if r.machine == Some(want) {
            return if r.fix_binary {
                Support::Interpreter(r.clone())
            } else {
                Support::InterpreterNeedsCopyIn(r.clone())
            };
        }
    }
    Support::None {
        why: format!(
            "no enabled binfmt_misc registration selects ELF machine 0x{want:x} ({arch}). \
             {} registration(s) are enabled and none of them match. Installing \
             qemu-user-static and registering it is what docker and podman rely on too",
            all.len()
        ),
    }
}

/// The interpreter's path inside the rootfs, for the copy-in.
pub fn copy_in_target(root: &str, interpreter: &str) -> PathBuf {
    Path::new(root).join(interpreter.trim_start_matches('/'))
}

/// Copy a `binfmt_misc` interpreter into a rootfs.
///
/// ⛔ **This writes a file into somebody else's image and the caller MUST
/// disclose it.** T-0506: podbox's honesty rules have no exception for a helpful
/// edit, and the payload can see this file.
///
/// ⚠ Only ever called for a registration WITHOUT `F`. With `F` the kernel holds
/// the interpreter open already and copying it in would be a write with no
/// purpose.
pub fn copy_interpreter_in(root: &str, r: &Registration) -> std::io::Result<PathBuf> {
    let dest = copy_in_target(root, &r.interpreter);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // ⚠ Skipped where it is already there and the same size: re-copying a
    // 10 MB static qemu on every `run` is a cost with no benefit, and the
    // interpreter is a fixed artefact rather than something that drifts.
    if let (Ok(a), Ok(b)) = (std::fs::metadata(&r.interpreter), std::fs::metadata(&dest)) {
        if a.len() == b.len() {
            return Ok(dest);
        }
    }
    std::fs::copy(&r.interpreter, &dest)?;
    // The interpreter has to be executable inside the rootfs.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_architecture_needs_no_interpreter() {
        assert!(matches!(support_for("amd64", "amd64"), Support::Native));
    }

    #[test]
    fn an_architecture_podbox_cannot_name_says_so_rather_than_guessing() {
        let s = support_for("nosucharch", "amd64");
        match s {
            Support::None { why } => assert!(why.contains("no ELF machine number"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_elf_machine_numbers_are_the_specifications() {
        // ⛔ Measured against real binaries rather than recalled: 0xb7 is what
        // `readelf -h` prints for an aarch64 ELF and 0x3e for x86-64, and
        // experiments/260-multiarch.sh registers a magic carrying 0xb7.
        assert_eq!(machine_for("arm64"), Some(0xb7));
        assert_eq!(machine_for("amd64"), Some(0x3e));
        assert_eq!(machine_for("riscv64"), Some(0xf3));
        assert_eq!(machine_for("nope"), None);
    }

    /// ⚠ Reads the real `/proc/sys/fs/binfmt_misc` where it is mounted, and
    /// asserts only what is true either way. A test that required a
    /// registration would fail on a machine that is simply configured
    /// differently, which is a test about the machine and not about podbox.
    #[test]
    fn a_missing_binfmt_directory_is_reported_and_not_a_panic() {
        let s = support_for("arm64", "amd64");
        match s {
            Support::None { why } => {
                assert!(!why.is_empty(), "a refusal with no reason");
            }
            Support::Interpreter(r) | Support::InterpreterNeedsCopyIn(r) => {
                assert!(!r.interpreter.is_empty());
                assert_eq!(r.machine, Some(0xb7));
            }
            Support::Native => panic!("arm64 is not amd64"),
        }
    }
}
