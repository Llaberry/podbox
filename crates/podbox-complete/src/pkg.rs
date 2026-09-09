//! The package managers, and the three walls each of them hits that are not
//! about the network.
//!
//! - [`pacman`] -- [`TODO/complete.md`](../../../TODO/complete.md) T-0406,
//!   `DownloadUser` and the keyring;
//! - [`apt`] -- T-0407, the sandbox user and the CA bundle;
//! - [`zypper`] -- T-0408, ⭐ **the RIS index and not `repos.d`**;
//! - [`ca_hash_dir`] -- T-0412, ⭐ **the CApath libzypp actually reads**;
//! - T-0409 is a rule rather than a fixup and it is enforced elsewhere: podbox
//!   never translates a package manager's exit status. See
//!   [`crate`]'s third rule and [`TODO/cli.md`](../../../TODO/cli.md) T-0802.
//!
//! ⚠ The `http://` half of every one of these is [`crate::sources`], T-0411.
//! Splitting them is deliberate: the protocol fixup is one rule applied to
//! every distribution, and these are three distributions' own walls.

use crate::write::{Kind, Root};
use crate::{Action, Fixup, Options, Report, Result, Step};

pub(crate) fn apply(root: &Root, opts: &Options, r: &mut Report) {
    crate::record_steps(r, "T-0406", "pacman", pacman(root, opts));
    crate::record(r, "T-0407", "apt", apt(root, opts));
    crate::record(r, "T-0408", "zypper", zypper(root, opts));
    // ⭐ Every rootfs, not only a Debian- or SUSE-family one: an https package
    // source is not a property of the package manager, and alpine, arch, fedora
    // and void all hit the same wall.
    crate::record(r, "T-0407", "ca-bundle", ca_bundle(root, "T-0407", opts));
    // ⭐ T-0412, and it is a SECOND trust store rather than a second copy of the
    // first: a CAfile and a hash-indexed CApath are read by different calls, and
    // `zypper` reads only the second.
    crate::record_steps(r, "T-0412", "ca-hash-dir", ca_hash_dir(root, opts));
}

/// Where a program is, inside the rootfs, or `None`.
///
/// ⛔ **Found rather than assumed.** A step names an absolute path this crate
/// has already `stat`ed, so a command announced on the banner cannot turn out
/// not to be there. ⚠ The list is the conventional `bin` directories and not the
/// image's own `PATH`: `PATH` is the payload's environment, and a step is
/// podbox's own command rather than the payload's.
fn find_program(root: &Root, name: &str) -> Result<Option<String>> {
    for dir in ["usr/bin", "usr/sbin", "bin", "sbin", "usr/local/bin"] {
        let rel = format!("{dir}/{name}");
        // ⚠ A symlink counts. `/usr/bin/openssl -> openssl-1.1` is one image's
        // packaging and podbox does not follow it: if it dangles the step fails
        // and says so, which is a better answer than podbox deciding the
        // command is absent because it could not read a link.
        let found = match root.kind(&rel)? {
            Kind::Regular { mode, .. } => mode & 0o111 != 0,
            Kind::Symlink(_) => true,
            _ => false,
        };
        if found {
            return Ok(Some(format!("/{rel}")));
        }
    }
    Ok(None)
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

fn pacman(root: &Root, opts: &Options) -> Result<(Vec<Fixup>, Vec<Step>)> {
    let path = "etc/pacman.conf";
    let Some(text) = root.read_text(path)? else {
        return Ok((
            vec![Fixup::new("T-0406", "pacman", path, Action::Skipped)
                .why("no pacman.conf: this is not an Arch-family rootfs")],
            Vec::new(),
        ));
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

    // ⭐ The keyring half needs a COMMAND: `pacman-key --init` and `--populate`
    // run gpg INSIDE the rootfs, which the completion layer cannot do from the
    // host. T-0412 gave it a step, so podbox asks the caller to run it rather
    // than only printing the two commands a reader would have to type.
    // ⚠ Measured on the pinned `archlinux` image: it ships an initialized
    // keyring, so this is for a rootfs that does not.
    let mut steps = Vec::new();
    let keyring = "etc/pacman.d/gnupg";
    let initialized = !root.kind("etc/pacman.d/gnupg/trustdb.gpg")?.is_missing();
    let key = find_program(root, "pacman-key")?;
    out.push(match (initialized, opts.steps, key) {
        (true, _, _) => Fixup::new("T-0406", "pacman-keyring", keyring, Action::Unchanged)
            .why("the image ships an initialized keyring, so signed packages verify"),
        (false, true, Some(prog)) => {
            for arg in ["--init", "--populate"] {
                steps.push(Step {
                    entry: "T-0406",
                    id: "pacman-keyring",
                    argv: vec![prog.clone(), arg.to_string()],
                    why: format!(
                        "this rootfs ships no initialized keyring, so pacman cannot \
                         verify a signed package. `{prog} {arg}` builds one, and it \
                         runs gpg inside the rootfs, which podbox cannot do from \
                         the host"
                    ),
                });
            }
            Fixup::new("T-0406", "pacman-keyring", keyring, Action::Skipped).why(
                "no initialized keyring here; podbox asks the caller to run \
                 pacman-key --init and --populate inside the rootfs (T-0412's step \
                 mechanism), because gpg cannot be run from outside the chroot",
            )
        }
        (false, true, None) => Fixup::new("T-0406", "pacman-keyring", keyring, Action::Skipped)
            .why(
                "this rootfs has no initialized keyring and no pacman-key to build \
                 one with. Signed packages will not verify",
            )
            .degraded(),
        (false, false, _) => Fixup::new("T-0406", "pacman-keyring", keyring, Action::Skipped)
            .why(
                "--no-steps: this rootfs has no initialized keyring and podbox did \
                 not run pacman-key inside it. Run `pacman-key --init && pacman-key \
                 --populate` in the container before installing signed packages",
            )
            .degraded(),
    });
    Ok((out, steps))
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

// ---------------------------------------------------------------- T-0412
//
// ⭐ **A CAfile and a hash-indexed CApath are two trust stores, and `zypper`
// reads only the second.** Measured on 2026-09-09 inside podbox, on
// `registry.opensuse.org/opensuse/leap:15.6`:
//
//     curl                               https://download.opensuse.org/  -> 200
//     curl --capath /etc/ssl/certs        https://download.opensuse.org/  -> 60
//
// libzypp hands libcurl a `CURLOPT_CAPATH` of `/etc/ssl/certs`, which on
// openSUSE is a symlink to `/var/lib/ca-certificates/pem`. OpenSSL looks a
// certificate up there by the SHA-1 of its canonical DER subject, so every
// CAfile T-0407 writes is invisible to it and a file dropped in under any other
// name is invisible too.
//
// ⛔ **The link cannot be made from the host, so this fixup has two halves.**
// podbox writes one PEM per root (the write), and asks the caller to run
// `openssl rehash` on the directory (the step). ⚠ `update-ca-certificates` is
// openSUSE's own answer and it CANNOT be the step: measured on the same image,
// it exits 1 at `done < <(find ...)` with `/dev/fd/62: No such file or
// directory`. bash's process substitution needs `/dev/fd`, which is
// `/proc/self/fd`, and `/proc` is not mounted inside a chroot -- the same shape
// as `/etc/mtab -> /proc/self/mounts` in T-0405.
//
// ⚠ ONE CERTIFICATE PER FILE, measured rather than assumed: a file holding
// three of them is refused by `openssl rehash` with `it does not contain
// exactly one certificate or CRL`, and nothing is linked.

/// The conventional hash-indexed CApath directories, rootfs-relative.
///
/// ⛔ **Only where the directory is ALREADY hash-indexed.** podbox writes where
/// the image's TLS stack looks; creating a hash directory in an image that has
/// none would be podbox inventing a trust store nothing consults.
pub const CAPATH_DIRS: &[&str] = &["etc/ssl/certs", "etc/pki/tls/certs"];

/// The prefix of every certificate file this fixup writes. ⛔ A prefix rather
/// than a marker line inside the file: the file has to be readable by
/// `openssl rehash` as one certificate, and the name is what a reader inside
/// the container greps for and what podbox itself removes when the machine's
/// bundle shrinks.
pub const HOST_CA_FILE_PREFIX: &str = "podbox-host-ca-";

/// Split a PEM bundle into one text per `CERTIFICATE` block.
///
/// ⚠ Everything outside a block is dropped, which is deliberate: a bundle
/// carries comments, subject lines and T-0407's own marker, and `openssl
/// rehash` wants a file it can read as exactly one certificate.
fn split_pem(bundle: &[u8]) -> Vec<String> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let text = String::from_utf8_lossy(bundle);
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if t == BEGIN {
            cur = Some(String::new());
        }
        if let Some(c) = cur.as_mut() {
            c.push_str(t);
            c.push('\n');
            if t == END {
                out.push(cur.take().expect("just pushed into it"));
            }
        }
    }
    out
}

/// Is `name` a name OpenSSL's hash lookup would find? `01419da9.0`, and the
/// CRL form `01419da9.r0`.
fn is_hash_link(name: &str) -> bool {
    let Some((hash, tail)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 8
        && hash.bytes().all(|b| b.is_ascii_hexdigit())
        && !tail.is_empty()
        && tail
            .trim_start_matches('r')
            .bytes()
            .all(|b| b.is_ascii_digit())
}

/// Where two candidates are the same directory, so it is not written and
/// rehashed twice. ⚠ `/etc/ssl/certs -> /etc/pki/tls/certs` is Fedora's own
/// link and both are in [`CAPATH_DIRS`].
fn dir_key(root: &Root, rel: &str) -> Result<String> {
    Ok(match root.kind(rel)? {
        Kind::Symlink(t) if t.starts_with('/') => t.trim_start_matches('/').to_string(),
        _ => rel.to_string(),
    })
}

fn ca_hash_dir(root: &Root, opts: &Options) -> Result<(Vec<Fixup>, Vec<Step>)> {
    let entry = "T-0412";
    let id = "ca-hash-dir";
    /// One row and no step, which is every arm of this fixup that does not
    /// write. ⚠ A free function rather than a closure so the `degraded` arms
    /// read the same as the quiet ones.
    fn only(path: &str, a: Action, degraded: bool, why: String) -> (Vec<Fixup>, Vec<Step>) {
        let f = Fixup::new("T-0412", "ca-hash-dir", path, a).why(why);
        (vec![if degraded { f.degraded() } else { f }], Vec::new())
    }

    // ⛔ The ANNOUNCEMENT is the discriminator, exactly as in T-0407's
    // `ca_bundle`: a bundle found at a default path says nothing about
    // interception and is no reason to touch somebody else's trust store.
    let Some(h) = podbox_image::tls::host_bundle_bytes() else {
        return Ok(only(
            CAPATH_DIRS[0],
            Action::Skipped,
            false,
            "this machine has no CA bundle podbox can find, so there is nothing to \
             index into the image's CApath"
                .to_string(),
        ));
    };
    let Some(var) = h.announced_by.clone() else {
        return Ok(only(
            CAPATH_DIRS[0],
            Action::Unchanged,
            false,
            "this machine's CA bundle came from a default path rather than from \
             $SSL_CERT_FILE, $CURL_CA_BUNDLE or $REQUESTS_CA_BUNDLE. ⛔ That is no \
             announcement, so podbox adds nothing to somebody else's trust store"
                .to_string(),
        ));
    };

    // Which directories are hash-indexed, deduplicated.
    let mut dirs: Vec<(String, Vec<String>)> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for d in CAPATH_DIRS {
        if !matches!(root.kind(d)?, Kind::Dir | Kind::Symlink(_)) {
            continue;
        }
        let names = root.list(d)?;
        if !names.iter().any(|n| is_hash_link(n)) {
            continue;
        }
        let key = dir_key(root, d)?;
        if keys.contains(&key) {
            continue;
        }
        keys.push(key);
        dirs.push(((*d).to_string(), names));
    }
    if dirs.is_empty() {
        return Ok(only(
            CAPATH_DIRS[0],
            Action::Skipped,
            false,
            "this image carries no hash-indexed CApath directory, so there is none \
             for podbox to index its roots into"
                .to_string(),
        ));
    }
    if !opts.host_cas {
        return Ok(only(
            &dirs[0].0,
            Action::Skipped,
            true,
            format!(
                "--no-host-cas: this machine announced its own CA bundle in ${var} \
                 and podbox left the image's hash-indexed trust store alone. ⚠ A \
                 tool reading a CApath -- libzypp is one -- will fail to verify \
                 here if this machine intercepts TLS"
            ),
        ));
    }

    // ⛔ The program is found BEFORE anything is written. A certificate in a
    // hash directory that nothing has linked is a file no TLS stack reads, and
    // writing 150 of them into somebody else's image for nothing is litter.
    let Some(rehash) = find_program(root, "openssl")? else {
        return Ok(only(
            &dirs[0].0,
            Action::Skipped,
            true,
            "this image has a hash-indexed CApath and no openssl to index it with, \
             so podbox cannot make this machine's roots visible to a tool that \
             reads a CApath rather than a CAfile"
                .to_string(),
        ));
    };
    if !opts.steps {
        return Ok(only(
            &dirs[0].0,
            Action::Skipped,
            true,
            "--no-steps: podbox did not write this machine's roots into the image's \
             hash-indexed CApath, because indexing them needs `openssl rehash` run \
             inside the rootfs. ⚠ A tool reading a CApath will not verify here"
                .to_string(),
        ));
    }

    let announced = split_pem(&h.bytes);
    let mut out = Vec::new();
    let mut steps = Vec::new();
    for (dir, names) in dirs {
        // ⛔ **What the INDEX can reach, and not what the directory contains.**
        // Measured on 2026-09-09 against `ghcr.io/void-linux/void-musl`: T-0407
        // writes the whole announced bundle to
        // `/etc/ssl/certs/ca-certificates.crt`, which is a CAfile sitting inside
        // the CApath, and counting it made every announced root look present
        // while the hash lookup could still find none of them. A CApath is read
        // by NAME, so only a `HASH.N` entry -- and the file a `HASH.N` symlink
        // points at -- is content this directory holds.
        //
        // ⚠ podbox's OWN files are excluded from the comparison, or a second
        // run would find every root already present and delete it as stale.
        let mut reachable: Vec<String> = Vec::new();
        for n in names.iter().filter(|n| is_hash_link(n)) {
            match root.kind(&format!("{dir}/{n}"))? {
                // ⚠ A name and never a path: `openssl rehash` writes a bare
                // name, and a target with a `/` in it is somebody else's
                // convention that podbox does not follow to another directory.
                // ⚠ Many links point at one file, so each name is collected
                // once: `contains` and not `dedup`, which only folds neighbours.
                Kind::Symlink(t) if !t.contains('/') && !reachable.contains(&t) => {
                    reachable.push(t)
                }
                Kind::Regular { .. } if !reachable.contains(n) => reachable.push(n.clone()),
                _ => {}
            }
        }
        let mut held: Vec<String> = Vec::new();
        for t in &reachable {
            if t.starts_with(HOST_CA_FILE_PREFIX) {
                continue;
            }
            if let Some(b) = root.read(&format!("{dir}/{t}"))? {
                held.extend(split_pem(&b));
            }
        }
        let adding: Vec<&String> = announced.iter().filter(|c| !held.contains(c)).collect();

        let mut wrote = 0usize;
        for (i, pem) in adding.iter().enumerate() {
            let rel = format!("{dir}/{HOST_CA_FILE_PREFIX}{i:03}.pem");
            let body = format!(
                "# podbox ({entry}): one root of this machine's own CA bundle, \
                 announced in ${var} as {}, that this image did not already \
                 carry. ⛔ podbox added it to somebody else's trust store; \
                 --no-host-cas refuses it\n{pem}",
                h.path
            );
            if root.write(&rel, body.as_bytes(), 0o644)? != crate::write::Wrote::Unchanged {
                wrote += 1;
            }
        }
        // ⚠ A bundle that SHRANK leaves files nothing wrote this run, and a
        // stale root in a trust store is the one kind of leftover that matters.
        let mut removed = 0usize;
        for n in &names {
            let Some(rest) = n.strip_prefix(HOST_CA_FILE_PREFIX) else {
                continue;
            };
            let keep = rest
                .strip_suffix(".pem")
                .and_then(|d| d.parse::<usize>().ok())
                .is_some_and(|i| i < adding.len());
            if !keep && root.unlink(&format!("{dir}/{n}"))? {
                removed += 1;
            }
        }
        // ⭐ The step is asked for only where the LINKS are missing, so a second
        // container on the same shared rootfs does not pay for a rehash that has
        // already happened. ⛔ Read off the index rather than off "podbox wrote
        // something this run": the rootfs is content-addressed and shared, and a
        // payload that deleted the links must get them back.
        let linked = reachable.iter().any(|t| t.starts_with(HOST_CA_FILE_PREFIX));
        if adding.is_empty() {
            out.push(Fixup::new(entry, id, &dir, Action::Unchanged).why(format!(
                "this image's CApath already holds every one of this machine's {} \
                 announced roots, so there is nothing to add to it",
                announced.len()
            )));
            continue;
        }
        let action = if wrote > 0 || removed > 0 {
            Action::Rewrote
        } else {
            Action::Unchanged
        };
        out.push(
            Fixup::new(entry, id, &dir, action)
                .why(format!(
                    "{} of this machine's {} announced roots are not in this image's \
                     CApath; written as one certificate per file ({}*.pem), {wrote} \
                     changed, {removed} stale one(s) removed. ⚠ A hash-indexed \
                     CApath is read by NAME, so they stay invisible until `openssl \
                     rehash` links them; this machine's trust is then the \
                     container's too",
                    adding.len(),
                    announced.len(),
                    HOST_CA_FILE_PREFIX
                ))
                .degraded(),
        );
        if !linked {
            steps.push(Step {
                entry,
                id,
                argv: vec![rehash.clone(), "rehash".to_string(), format!("/{dir}")],
                why: format!(
                    "/{dir} is a hash-indexed CApath and OpenSSL looks a certificate \
                     up there by the hash of its subject, so the roots podbox just \
                     wrote are invisible until they are linked. libzypp reads this \
                     directory and no CAfile at all"
                ),
            });
        }
    }
    Ok((out, steps))
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

    /// ⛔ **THE TESTS THAT SET `$SSL_CERT_FILE` MAY NOT RUN AT THE SAME TIME**,
    /// and this is what stops them.
    ///
    /// `cargo test` runs tests in THREADS of one process, and an environment
    /// variable belongs to the process. One test's `remove_var` lands in the
    /// middle of another's `ca_bundle`, which then reads THIS MACHINE's real
    /// bundle -- this development container announces one -- and asserts on it.
    /// ⚠ The same shape as `podbox-image`'s two fork tests, which serialise for
    /// the same reason with a different shared thing.
    static ANNOUNCED_BUNDLE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        let (fs, steps) = pacman(&root, &Options::default()).unwrap();
        assert_eq!(fs[0].action, Action::Skipped);
        assert!(!fs[0].degraded);
        // ⛔ And no step: a rootfs that is not Arch-family has no keyring to
        // build, and a step announced on the banner for it would be podbox
        // running `pacman-key` in an image with no pacman.
        assert!(steps.is_empty(), "{steps:?}");
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
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
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
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
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
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
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

    // ------------------------------------------------------------ T-0412

    /// ⛔ Exactly one certificate per file, which is what `openssl rehash`
    /// requires: measured on 2026-09-09, a file holding three of them is
    /// refused with `it does not contain exactly one certificate or CRL` and
    /// NOTHING in the directory is linked.
    #[test]
    fn a_bundle_splits_into_one_certificate_per_block() {
        let bundle = format!("# a comment\n{SELF_SIGNED_PEM}\nsubject=CN=x\n{SELF_SIGNED_PEM}");
        let got = split_pem(bundle.as_bytes());
        assert_eq!(got.len(), 2, "{got:?}");
        for c in &got {
            assert_eq!(c.matches("BEGIN CERTIFICATE").count(), 1, "{c}");
            assert!(!c.contains("a comment"), "{c}");
            assert!(!c.contains("subject="), "{c}");
        }
    }

    /// ⭐ The name OpenSSL's hash lookup would find, and nothing else. A
    /// certificate file podbox writes must NOT read as one, or the stale sweep
    /// would delete a link.
    #[test]
    fn only_a_hash_name_reads_as_a_hash_link() {
        for yes in ["01419da9.0", "002c0b4f.12", "abcdef01.r0"] {
            assert!(is_hash_link(yes), "{yes}");
        }
        for no in [
            "ca-certificates.crt",
            "podbox-host-ca-000.pem",
            "0141.0",
            "01419da9",
            "01419da9.",
            "zzzzzzzz.0",
            "01419da9.pem",
        ] {
            assert!(!is_hash_link(no), "{no}");
        }
    }

    /// A rootfs with an announced host bundle and a hash-indexed CApath, and a
    /// program to index it with.
    fn capath_scratch(name: &str) -> (String, std::path::PathBuf) {
        let d = scratch(name);
        std::fs::create_dir_all(format!("{d}/etc/ssl/certs")).unwrap();
        std::fs::create_dir_all(format!("{d}/usr/bin")).unwrap();
        // ⚠ The image's own root, so the dedup has something to find, plus one
        // name that makes the directory hash-indexed.
        std::fs::write(format!("{d}/etc/ssl/certs/theirs.pem"), SELF_SIGNED_PEM).unwrap();
        std::fs::write(format!("{d}/etc/ssl/certs/01419da9.0"), SELF_SIGNED_PEM).unwrap();
        std::fs::write(format!("{d}/usr/bin/openssl"), b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(
            format!("{d}/usr/bin/openssl"),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
        )
        .unwrap();
        let host = std::env::temp_dir().join(format!("podbox-{name}-{}.pem", std::process::id()));
        std::fs::write(&host, format!("{SELF_SIGNED_PEM}{OTHER_PEM}")).unwrap();
        std::env::set_var("SSL_CERT_FILE", &host);
        (d, host)
    }

    /// ⭐ The whole of T-0412's write half: only the roots the image does NOT
    /// already carry are written, and a step asks for the one command that can
    /// make them findable.
    #[test]
    fn only_the_roots_the_image_lacks_are_written_and_a_rehash_is_asked_for() {
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
        let (d, host) = capath_scratch("cahash");
        let root = Root::open(&d).unwrap();
        let (fs, steps) = ca_hash_dir(&root, &Options::default()).unwrap();
        std::env::remove_var("SSL_CERT_FILE");

        assert_eq!(fs[0].action, Action::Rewrote, "{:?}", fs[0]);
        assert!(fs[0].degraded, "a trust change is a degradation");
        // ⛔ ONE file: the image already holds the other root, and writing it
        // again is a duplicate `openssl rehash` skips and a file nobody needs.
        let written: Vec<String> = std::fs::read_dir(format!("{d}/etc/ssl/certs"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(HOST_CA_FILE_PREFIX))
            .collect();
        assert_eq!(written, vec!["podbox-host-ca-000.pem"], "{written:?}");
        let body = std::fs::read_to_string(format!("{d}/etc/ssl/certs/{}", written[0])).unwrap();
        assert_eq!(body.matches("BEGIN CERTIFICATE").count(), 1, "{body}");
        assert!(body.contains("T-0412"), "{body}");

        assert_eq!(steps.len(), 1, "{steps:?}");
        assert_eq!(
            steps[0].argv,
            vec!["/usr/bin/openssl", "rehash", "/etc/ssl/certs"]
        );
        let _ = std::fs::remove_file(&host);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ A second run does not re-ask for the command once the links are
    /// there, and it does not delete what the first run wrote. ⚠ The rootfs is
    /// content-addressed and SHARED, so this is every container after the first
    /// rather than a rerun nobody does.
    #[test]
    fn a_linked_capath_needs_no_second_rehash_and_keeps_its_files() {
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
        let (d, host) = capath_scratch("cahash2");
        let root = Root::open(&d).unwrap();
        let _ = ca_hash_dir(&root, &Options::default()).unwrap();
        // What `openssl rehash` would have made.
        std::os::unix::fs::symlink(
            "podbox-host-ca-000.pem",
            format!("{d}/etc/ssl/certs/deadbeef.0"),
        )
        .unwrap();
        let (fs, steps) = ca_hash_dir(&root, &Options::default()).unwrap();
        std::env::remove_var("SSL_CERT_FILE");

        assert_eq!(fs[0].action, Action::Unchanged, "{:?}", fs[0]);
        assert!(steps.is_empty(), "{steps:?}");
        assert!(
            std::path::Path::new(&format!("{d}/etc/ssl/certs/podbox-host-ca-000.pem")).exists()
        );
        let _ = std::fs::remove_file(&host);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ `--no-steps` writes NOTHING. A certificate in a hash directory that
    /// nothing has linked is a file no TLS stack reads, so writing it and then
    /// refusing to link it would be litter in somebody else's image.
    #[test]
    fn no_steps_leaves_the_capath_untouched_and_says_what_was_given_up() {
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
        let (d, host) = capath_scratch("cahash3");
        let root = Root::open(&d).unwrap();
        let (fs, steps) = ca_hash_dir(
            &root,
            &Options {
                steps: false,
                ..Options::default()
            },
        )
        .unwrap();
        std::env::remove_var("SSL_CERT_FILE");

        assert_eq!(fs[0].action, Action::Skipped);
        assert!(fs[0].degraded);
        assert!(fs[0].detail.contains("--no-steps"), "{:?}", fs[0]);
        assert!(steps.is_empty());
        assert!(std::fs::read_dir(format!("{d}/etc/ssl/certs"))
            .unwrap()
            .all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(HOST_CA_FILE_PREFIX)));
        let _ = std::fs::remove_file(&host);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ **A CAfile sitting INSIDE the CApath is not content the CApath
    /// holds.** Found on 2026-09-09 against `ghcr.io/void-linux/void-musl`:
    /// T-0407 writes the whole announced bundle to
    /// `/etc/ssl/certs/ca-certificates.crt`, and counting that made every
    /// announced root look present while OpenSSL's hash lookup could still find
    /// none of them. Only a `HASH.N` entry, and the file one points at, is
    /// reachable by name.
    #[test]
    fn a_bundle_inside_the_capath_does_not_count_as_indexed() {
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
        let (d, host) = capath_scratch("cahash5");
        // ⚠ Exactly what T-0407 leaves behind: every announced root, in one
        // file, in the CApath directory, with no hash link to it.
        std::fs::write(
            format!("{d}/etc/ssl/certs/ca-certificates.crt"),
            format!("{SELF_SIGNED_PEM}{OTHER_PEM}"),
        )
        .unwrap();
        let root = Root::open(&d).unwrap();
        let (fs, steps) = ca_hash_dir(&root, &Options::default()).unwrap();
        std::env::remove_var("SSL_CERT_FILE");
        assert_eq!(fs[0].action, Action::Rewrote, "{:?}", fs[0]);
        assert_eq!(steps.len(), 1, "{steps:?}");
        assert!(
            std::path::Path::new(&format!("{d}/etc/ssl/certs/podbox-host-ca-000.pem")).exists(),
            "the root the index cannot reach was treated as present"
        );
        let _ = std::fs::remove_file(&host);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔ A directory that is not hash-indexed is not one podbox invents an
    /// index in: podbox writes where the image's TLS stack already looks.
    #[test]
    fn a_capath_that_is_not_hash_indexed_is_left_alone() {
        let _serialised = ANNOUNCED_BUNDLE.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("cahash4");
        std::fs::create_dir_all(format!("{d}/etc/ssl/certs")).unwrap();
        std::fs::write(
            format!("{d}/etc/ssl/certs/ca-certificates.crt"),
            SELF_SIGNED_PEM,
        )
        .unwrap();
        let host = std::env::temp_dir().join(format!("podbox-cahash4-{}.pem", std::process::id()));
        std::fs::write(&host, OTHER_PEM).unwrap();
        std::env::set_var("SSL_CERT_FILE", &host);
        let root = Root::open(&d).unwrap();
        let (fs, steps) = ca_hash_dir(&root, &Options::default()).unwrap();
        std::env::remove_var("SSL_CERT_FILE");
        assert_eq!(fs[0].action, Action::Skipped);
        assert!(!fs[0].degraded, "nothing was given up: there is no CApath");
        assert!(steps.is_empty());
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

    /// ⚠ A SECOND certificate, and it has to be a different one: T-0412's write
    /// half exists to add the roots an image does not already carry, and a test
    /// whose two roots were the same would pass whether that worked or not.
    /// Generated on 2026-09-09 with
    /// `openssl req -x509 -newkey ec -subj /CN=podbox-test-other`.
    const OTHER_PEM: &str = "\
-----BEGIN CERTIFICATE-----
MIIBjjCCATOgAwIBAgIUKdYowWIy0Gmd3YUnjF0hYcawgf0wCgYIKoZIzj0EAwIw
HDEaMBgGA1UEAwwRcG9kYm94LXRlc3Qtb3RoZXIwHhcNMjYwOTA5MTcyNzIyWhcN
MzYwOTA2MTcyNzIyWjAcMRowGAYDVQQDDBFwb2Rib3gtdGVzdC1vdGhlcjBZMBMG
ByqGSM49AgEGCCqGSM49AwEHA0IABFqqhv1zdvRlJb75xIArQwVSQ7Pi++4g2bJS
62kvM4fB90fq2bo1CpjOAXlYgjzlVJeHC9ECta4eixwVkFaSpGejUzBRMB0GA1Ud
DgQWBBQ2ZA6BxBtrckYJg3zZiHf20uhbljAfBgNVHSMEGDAWgBQ2ZA6BxBtrckYJ
g3zZiHf20uhbljAPBgNVHRMBAf8EBTADAQH/MAoGCCqGSM49BAMCA0kAMEYCIQCD
9AB9GIOuPkGaeew5iCPAGOcB2p4XjCuQoUjogGrGxgIhAOsLORizWLiyWh6/E9Z9
+rP1OdIyI4oIqBMOX8k8x6Az
-----END CERTIFICATE-----
";
}
