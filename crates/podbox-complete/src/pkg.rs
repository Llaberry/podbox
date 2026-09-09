//! The package managers, and the three walls each of them hits that are not
//! about the network.
//!
//! - [`pacman`] -- [`TODO/complete.md`](../../../TODO/complete.md) T-0406,
//!   `DownloadUser` and the keyring;
//! - [`apt`] -- T-0407, the sandbox user and the CA bundle;
//! - [`zypper`] -- T-0408, ⭐ **the RIS index and not `repos.d`**;
//! - T-0409 is a rule rather than a fixup and it is enforced elsewhere: podbox
//!   never translates a package manager's exit status. See
//!   [`crate`]'s third rule and [`TODO/cli.md`](../../../TODO/cli.md) T-0802.
//!
//! ⚠ The `http://` half of every one of these is [`crate::sources`], T-0411.
//! Splitting them is deliberate: the protocol fixup is one rule applied to
//! every distribution, and these are three distributions' own walls.

use crate::write::{Kind, Root};
use crate::{Action, Fixup, Options, Report, Result};

pub(crate) fn apply(root: &Root, opts: &Options, r: &mut Report) {
    crate::record(r, "T-0406", "pacman", pacman(root));
    crate::record(r, "T-0407", "apt", apt(root, opts));
    crate::record(r, "T-0408", "zypper", zypper(root, opts));
    // ⭐ Every rootfs, not only a Debian- or SUSE-family one: an https package
    // source is not a property of the package manager, and alpine, arch, fedora
    // and void all hit the same wall.
    crate::record(r, "T-0407", "ca-bundle", ca_bundle(root, "T-0407", opts));
}

// ---------------------------------------------------------------- T-0406
//
// `pacman -Sy` fails with `failed to chown temporary download directory ...:
// Invalid argument`. `DownloadUser = alpm` names a uid this runtime cannot map
// and the errno is the section 9.1 wall again.
//
// ⛔ Creating the `alpm` user does not help: the id still has no mapping, so
// the chown still returns EINVAL. The config line is what has to go.
//
// ⚠ CheckSpace is deliberately NOT enabled. It is what makes `/etc/mtab`
// load-bearing (T-0405), and Arch ships it commented out.

fn pacman(root: &Root) -> Result<Vec<Fixup>> {
    let path = "etc/pacman.conf";
    let Some(text) = root.read_text(path)? else {
        return Ok(vec![Fixup::new("T-0406", "pacman", path, Action::Skipped)
            .why("no pacman.conf: this is not an Arch-family rootfs")]);
    };
    let mut out = Vec::new();
    let (new, hits) = comment_out(&text, "DownloadUser");
    if hits == 0 {
        out.push(
            Fixup::new("T-0406", "pacman", path, Action::Unchanged)
                .why("no active DownloadUser line, so pacman already downloads as root"),
        );
    } else {
        let w = root.write(path, new.as_bytes(), 0o644)?;
        out.push(
            Fixup::new("T-0406", "pacman", path, crate::act(w)).why(format!(
                "commented out {hits} DownloadUser line(s). It names a uid this runtime \
             cannot map, so pacman's chown of its download directory returns EINVAL \
             and the whole transaction fails"
            )),
        );
    }

    // ⭐ The keyring half is a READING rather than an edit. `pacman-key --init`
    // and `--populate` run gpg inside the rootfs, which the completion layer
    // cannot do from the host, so podbox reports the state and the caller can
    // act on it. ⚠ Measured on the pinned `archlinux` image: it ships an
    // initialized keyring, so this is a diagnostic for a rootfs that does not.
    let initialized = !root.kind("etc/pacman.d/gnupg/trustdb.gpg")?.is_missing();
    out.push(if initialized {
        Fixup::new(
            "T-0406",
            "pacman-keyring",
            "etc/pacman.d/gnupg",
            Action::Unchanged,
        )
        .why("the image ships an initialized keyring, so signed packages verify")
    } else {
        Fixup::new(
            "T-0406",
            "pacman-keyring",
            "etc/pacman.d/gnupg",
            Action::Skipped,
        )
        .why(
            "this rootfs has no initialized keyring and podbox cannot run \
                 pacman-key from outside the chroot. Run `pacman-key --init && \
                 pacman-key --populate` inside the container before installing \
                 signed packages",
        )
        .degraded()
    });
    Ok(out)
}

/// Comment out every uncommented line whose first word is `key`, and say how
/// many. ⚠ The first word, not a substring: `#DownloadUser` is already
/// commented and `SomeDownloadUserThing` is a different key.
fn comment_out(text: &str, key: &str) -> (String, usize) {
    let mut hits = 0;
    let mut out = String::with_capacity(text.len() + 32);
    for line in text.split_inclusive('\n') {
        let t = line.trim_start();
        let first = t.split(|c: char| c.is_whitespace() || c == '=').next();
        if first == Some(key) {
            hits += 1;
            out.push_str("# podbox (T-0406): ");
            out.push_str(line);
        } else {
            out.push_str(line);
        }
    }
    (out, hits)
}

// ---------------------------------------------------------------- T-0407
//
// Two failures. `_apt` is an unmapped user and apt drops privileges to it for
// downloads. And the image may have no CA bundle, which the https rewrite in
// T-0411 then needs.
//
// ⛔ A DROP-IN, never an edit of `apt.conf`: the fixup is then visible,
// removable and idempotent, and an image that already sets the key is not
// clobbered.

const APT_DROP_IN: &str = "etc/apt/apt.conf.d/99podbox";

const APT_DROP_IN_BODY: &str = "\
// Written by podbox (TODO/complete.md T-0407). apt drops privileges to the
// `_apt` user for downloads, and that uid is not mapped on this runtime, so
// every fetch fails with a permission error that names the method rather than
// the id. podbox runs as uid 0 and cannot become anything else.
APT::Sandbox::User \"root\";
";

fn apt(root: &Root, _opts: &Options) -> Result<Vec<Fixup>> {
    if root.kind("etc/apt")?.is_missing() {
        return Ok(vec![Fixup::new(
            "T-0407",
            "apt",
            "etc/apt",
            Action::Skipped,
        )
        .why("no /etc/apt: this is not a Debian-family rootfs")]);
    }
    let mut out = Vec::new();
    let w = root.write(APT_DROP_IN, APT_DROP_IN_BODY.as_bytes(), 0o644)?;
    out.push(
        Fixup::new("T-0407", "apt-sandbox", APT_DROP_IN, crate::act(w))
            .why("APT::Sandbox::User root. The _apt user is not mapped on this runtime"),
    );
    Ok(out)
}

/// Make an `https://` package source verify, on a machine whose egress is
/// intercepted.
///
/// ⛔ **Never disable certificate verification to make https work.** If no
/// bundle can be found that is a named refusal, and the scheme rewrite in
/// T-0411 is what then does not happen.
///
/// ⭐ **Two cases, and the second is what the runtimes podbox targets are.**
///
/// 1. **the image ships no bundle**: install this machine's, so an `https://`
///    source has something to verify against;
/// 2. **the image ships one and THIS MACHINE ANNOUNCED ITS OWN** through
///    `$SSL_CERT_FILE`, `$CURL_CA_BUNDLE` or `$REQUESTS_CA_BUNDLE`: append it.
///    ⚠ Measured on 2026-09-09 on a host whose egress is intercepted:
///    `apk add gcc` inside `alpine:3.20` fails with `certificate verify
///    failed`, and it fails **identically under docker**, so this is a property
///    of the machine rather than of either runtime. Every other tool on such a
///    machine -- `curl`, `python`, `node`, `cargo` -- already reads those
///    variables; a container that does not is the anomaly.
///
/// ⛔ **A machine that did NOT announce one gets nothing appended.** A bundle
/// found only at a default path is that distribution's own trust store and says
/// nothing about interception, and adding a root to somebody else's image on
/// that basis would be a trust change nobody asked for. `--no-host-cas` refuses
/// it in the announced case too.
///
/// ⚠ The candidate list is [`podbox_image::tls`]'s, because podbox's own
/// transport already had to answer this question and two lists would drift.
const BUNDLE_PATHS: &[&str] = &[
    "etc/ssl/certs/ca-certificates.crt",
    "etc/pki/tls/certs/ca-bundle.crt",
    "etc/ssl/ca-bundle.pem",
    "etc/ssl/cert.pem",
    "etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
];

/// The line that makes the append idempotent and auditable. ⛔ A marker rather
/// than a byte comparison: the appended block has to be findable by a human
/// reading the file inside the container, and re-appending it on every run
/// would grow the bundle without bound.
pub const HOST_CA_MARKER: &str = "# podbox (TODO/complete.md T-0407): this machine's own CA bundle";

fn ca_bundle(root: &Root, entry: &'static str, opts: &Options) -> Result<Vec<Fixup>> {
    // Where the image already keeps one, and the default for an image with none.
    // ⚠ Read THROUGH a link: `/etc/ssl/ca-bundle.pem` is a symlink to
    // `/var/lib/ca-certificates/ca-bundle.pem` on openSUSE, and a non-following
    // read reports "no bundle here" for a store the image very much has.
    let mut present: Vec<String> = Vec::new();
    for p in BUNDLE_PATHS {
        if root
            .read_following(p)?
            .map(|b| !b.is_empty())
            .unwrap_or(false)
        {
            present.push((*p).to_string());
        }
    }
    let host = podbox_image::tls::host_bundle_bytes();

    if present.is_empty() {
        let Some(h) = host else {
            return Ok(vec![Fixup::new(
                entry,
                "ca-bundle",
                BUNDLE_PATHS[0],
                Action::Skipped,
            )
            .why(
                "the image has no CA bundle and this machine has none podbox can \
                 find, so an https source in it will not verify. ⛔ podbox does \
                 not disable verification to make one work",
            )
            .degraded()]);
        };
        // ⛔ **EVERY conventional location whose directory is already there, not
        // one.** Measured on 2026-09-09 against `ghcr.io/void-linux/void-musl`:
        // it ships `/etc/ssl/certs/` as a HASHED DIRECTORY with no bundle file
        // in it, and OpenSSL's default `CAfile` is `$OPENSSLDIR/cert.pem` =
        // `/etc/ssl/cert.pem`. Writing only `ca-certificates.crt` put the roots
        // somewhere `xbps`'s libfetch never looks, and every fetch failed with
        // `SSL_connect returned 1`.
        //
        // ⚠ The directory has to exist already: podbox writes where the image's
        // TLS stack looks, and inventing `/etc/pki/...` in an image that has no
        // such tree is podbox guessing.
        let mut out = Vec::new();
        for dest in BUNDLE_PATHS {
            let Some((dir, _)) = dest.rsplit_once('/') else {
                continue;
            };
            if !matches!(root.kind(dir)?, Kind::Dir) {
                continue;
            }
            let Some(w) = root.write_following(dest, &h.bytes, 0o644)? else {
                out.push(
                    Fixup::new(entry, "ca-bundle", dest, Action::Failed)
                        .why(
                            "this kernel has no openat2(2), so podbox cannot write \
                             through a symlinked trust store while proving it stays \
                             inside the rootfs",
                        )
                        .degraded(),
                );
                continue;
            };
            out.push(
                Fixup::new(entry, "ca-bundle", dest, crate::act(w)).why(format!(
                    "the image had no bundle here; installed this machine's, from \
                     {}, {} bytes",
                    h.path,
                    h.bytes.len()
                )),
            );
        }
        if out.is_empty() {
            // Not even `/etc/ssl` exists: a distroless or scratch image.
            let w = root.write(BUNDLE_PATHS[0], &h.bytes, 0o644)?;
            out.push(
                Fixup::new(entry, "ca-bundle", BUNDLE_PATHS[0], crate::act(w)).why(format!(
                    "this image carries no TLS directory at all; installed this \
                     machine's bundle at the conventional path, from {}",
                    h.path
                )),
            );
        }
        return Ok(out);
    }

    let Some(h) = host else {
        return Ok(vec![Fixup::new(
            entry,
            "ca-bundle",
            &present.join(", "),
            Action::Unchanged,
        )
        .why(
            "the image ships its own bundle and this machine has none to add",
        )]);
    };
    let Some(var) = h.announced_by.clone() else {
        return Ok(vec![Fixup::new(
            entry,
            "ca-bundle",
            &present.join(", "),
            Action::Unchanged,
        )
        .why(
            "the image ships its own bundle, and this machine's came from a \
                 default path rather than from $SSL_CERT_FILE, $CURL_CA_BUNDLE or \
                 $REQUESTS_CA_BUNDLE. ⛔ That is no announcement, so podbox adds \
                 nothing to somebody else's trust store",
        )]);
    };
    if !opts.host_cas {
        return Ok(vec![Fixup::new(
            entry,
            "ca-bundle",
            &present.join(", "),
            Action::Skipped,
        )
        .why(format!(
            "--no-host-cas: this machine announced its own CA bundle in ${var} \
                 and podbox left the image's trust store alone. ⚠ An https package \
                 source will fail to verify here if this machine intercepts TLS"
        ))
        .degraded()]);
    }

    // ⛔ **EVERY conventional path, not only the ones the image already has.**
    // Measured on 2026-09-09 against `ghcr.io/void-linux/void-musl`: it ships
    // `/etc/ssl/certs/ca-certificates.crt`, so the append landed there and
    // `xbps` still failed with `SSL_connect returned 1`, because libfetch's
    // OpenSSL reads its default `CAfile`, `$OPENSSLDIR/cert.pem` =
    // `/etc/ssl/cert.pem`, which the image does not ship. A bundle in the wrong
    // file is a bundle nothing reads.
    //
    // ⚠ Only where the DIRECTORY is already there: inventing `/etc/pki/...` in an
    // image with no such tree is podbox guessing at a layout.
    let mut targets = present.clone();
    for cand in BUNDLE_PATHS {
        if targets.iter().any(|p| p == cand) {
            continue;
        }
        let Some((dir, _)) = cand.rsplit_once('/') else {
            continue;
        };
        if matches!(root.kind(dir)?, Kind::Dir) {
            targets.push((*cand).to_string());
        }
    }

    let mut out = Vec::new();
    for p in targets {
        let existing = root
            .read_following(&p)?
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
        if existing.contains(HOST_CA_MARKER) {
            out.push(
                Fixup::new(entry, "ca-bundle", &p, Action::Unchanged)
                    .why("this machine's bundle is already appended here"),
            );
            continue;
        }
        // ⛔ APPENDED, never replaced: the image's own roots stay, and the file
        // says where the extra block came from so a reader inside the container
        // can see it and remove it.
        let mut merged = existing.into_bytes();
        if !merged.is_empty() && !merged.ends_with(b"\n") {
            merged.push(b'\n');
        }
        merged.extend_from_slice(
            format!("{HOST_CA_MARKER}, announced in ${var} as {}\n", h.path).as_bytes(),
        );
        merged.extend_from_slice(&h.bytes);
        let Some(w) = root.write_following(&p, &merged, 0o644)? else {
            out.push(
                Fixup::new(entry, "ca-bundle", &p, Action::Failed)
                    .why(
                        "this kernel has no openat2(2), so podbox cannot write through \
                         a symlinked trust store while proving it stays inside the \
                         rootfs",
                    )
                    .degraded(),
            );
            continue;
        };
        out.push(
            Fixup::new(entry, "ca-bundle", &p, crate::act(w))
                .why(format!(
                    "APPENDED this machine's CA bundle ({} bytes, announced in ${var} \
                     as {}). The image's own roots are kept and the added block is \
                     marked. ⚠ This machine's trust is now the container's too; \
                     --no-host-cas refuses it",
                    h.bytes.len(),
                    h.path
                ))
                .degraded(),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------- T-0408
//
// ⭐ A naive edit to `/etc/zypp/repos.d/` REVERTS. `refresh-services`
// regenerates that directory from the RIS index and overwrites whatever was
// written there, so the fixup edits the source of the generated file.
//
// ⚠ The rewrite itself is T-0411's, applied to both directories. What is here
// is the discovery of the index and the report that says which of the two
// podbox actually reached, because "edited repos.d" and "edited the service
// definitions" are different claims.

pub const ZYPP_SERVICE_DIRS: &[&str] = &["usr/share/zypp/local/service", "etc/zypp/services.d"];

fn zypper(root: &Root, _opts: &Options) -> Result<Vec<Fixup>> {
    if root.kind("etc/zypp")?.is_missing() {
        return Ok(vec![Fixup::new(
            "T-0408",
            "zypper",
            "etc/zypp",
            Action::Skipped,
        )
        .why("no /etc/zypp: this is not a SUSE-family rootfs")]);
    }
    let mut found = Vec::new();
    for d in ZYPP_SERVICE_DIRS {
        if !root.list(d)?.is_empty() {
            found.push(*d);
        }
    }
    let out = vec![if found.is_empty() {
        Fixup::new(
            "T-0408",
            "zypper-index",
            "etc/zypp/repos.d",
            Action::Skipped,
        )
        .why(
            "this rootfs carries no RIS service index, so repos.d is not \
             regenerated and editing it in place holds",
        )
    } else {
        Fixup::new(
            "T-0408",
            "zypper-index",
            &found.join(", "),
            Action::Unchanged,
        )
        .why(
            "a RIS service index is present. ⚠ Any scheme fixup (T-0411) is \
             applied HERE as well as to repos.d, or `zypper refresh-services` \
             regenerates repos.d from the uncorrected index and reverts it",
        )
    }];
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        let p = std::env::temp_dir().join(format!("podbox-pkg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("etc")).unwrap();
        p.to_string_lossy().to_string()
    }

    /// ⚠ The first word, not a substring. An already-commented line must not be
    /// counted or commented twice.
    #[test]
    fn comment_out_matches_the_key_and_not_a_substring() {
        let src = "\
[options]
#DownloadUser = alpm
DownloadUser = alpm
DownloadUserAgent = x
  DownloadUser=alpm
";
        let (got, hits) = comment_out(src, "DownloadUser");
        assert_eq!(hits, 2, "{got}");
        assert!(
            got.contains("# podbox (T-0406): DownloadUser = alpm\n"),
            "{got}"
        );
        assert!(got.contains("DownloadUserAgent = x\n"), "{got}");
        assert!(!got.contains("# podbox (T-0406): #DownloadUser"), "{got}");
    }

    #[test]
    fn pacman_conf_absent_is_skipped_rather_than_failed() {
        let d = scratch("nopacman");
        let root = Root::open(&d).unwrap();
        let fs = pacman(&root).unwrap();
        assert_eq!(fs[0].action, Action::Skipped);
        assert!(!fs[0].degraded);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_apt_drop_in_is_idempotent() {
        let d = scratch("apt");
        std::fs::create_dir_all(format!("{d}/etc/apt")).unwrap();
        let root = Root::open(&d).unwrap();
        assert_eq!(
            apt(&root, &Options::default()).unwrap()[0].action,
            Action::Created
        );
        assert_eq!(
            apt(&root, &Options::default()).unwrap()[0].action,
            Action::Unchanged
        );
        let got = std::fs::read_to_string(format!("{d}/{APT_DROP_IN}")).unwrap();
        assert!(got.contains("APT::Sandbox::User \"root\";"), "{got}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ A rootfs with no CA bundle gets one, or podbox says it could not.
    /// ⚠ Whichever arm this host takes, the rule asserted is the same: podbox
    /// either installs a real bundle or says so, and never turns verification
    /// off.
    #[test]
    fn a_rootfs_with_no_ca_bundle_gets_one_or_is_told_why_not() {
        let d = scratch("ca");
        let root = Root::open(&d).unwrap();
        let fs = ca_bundle(&root, "T-0407", &Options::default()).unwrap();
        let ca = &fs[0];
        assert!(
            matches!(ca.action, Action::Created | Action::Skipped),
            "{ca:?}"
        );
        assert!(!ca.detail.contains("verify=false"), "{ca:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⭐ The append is idempotent and it KEEPS the image's own roots. A second
    /// run must not grow the file, which is what a byte comparison instead of a
    /// marker would have done.
    #[test]
    fn an_announced_host_bundle_is_appended_once_and_the_image_roots_survive() {
        let d = scratch("caappend");
        std::fs::create_dir_all(format!("{d}/etc/ssl/certs")).unwrap();
        std::fs::write(
            format!("{d}/etc/ssl/certs/ca-certificates.crt"),
            "# THE IMAGE OWN ROOTS\n",
        )
        .unwrap();
        // ⚠ A real PEM, because `host_bundle_bytes` returns a path only where it
        // parses as at least one certificate.
        let host = std::env::temp_dir().join(format!("podbox-hostca-{}.pem", std::process::id()));
        std::fs::write(&host, SELF_SIGNED_PEM).unwrap();
        std::env::set_var("SSL_CERT_FILE", &host);

        let root = Root::open(&d).unwrap();
        let first = ca_bundle(&root, "T-0407", &Options::default()).unwrap();
        let second = ca_bundle(&root, "T-0407", &Options::default()).unwrap();
        std::env::remove_var("SSL_CERT_FILE");

        let got =
            std::fs::read_to_string(format!("{d}/etc/ssl/certs/ca-certificates.crt")).unwrap();
        assert_eq!(first[0].action, Action::Rewrote, "{:?}", first[0]);
        assert_eq!(second[0].action, Action::Unchanged, "{:?}", second[0]);
        assert!(
            got.contains("# THE IMAGE OWN ROOTS"),
            "the image roots were dropped"
        );
        assert_eq!(got.matches(HOST_CA_MARKER).count(), 1, "appended twice");
        assert!(first[0].degraded, "a trust change is a degradation");
        let _ = std::fs::remove_file(&host);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ `--no-host-cas` refuses, and says what the caller gave up.
    #[test]
    fn no_host_cas_leaves_the_image_trust_store_byte_identical() {
        let d = scratch("canohost");
        std::fs::create_dir_all(format!("{d}/etc/ssl/certs")).unwrap();
        let p = format!("{d}/etc/ssl/certs/ca-certificates.crt");
        std::fs::write(&p, "# THE IMAGE OWN ROOTS\n").unwrap();
        let host = std::env::temp_dir().join(format!("podbox-hostca2-{}.pem", std::process::id()));
        std::fs::write(&host, SELF_SIGNED_PEM).unwrap();
        std::env::set_var("SSL_CERT_FILE", &host);
        let root = Root::open(&d).unwrap();
        let fs = ca_bundle(
            &root,
            "T-0407",
            &Options {
                host_cas: false,
                ..Options::default()
            },
        )
        .unwrap();
        std::env::remove_var("SSL_CERT_FILE");
        assert_eq!(fs[0].action, Action::Skipped);
        assert!(fs[0].degraded);
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "# THE IMAGE OWN ROOTS\n"
        );
        let _ = std::fs::remove_file(&host);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⚠ Any certificate at all; nothing here checks its contents, only that
    /// `rustls_pemfile` finds one, which is what `host_bundle_bytes` requires.
    const SELF_SIGNED_PEM: &str = "\
-----BEGIN CERTIFICATE-----
MIIBITCBxaADAgECAgEBMAoGCCqGSM49BAMCMA8xDTALBgNVBAMMBHRlc3QwHhcN
MjQwMTAxMDAwMDAwWhcNMzQwMTAxMDAwMDAwWjAPMQ0wCwYDVQQDDAR0ZXN0MFkw
EwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEwqQ0KJp8yQ4XKm1UGT1qkPD0iCJ0V7gI
Hs9pTfl0RCgW8gJ0KsCTb2xkPz8dK0FtnZL7lS+7CqvBqYc0z0m4hqMdMBswDAYD
VR0TAQH/BAIwADALBgNVHQ8EBAMCBeAwCgYIKoZIzj0EAwIDSAAwRQIhAP0J7d1z
bB0hLYnzMkGqXn6c8Q9d8sZmT7d5m7z1KQvVAiA5cBqAqCkTLh3n3sVh6Nn3F8mR
7dZQeK1p3QAoLtQ9tA==
-----END CERTIFICATE-----
";
}
