//! Containment: a destructive verb may only reach inside its own subtree.
//!
//! `TODO/image.md` T-0204. ⚠ The opposite failure is in the corpus: `ruri`
//! tracker issue #59 records that its `-U` unmount reached **outside** the
//! container when the container directory was under a FUSE mount. A cleanup
//! verb that resolves outside its own subtree is the same class of defect
//! whether it unmounts or unlinks.
//!
//! ⛔ The check is on the **resolved** path, not the written one. A store entry
//! that is a symlink to `/etc` deletes `/etc` under a check that only inspects
//! the string.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Resolve `candidate` and assert it is `root` or below it.
///
/// The candidate does not have to exist: its nearest existing ancestor is
/// resolved and the remaining components are appended, so a path about to be
/// created is checked by the same function as one about to be deleted.
/// ⛔ One gate per action: `docs/conventions/code.md`. Every path `rmi`,
/// `prune` and the blob writer touch comes through here.
pub fn within(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let real_root = root.canonicalize().map_err(|e| {
        Error::Store(format!(
            "the store root {} does not resolve: {e}",
            root.display()
        ))
    })?;

    // ⛔ A relative candidate is refused rather than joined to the working
    // directory: which directory that is depends on the caller, and a
    // containment check whose answer depends on `cd` is not one.
    if !candidate.is_absolute() {
        return Err(Error::Store(format!(
            "{} is not an absolute path, so it names nothing this check can \
             resolve against the store root",
            candidate.display()
        )));
    }

    // ⛔ `..` is refused outright rather than normalised away, and BEFORE
    // anything is resolved. Two reasons, and the second was measured here:
    // a `..` under a directory that does not exist yet climbs out through a
    // prefix `starts_with` never sees; and `Path::file_name` returns `None`
    // for a path ending in `..`, so the walk below cannot even take such a
    // path apart. Refusing first makes both moot.
    if candidate
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(Error::Store(format!(
            "{} climbs out of the store with `..`",
            candidate.display()
        )));
    }

    let mut existing = candidate.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let real = loop {
        match existing.canonicalize() {
            Ok(p) => break p,
            Err(_) => match (existing.file_name(), existing.parent()) {
                (Some(name), Some(parent)) => {
                    tail.push(name.to_os_string());
                    existing = parent.to_path_buf();
                }
                _ => {
                    return Err(Error::Store(format!(
                        "{} has no existing ancestor to resolve against",
                        candidate.display()
                    )))
                }
            },
        }
    };
    let mut resolved = real;
    for name in tail.iter().rev() {
        resolved.push(name);
    }

    if resolved != real_root && !resolved.starts_with(&real_root) {
        return Err(Error::Store(format!(
            "{} resolves to {}, which is outside the store at {}. podbox \
             refuses to delete outside its own subtree (TODO/image.md T-0204)",
            candidate.display(),
            resolved.display(),
            real_root.display()
        )));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "podbox-contain-{}-{name}-{}",
            std::process::id(),
            name.len()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("blobs")).unwrap();
        d
    }

    #[test]
    fn a_path_inside_the_store_resolves() {
        let root = scratch("inside");
        let got = within(&root, &root.join("blobs/sha256/abc")).unwrap();
        assert!(got.starts_with(root.canonicalize().unwrap()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_symlink_pointing_out_of_the_store_is_refused() {
        // ⭐ The whole reason the check is on the resolved path. A string check
        // passes this and deletes /etc.
        let root = scratch("symlink");
        let target = std::env::temp_dir().join(format!("podbox-outside-{}", std::process::id()));
        std::fs::create_dir_all(&target).unwrap();
        let link = root.join("escape");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let e = within(&root, &link).unwrap_err();
        assert!(format!("{e}").contains("outside the store"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&target);
    }

    #[test]
    fn a_traversal_through_a_directory_that_does_not_exist_yet_is_refused() {
        let root = scratch("traversal");
        let e = within(&root, &root.join("not-yet/../../../etc/passwd")).unwrap_err();
        assert!(format!("{e}").contains("climbs out"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sibling_directory_sharing_a_name_prefix_is_not_inside() {
        // ⚠ `starts_with` on a Path compares COMPONENTS, so `/a/store-2` is not
        // inside `/a/store`. The test exists because the string form of the
        // same check would say it was.
        let root = scratch("prefix");
        let sibling = PathBuf::from(format!("{}-2", root.display()));
        std::fs::create_dir_all(&sibling).unwrap();
        assert!(within(&root, &sibling).is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&sibling);
    }

    #[test]
    fn a_relative_path_is_refused_rather_than_joined_to_the_working_directory() {
        let root = scratch("relative");
        assert!(within(&root, Path::new("blobs/sha256/abc")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
