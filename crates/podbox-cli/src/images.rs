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
usage: podbox pull <image>

  Fetch an image and everything it needs into the content-addressed store.
  HTTPS only: a registry that offers only http:// is a named refusal, never a
  downgrade, because tcp/80 egress hangs rather than failing on the runtime
  podbox targets.

  The store is $PODBOX_STORE, else $XDG_DATA_HOME/podbox, else
  $HOME/.local/share/podbox. Blocks AND inodes are checked at it before the
  first layer is fetched, and the refusal names the destination, the free
  amount, the required amount and the unit.
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
          .Platform .Size .Store .Layers

  Without --format, one JSON array, as docker prints.
";

/// `podbox pull`.
pub fn pull(args: &[String]) -> i32 {
    let mut want: Option<&str> = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{PULL_USAGE}");
                return 0;
            }
            other if other.starts_with('-') => return unknown("pull", other, PULL_USAGE),
            other if want.is_none() => want = Some(other),
            other => {
                eprintln!("podbox pull: {other:?}: pull takes one image");
                return EXIT_USAGE;
            }
        }
    }
    let Some(want) = want else {
        eprint!("{PULL_USAGE}");
        return EXIT_USAGE;
    };

    let store = match podbox_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let mut out = std::io::stdout().lock();
    match pull::pull(&store, want, &mut out) {
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
    fn an_image_with_no_creation_timestamp_prints_a_dash_and_not_a_guess() {
        // ⛔ docs/AGENTS.md absolute 3.
        let store =
            Store::open(std::env::temp_dir().join(format!("podbox-fields-{}", std::process::id())))
                .unwrap();
        let r = Record {
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
        };
        let f = image_fields(&r, &store, false);
        assert_eq!(pick(&f, "CreatedSince"), "-");
        assert_eq!(pick(&f, "CreatedAt"), "-");
        assert_eq!(pick(&f, "ID"), "333333333333");
        assert_eq!(pick(&f, "Repository"), "alpine");
        let _ = std::fs::remove_dir_all(store.root());
    }
}
