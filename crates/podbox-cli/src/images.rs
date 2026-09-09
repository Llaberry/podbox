//! The M1 verbs: `pull`, `images`, `rmi`, `tag`, `image prune`, `inspect`.
//!
//! `TODO/image.md` T-0201 to T-0204 and `TOOL.md` section 6.8. The argument
//! surface lives here and the work lives in `podbox-image`, so a flag is parsed
//! in one place and a registry is spoken to in one place.
//!
//! ⚠ **`{{.Size}}` is what podbox HOLDS, not what docker reports.** docker's
//! `SIZE` column is the sum of the uncompressed layers, and M1 does not extract,
//! so podbox does not know that number. Printing docker's column heading over a
//! different quantity would be a number that was not measured
//! (`docs/AGENTS.md` absolute 3), so the column says `SIZE (STORED)` and the
//! field is documented as the compressed bytes in `blobs/`. It becomes the
//! extracted size when M2 lands, in `TODO/extract.md`.

use std::io::Write;

use podbox_image::error::{Error, EXIT_USAGE};
use podbox_image::{clock, pull, space, Record, Store};

use crate::format;

pub const PULL_USAGE: &str = "\
usage: podbox pull [--platform os/arch[/variant]] <image>

  --tls-verify=B   verify the registry's certificate. Default true. false
                   applies to every registry THIS invocation touches, which is
                   podman's meaning, and it does NOT permit plain HTTP.
  --insecure-registry HOST
                   for HOST only: do not verify its certificate, and fall back
                   to plain HTTP if HTTPS cannot connect. Repeatable. This is
                   docker's flag and it means both things, as docker's does.
                   Also $PODBOX_INSECURE_REGISTRIES (comma separated) and one
                   host per line in $PODBOX_CONFIG, else
                   $XDG_CONFIG_HOME/podbox/registries.conf.

  ⛔ Every use of either is printed on stderr, naming the registry. podbox
    never decides on its own to stop verifying or to speak HTTP: a downgrade
    a caller did not ask for is the one thing an automated caller cannot
    notice.

  --platform P     which manifest to take out of a multi-platform index.
                   Defaults to $PODBOX_DEFAULT_PLATFORM, then to docker's
                   $DOCKER_DEFAULT_PLATFORM, then to the platform this podbox
                   was built for. A bare word is an ARCHITECTURE, as docker
                   reads it: --platform arm64 means linux/arm64.

  ⚠ Pulling a platform this machine cannot execute is allowed and is not a
    warning here: it is what a caller building for another machine wants. What
    refuses is `podbox run`, and only where nothing can execute it.

  Fetch an image and everything it needs into the content-addressed store.
  HTTPS only: a registry that offers only http:// is a named refusal, never a
  downgrade, because tcp/80 egress hangs rather than failing on the runtime
  podbox targets.

  The store is $PODBOX_STORE, else $XDG_DATA_HOME/podbox, else
  $HOME/.local/share/podbox. Blocks AND inodes are checked at it before the
  first layer is fetched, and the refusal names the destination, the free
  amount, the required amount and the unit.
";

pub const EXTRACT_USAGE: &str = "\
usage: podbox extract [--force] <image>

  Unpack a pulled image's layers into a rootfs in the store, and write the
  ownership sidecar beside it. Prints the rootfs path.

  ⛔ Ownership is NEVER restored. chown to an id this machine's user namespace
  does not map returns EINVAL, and that is where five other tools stop. What
  the image intended is recorded in .meta.jsonl beside the rootfs, keyed by
  path; it changes no kernel permission check and is not presented as if it
  does.

  An entry that resolves outside the destination, including through a symlink
  an earlier entry of the same layer created, is refused and the extraction
  fails. A repaired layer cannot be told from a clean one, so podbox does not
  repair one.

  --force          extract again over an existing rootfs
  --platform P     which platform, where the store holds more than one

  ⛔ A reference naming more than one image is REFUSED rather than resolved by
    position. The store holds one record per platform, and picking the most
    recently pulled would unpack an architecture nobody asked for.
";

pub const IMAGES_USAGE: &str = "\
usage: podbox images [options] [image]

  -a, --all        accepted for docker parity; podbox stores no intermediate
                   images, so every image is already listed
  -q, --quiet      print image IDs only, the same as --format '{{.ID}}'
      --digests    show the DIGEST column
      --no-trunc   print full IDs and digests
      --format T   a Go-template-shaped string of {{.Field}} placeholders

  Fields: .Repository .Tag .ID .Digest .CreatedSince .CreatedAt .Size
          .Platform .Store

  ⚠ .Size is the COMPRESSED bytes podbox holds in blobs/, not docker's
    uncompressed total. M1 acquires and does not extract, so the uncompressed
    size is not a number podbox has measured.
";

pub const RMI_USAGE: &str = "\
usage: podbox rmi <image> [image...]
       podbox image rm <image> [image...]

  Remove images and every blob no remaining image reaches. Refuses an image a
  running container holds, and names it.
";

pub const TAG_USAGE: &str = "\
usage: podbox tag <source> <target>

  Point a second name at the same manifest digest. Nothing is fetched and no
  blob is copied.
";

pub const PRUNE_USAGE: &str = "\
usage: podbox image prune [-a|--all] [-f|--force]

  -a, --all    remove every image no container holds, not only untagged ones
  -f, --force  accepted for docker parity. podbox never prompts, so there is
               no confirmation for this to suppress (TODO/cli.md T-0806)

  Skips anything a running container holds and says which.
";

pub const INSPECT_USAGE: &str = "\
usage: podbox inspect [--format T] <image> [image...]

  Fields: .Id .Digest .RepoTags .RepoDigests .Architecture .Os .Created
          .Platform .Size .Store .Layers .RootfsPath .Extracted
          .Exec.Mode .Exec.Shares

  ⚠ .RootfsPath is where the rootfs WOULD be. .Extracted says whether it is
    there; `podbox extract` is what puts it there.

  ⛔ .Exec.* is what `podbox exec` against this image WOULD be, not a property
    of the image. It is a fresh chroot re-entry sharing only the filesystem
    (TODO/enter.md T-0505), and a caller reads it rather than assuming
    docker's namespace entry.

  Without --format, one JSON array, as docker prints.
";

/// `podbox pull`.
pub fn pull(args: &[String]) -> i32 {
    let mut want: Option<&str> = None;
    let mut platform_flag: Option<String> = None;
    let mut insecure: Vec<String> = Vec::new();
    let mut tls_verify: Option<bool> = None;
    // ⚠ Which flag is still waiting for its value, so `--platform <image>` is a
    // usage error naming the flag rather than a pull of something odd.
    let mut expecting: Option<&'static str> = None;
    for a in args {
        if let Some(flag) = expecting {
            match flag {
                "--platform" => platform_flag = Some(a.clone()),
                _ => insecure.push(a.clone()),
            }
            expecting = None;
            continue;
        }
        match a.as_str() {
            "-h" | "--help" => {
                print!("{PULL_USAGE}");
                return 0;
            }
            "--platform" => expecting = Some("--platform"),
            "--insecure-registry" => expecting = Some("--insecure-registry"),
            // ⚠ A bare `--tls-verify` is `=true`, as docker and podman read it.
            "--tls-verify" => tls_verify = Some(true),
            other if other.starts_with("--platform=") => {
                platform_flag = Some(other["--platform=".len()..].to_string());
            }
            other if other.starts_with("--insecure-registry=") => {
                insecure.push(other["--insecure-registry=".len()..].to_string());
            }
            other if other.starts_with("--tls-verify=") => match &other["--tls-verify=".len()..] {
                "true" | "1" => tls_verify = Some(true),
                "false" | "0" => tls_verify = Some(false),
                v => {
                    eprintln!("podbox pull: --tls-verify takes true or false, not {v:?}");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with('-') => return unknown("pull", other, PULL_USAGE),
            other if want.is_none() => want = Some(other),
            other => {
                eprintln!("podbox pull: {other:?}: pull takes one image");
                return EXIT_USAGE;
            }
        }
    }
    if let Some(flag) = expecting {
        eprintln!("podbox pull: {flag} needs a value");
        return EXIT_USAGE;
    }
    let Some(want) = want else {
        eprint!("{PULL_USAGE}");
        return EXIT_USAGE;
    };
    // ⛔ Resolved before the store is opened, so a malformed --platform is a
    // usage error and not a runtime one. TODO/probe.md T-0110's contract.
    let platform = match podbox_image::platform::Platform::wanted(platform_flag.as_deref()) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    // ⛔ Also before the store is opened: a bad host in the config file or the
    // environment is invalid input and exits 2, not 125.
    let policy = match podbox_image::transport::Policy::resolve(&insecure, tls_verify) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };

    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let mut out = std::io::stdout().lock();
    match pull::pull(&store, want, &platform, &policy, &mut out) {
        Ok(done) => {
            // ⛔ The provenance of the probe answer is on stderr, never implied.
            // A cached rung and a measured one are different sentences.
            if let podbox_image::probe_cache::Source::Measured(why) = &done.probe_source {
                eprintln!("podbox: probed this machine rather than using the cache: {why}");
            }
            0
        }
        Err(e) => fail(e),
    }
}

struct Options {
    quiet: bool,
    digests: bool,
    no_trunc: bool,
    format: Option<String>,
    filter: Option<String>,
}

/// `podbox images`.
pub fn images(args: &[String]) -> i32 {
    let mut o = Options {
        quiet: false,
        digests: false,
        no_trunc: false,
        format: None,
        filter: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{IMAGES_USAGE}");
                return 0;
            }
            "-a" | "--all" => {}
            "-q" | "--quiet" => o.quiet = true,
            "--digests" => o.digests = true,
            "--no-trunc" => o.no_trunc = true,
            "--format" => match it.next() {
                Some(t) => o.format = Some(t.clone()),
                None => {
                    eprintln!("podbox images: --format needs a template");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with("--format=") => {
                o.format = Some(other["--format=".len()..].to_string())
            }
            other if other.starts_with('-') => return unknown("images", other, IMAGES_USAGE),
            other if o.filter.is_none() => o.filter = Some(other.to_string()),
            other => {
                eprintln!("podbox images: {other:?}: images takes at most one image");
                return EXIT_USAGE;
            }
        }
    }
    if o.quiet && o.format.is_none() {
        o.format = Some("{{.ID}}".to_string());
    }
    // ⛔ Validated BEFORE the store is opened, and therefore before the loop
    // over records. An empty store means zero iterations, and a template
    // checked only inside the loop is never checked at all: measured on
    // 2026-09-08, `--format '{{.Nope}}'` against an empty store printed
    // nothing and exited 0, so a caller's typo read as an empty result set.
    if let Some(t) = &o.format {
        if let Err(bad) = format::check(t, IMAGE_FIELDS) {
            eprintln!("podbox images: {bad}");
            return EXIT_USAGE;
        }
    }

    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let records = match &o.filter {
        Some(f) => match store.find(f) {
            Ok(r) => r,
            Err(e) => return fail(e),
        },
        None => match store.list() {
            Ok(r) => r,
            Err(e) => return fail(e),
        },
    };

    let mut out = std::io::stdout().lock();
    if let Some(template) = &o.format {
        for r in &records {
            match format::render(template, &image_fields(r, &store, o.no_trunc)) {
                Ok(line) => {
                    let _ = writeln!(out, "{line}");
                }
                Err(bad) => {
                    eprintln!("podbox images: {bad}");
                    return EXIT_USAGE;
                }
            }
        }
        return 0;
    }

    let mut header: Vec<&str> = vec!["REPOSITORY", "TAG"];
    if o.digests {
        header.push("DIGEST");
    }
    header.extend(["IMAGE ID", "CREATED", "SIZE (STORED)"]);
    let mut rows: Vec<Vec<String>> = vec![header.iter().map(|s| s.to_string()).collect()];
    for r in &records {
        let f = image_fields(r, &store, o.no_trunc);
        let get = |k: &str| pick(&f, k);
        let mut row = vec![get("Repository"), get("Tag")];
        if o.digests {
            row.push(get("Digest"));
        }
        row.extend([get("ID"), get("CreatedSince"), get("Size")]);
        rows.push(row);
    }
    let _ = write!(out, "{}", table(&rows));
    0
}

/// `podbox rmi` and `podbox image rm`.
pub fn rmi(args: &[String]) -> i32 {
    let mut wanted: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{RMI_USAGE}");
                return 0;
            }
            // docker's `-f` forces removal of a tagged image. podbox already
            // removes every name the reference gives; what it refuses is an
            // image IN USE, and no flag overrides that, because the deletion
            // would be under a running payload.
            "-f" | "--force" => {}
            other if other.starts_with('-') => return unknown("rmi", other, RMI_USAGE),
            other => wanted.push(other),
        }
    }
    if wanted.is_empty() {
        eprint!("{RMI_USAGE}");
        return EXIT_USAGE;
    }
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let mut code = 0;
    for want in wanted {
        match store.remove(want) {
            Ok(done) => {
                for name in &done.untagged {
                    println!("Untagged: {name}");
                }
                for d in &done.deleted {
                    println!("Deleted: {d}");
                }
            }
            Err(e) => {
                eprintln!("podbox rmi: {e}");
                code = e.exit_code();
            }
        }
    }
    code
}

/// `podbox tag`.
pub fn tag(args: &[String]) -> i32 {
    let mut positional: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{TAG_USAGE}");
                return 0;
            }
            other if other.starts_with('-') => return unknown("tag", other, TAG_USAGE),
            other => positional.push(other),
        }
    }
    if positional.len() != 2 {
        eprint!("{TAG_USAGE}");
        return EXIT_USAGE;
    }
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    match store.tag(positional[0], positional[1]) {
        Ok(_) => 0,
        Err(e) => fail(e),
    }
}

/// `podbox image prune`.
pub fn prune(args: &[String]) -> i32 {
    let mut all = false;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{PRUNE_USAGE}");
                return 0;
            }
            "-a" | "--all" => all = true,
            "-f" | "--force" => {}
            // docker takes `-af` as one cluster, and the acceptance in
            // TODO/image.md T-0204 writes it that way.
            other if other.starts_with('-') && !other.starts_with("--") => {
                for c in other[1..].chars() {
                    match c {
                        'a' => all = true,
                        'f' => {}
                        _ => return unknown("image prune", other, PRUNE_USAGE),
                    }
                }
            }
            other => return unknown("image prune", other, PRUNE_USAGE),
        }
    }
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    match store.prune(all) {
        Ok(done) => {
            if !done.deleted.is_empty() {
                println!("Deleted Images:");
                for name in &done.untagged {
                    println!("untagged: {name}");
                }
                for d in &done.deleted {
                    println!("deleted: {d}");
                }
            }
            // ⛔ T-0204: prune skips anything locked AND SAYS WHICH. A silent
            // skip is a prune that reports success having done nothing it was
            // asked to do.
            for name in &done.skipped {
                println!("skipped: {name} is in use by a running container");
            }
            println!("\nTotal reclaimed space: {}", space::mib(done.freed_bytes));
            0
        }
        Err(e) => fail(e),
    }
}

/// `podbox inspect`, for images. Containers are M3.
/// `podbox extract <image>`: `TODO/extract.md` T-0301 to T-0307, milestone M2.
///
/// ⛔ **This is the verb M2 can drive.** [`TODO/milestones.md`](../../../TODO/milestones.md)
/// T-1103's acceptance runs `podbox run --rm`, which is M3, so extraction would
/// otherwise be implemented with nothing able to exercise it until another
/// milestone lands. That is how a component ships untested.
pub fn extract(args: &[String]) -> i32 {
    let mut want: Option<&str> = None;
    let mut force = false;
    let mut platform_flag: Option<String> = None;
    let mut expect_platform = false;
    for a in args {
        if expect_platform {
            platform_flag = Some(a.clone());
            expect_platform = false;
            continue;
        }
        match a.as_str() {
            "-h" | "--help" => {
                print!("{EXTRACT_USAGE}");
                return 0;
            }
            "--force" => force = true,
            "--platform" => expect_platform = true,
            other if other.starts_with("--platform=") => {
                platform_flag = Some(other["--platform=".len()..].to_string());
            }
            other if other.starts_with('-') => return unknown("extract", other, EXTRACT_USAGE),
            other if want.is_none() => want = Some(other),
            other => {
                eprintln!("podbox extract: {other:?}: extract takes one image");
                return EXIT_USAGE;
            }
        }
    }
    if expect_platform {
        eprintln!("podbox extract: --platform needs a value, for example linux/arm64");
        return EXIT_USAGE;
    }
    let Some(want) = want else {
        eprint!("{EXTRACT_USAGE}");
        return EXIT_USAGE;
    };
    // ⚠ `None` where no flag was given, so `find_one_for` prefers the host's
    // platform and refuses only where that leaves more than one.
    let platform = match platform_flag.as_deref() {
        Some(p) => match podbox_image::platform::Platform::parse(p) {
            Ok(p) => Some(p),
            Err(e) => return fail(e),
        },
        None => None,
    };

    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let record = match store.find_one_for(want, platform.as_ref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("podbox extract: {e}");
            return e.exit_code();
        }
    };

    if podbox_extract::is_extracted(&store, &record.manifest_digest) && !force {
        // ⚠ Already done is not an error, and the path is still printed: a
        // caller pipes this into `run`, and a second invocation must answer the
        // same thing as the first.
        let (rootfs, _) = podbox_extract::paths(&store, &record.manifest_digest);
        println!("{}", rootfs.display());
        return 0;
    }
    if force {
        if let Err(e) = podbox_extract::remove_extracted(&store, &record.manifest_digest) {
            eprintln!("podbox extract: {e}");
            return podbox_image::error::EXIT_RUNTIME_ERROR;
        }
    }

    let manifest_digest = match podbox_image::digest::Digest::parse(&record.manifest_digest) {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    let bytes = match store.read_blob(&manifest_digest) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    let manifest: podbox_image::oci::Manifest = match serde_json::from_slice(&bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "podbox extract: the stored manifest for {want} does not parse: {e}. \
                 Re-pull the image; the blob is present and is not a manifest."
            );
            return podbox_image::error::EXIT_RUNTIME_ERROR;
        }
    };

    // ⛔ A GC must not delete the blobs out from under an extraction in flight,
    // and T-0204's lock is what stops it.
    let _held = match store.hold(&record) {
        Ok(l) => Some(l),
        Err(e) => {
            eprintln!("podbox extract: {e}");
            return podbox_image::error::EXIT_RUNTIME_ERROR;
        }
    };

    let mut out = std::io::stderr().lock();
    match podbox_extract::extract(&store, &manifest, &record.manifest_digest, &mut out) {
        Ok(done) => {
            // ⚠ The estimate flag travels with the number, every time.
            let sized = if done.uncompressed_estimated {
                format!("{} (estimated)", space::mib(done.uncompressed_bytes))
            } else {
                space::mib(done.uncompressed_bytes)
            };
            let _ = writeln!(
                out,
                "podbox: {} layers, {} entries, {} removed by whiteouts, {sized} uncompressed",
                done.layers, done.entries, done.removed
            );
            if done.ownership_dropped > 0 {
                // ⛔ Said out loud. A tree whose ownership differs from the
                // image's in 900 places and says nothing is the dishonesty this
                // project exists to refuse.
                let _ = writeln!(
                    out,
                    "podbox: {} of {} entries carry an id this machine cannot apply; \
                     what the image intended is in {}",
                    done.ownership_dropped,
                    done.sidecar_rows,
                    done.sidecar.display()
                );
            }
            if done.skipped > 0 {
                let _ = writeln!(
                    out,
                    "podbox: {} entries were not materialised ({}). podbox cannot \
                     mknod on this class of runtime, which is the premise of the \
                     whole tool rather than a defect here",
                    done.skipped,
                    done.skipped_kinds.join(", ")
                );
            }
            // ⛔ stdout carries the answer and nothing else, so this verb
            // composes. T-0110 settled that channel contract.
            println!("{}", done.rootfs.display());
            0
        }
        Err(e) => {
            eprintln!("podbox extract: {e}");
            podbox_image::error::EXIT_RUNTIME_ERROR
        }
    }
}

pub fn inspect(args: &[String]) -> i32 {
    let mut template: Option<String> = None;
    let mut wanted: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{INSPECT_USAGE}");
                return 0;
            }
            "-f" | "--format" => match it.next() {
                Some(t) => template = Some(t.clone()),
                None => {
                    eprintln!("podbox inspect: --format needs a template");
                    return EXIT_USAGE;
                }
            },
            other if other.starts_with("--format=") => {
                template = Some(other["--format=".len()..].to_string())
            }
            other if other.starts_with('-') => return unknown("inspect", other, INSPECT_USAGE),
            other => wanted.push(other),
        }
    }
    if wanted.is_empty() {
        eprint!("{INSPECT_USAGE}");
        return EXIT_USAGE;
    }
    // ⛔ Before the store is even opened. Every reference may fail to resolve,
    // and a template checked only inside the loop over what DID resolve is
    // never checked at all. It is also the caller's own input, so it is
    // reported before anything about the machine is.
    if let Some(t) = &template {
        if let Err(bad) = format::check(t, INSPECT_FIELDS) {
            eprintln!("podbox inspect: {bad}");
            return EXIT_USAGE;
        }
    }
    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };

    let mut records = Vec::new();
    let mut code = 0;
    for want in &wanted {
        match store.find_one(want) {
            Ok(r) => records.push(r),
            Err(e) => {
                eprintln!("podbox inspect: {e}");
                code = e.exit_code();
            }
        }
    }
    if let Some(t) = &template {
        for r in &records {
            match format::render(t, &inspect_fields(r, &store)) {
                Ok(line) => println!("{line}"),
                Err(bad) => {
                    eprintln!("podbox inspect: {bad}");
                    return EXIT_USAGE;
                }
            }
        }
        return code;
    }
    let docs: Vec<String> = records.iter().map(|r| inspect_json(r, &store)).collect();
    println!("[{}]", docs.join(","));
    code
}

// ------------------------------------------------------------------- fields

/// The field names `podbox images --format` answers to.
///
/// ⚠ These names exist so a template can be validated with NO record in hand,
/// which is what an empty store leaves the caller with. The test
/// `the_field_names_are_the_names_the_builder_produces` is what stops this list
/// and the builder below drifting: `docs/conventions/forbidden-patterns.md`
/// forbids a value in two places with no check that they agree.
pub const IMAGE_FIELDS: &[&str] = &[
    "Repository",
    "Tag",
    "ID",
    "Digest",
    "CreatedSince",
    "CreatedAt",
    "Size",
    "Platform",
    "Store",
];

/// The same, for `podbox inspect --format`.
pub const INSPECT_FIELDS: &[&str] = &[
    "Id",
    "Digest",
    "RepoTags",
    "RepoDigests",
    "Architecture",
    "Os",
    "Created",
    "Platform",
    "Size",
    "Store",
    "Layers",
    // ⭐ M2. `TODO/extract.md` T-0302's Prove reads the sidecar at
    // `<RootfsPath>/../.meta.jsonl`, so this field is what makes that command
    // writable at all.
    "RootfsPath",
    "Extracted",
    // ⛔ TODO/enter.md T-0505. What `podbox exec` against this image WOULD be,
    // and not a property of the image. It is constant because podbox has one
    // exec mechanism; when M4 gives a container its own record the same two
    // names sit on that record and answer from the container that was entered.
    "Exec.Mode",
    "Exec.Shares",
];

/// `podbox exec`'s mode, in one word each, for `inspect` and for the banner.
///
/// ⛔ TODO/enter.md T-0505. `docker exec` enters the container's namespaces.
/// podbox has none to enter, so its `exec` re-runs the section 6.5 sequence
/// against the same rootfs: it shares the filesystem tree and NOTHING else, not
/// the process table, not `/proc`, not signals, not the original's environment.
/// A caller reads these rather than assuming.
pub const EXEC_MODE: &str = "fresh-chroot";
pub const EXEC_SHARES: &str = "filesystem";

fn image_fields(r: &Record, store: &Store, no_trunc: bool) -> Vec<(&'static str, String)> {
    let id = r.config_digest.clone();
    let short_id = id
        .strip_prefix("sha256:")
        .map(|h| h[..12.min(h.len())].to_string())
        .unwrap_or_else(|| id.clone());
    vec![
        ("Repository", r.display_repository()),
        ("Tag", r.tag.clone().unwrap_or_else(|| "<none>".into())),
        ("ID", if no_trunc { id } else { short_id }),
        ("Digest", r.digest.clone()),
        (
            "CreatedSince",
            r.created
                .as_deref()
                .and_then(clock::since)
                // ⛔ A dash where the value is unknown, never a fabricated one.
                .unwrap_or_else(|| "-".into()),
        ),
        ("CreatedAt", r.created.clone().unwrap_or_else(|| "-".into())),
        ("Size", space::mib(r.stored_bytes)),
        ("Platform", r.platform.clone()),
        ("Store", store.root().display().to_string()),
    ]
}

fn inspect_fields(r: &Record, store: &Store) -> Vec<(&'static str, String)> {
    vec![
        ("Id", r.config_digest.clone()),
        ("Digest", r.digest.clone()),
        (
            "RepoTags",
            r.tag.as_ref().map(|_| r.name()).unwrap_or_default(),
        ),
        (
            "RepoDigests",
            format!("{}@{}", r.display_repository(), r.digest),
        ),
        ("Architecture", r.architecture.clone()),
        ("Os", r.os.clone()),
        ("Created", r.created.clone().unwrap_or_else(|| "-".into())),
        ("Platform", r.platform.clone()),
        ("Size", r.stored_bytes.to_string()),
        ("Store", store.root().display().to_string()),
        ("Layers", r.layers.join(" ")),
        ("RootfsPath", {
            let (rootfs, _) = podbox_extract::paths(store, &r.manifest_digest);
            rootfs.display().to_string()
        }),
        // ⚠ Whether the rootfs is THERE, which is a different question from
        // where it would be. `inspect` on a pulled-but-unextracted image must
        // not imply a tree that does not exist.
        (
            "Extracted",
            podbox_extract::is_extracted(store, &r.manifest_digest).to_string(),
        ),
        ("Exec.Mode", EXEC_MODE.to_string()),
        ("Exec.Shares", EXEC_SHARES.to_string()),
    ]
}

fn inspect_json(r: &Record, store: &Store) -> String {
    // ⚠ Built through serde_json rather than by concatenation, so a repository
    // or a tag carrying a quote cannot break the document.
    serde_json::json!({
        "Id": r.config_digest,
        "Digest": r.digest,
        "RepoTags": r.tag.as_ref().map(|_| vec![r.name()]).unwrap_or_default(),
        "RepoDigests": [format!("{}@{}", r.display_repository(), r.digest)],
        "Architecture": r.architecture,
        "Os": r.os,
        "Created": r.created,
        "Platform": r.platform,
        "Size": r.stored_bytes,
        "Store": store.root().display().to_string(),
        "Layers": r.layers,
        "ManifestDigest": r.manifest_digest,
        "PulledAt": r.pulled_at,
        // ⛔ T-0505, and nested here because it is nested in --format too. A
        // caller that reads one and not the other must not find two shapes.
        "Exec": { "Mode": EXEC_MODE, "Shares": EXEC_SHARES },
    })
    .to_string()
}

fn pick(fields: &[(&str, String)], key: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// docker's two-space-padded columns.
fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i + 1 == row.len() {
                out.push_str(cell);
            } else {
                out.push_str(cell);
                out.push_str(&" ".repeat(widths[i] - cell.chars().count() + 3));
            }
        }
        out.push('\n');
    }
    out
}

fn unknown(verb: &str, flag: &str, usage: &str) -> i32 {
    eprintln!("podbox {verb}: unknown option {flag:?}");
    eprint!("{usage}");
    EXIT_USAGE
}

fn fail(e: Error) -> i32 {
    eprintln!("podbox: {e}");
    e.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_pads_every_column_to_its_widest_cell() {
        let rows = vec![
            vec!["REPOSITORY".to_string(), "TAG".to_string()],
            vec!["alpine".to_string(), "latest".to_string()],
        ];
        let got = table(&rows);
        let lines: Vec<&str> = got.lines().collect();
        assert!(lines[0].starts_with("REPOSITORY   TAG"), "{got}");
        assert!(lines[1].starts_with("alpine       latest"), "{got}");
        // ⚠ No trailing padding on the last column: a trailing run of spaces
        // is invisible in review and breaks an exact-match acceptance.
        assert!(!lines[1].ends_with(' '), "{got:?}");
    }

    #[test]
    fn the_field_names_are_the_names_the_builder_produces() {
        // ⛔ The remedy docs/conventions/forbidden-patterns.md names for a value
        // in two places: a check that they agree. The list is what validates a
        // template with no record in hand; the builder is what fills one in.
        let store =
            Store::open(std::env::temp_dir().join(format!("podbox-names-{}", std::process::id())))
                .unwrap();
        let r = a_record();
        let built: Vec<&str> = image_fields(&r, &store, false)
            .iter()
            .map(|(k, _)| *k)
            .collect();
        assert_eq!(built, IMAGE_FIELDS);
        let built: Vec<&str> = inspect_fields(&r, &store).iter().map(|(k, _)| *k).collect();
        assert_eq!(built, INSPECT_FIELDS);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn a_template_naming_a_field_no_verb_has_is_refused_without_any_record() {
        // ⭐ The defect this pass found: with the check inside the loop, an
        // empty store made `--format '{{.Nope}}'` print nothing and exit 0.
        assert!(format::check("{{.Nope}}", IMAGE_FIELDS).is_err());
        assert!(format::check("{{.Tag", IMAGE_FIELDS).is_err());
        assert!(format::check("table {{.Tag}}", IMAGE_FIELDS).is_err());
        assert!(format::check("{{.Digest}}", IMAGE_FIELDS).is_ok());
        // ⚠ The two verbs do not have the same fields, and each is checked
        // against its own list.
        assert!(format::check("{{.RepoTags}}", IMAGE_FIELDS).is_err());
        assert!(format::check("{{.RepoTags}}", INSPECT_FIELDS).is_ok());
    }

    fn a_record() -> Record {
        Record {
            repository: "docker.io/library/alpine".into(),
            tag: Some("latest".into()),
            digest: format!("sha256:{}", "1".repeat(64)),
            digest_media_type: podbox_image::oci::MEDIA_OCI_INDEX.into(),
            manifest_digest: format!("sha256:{}", "2".repeat(64)),
            config_digest: format!("sha256:{}", "3".repeat(64)),
            platform: "linux/amd64".into(),
            layers: vec![],
            stored_bytes: 0,
            architecture: "amd64".into(),
            os: "linux".into(),
            created: None,
            pulled_at: clock::now(),
        }
    }

    #[test]
    fn an_image_with_no_creation_timestamp_prints_a_dash_and_not_a_guess() {
        // ⛔ docs/AGENTS.md absolute 3.
        let store =
            Store::open(std::env::temp_dir().join(format!("podbox-fields-{}", std::process::id())))
                .unwrap();
        let f = image_fields(&a_record(), &store, false);
        assert_eq!(pick(&f, "CreatedSince"), "-");
        assert_eq!(pick(&f, "CreatedAt"), "-");
        assert_eq!(pick(&f, "ID"), "333333333333");
        assert_eq!(pick(&f, "Repository"), "alpine");
        let _ = std::fs::remove_dir_all(store.root());
    }
}
