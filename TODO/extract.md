# extract

`crates/podbox-extract`. `TOOL.md` section 6.3, milestone M2.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

The component with the most exposure and the least room for shortcuts. Four
separate tools in the corpus stop here, all at the same wall, and the wall is
`chown` to an id the user namespace does not map.

⭐ **The measured facts this file rests on are in
`experiments/results/whiteout-contract.txt`**, taken by
`experiments/70-whiteout-contract.sh` against two images pinned by digest. Four
requirements come out of it and each is an entry below.

---

### T-0301 Extract in-process, at entry level, never through system `tar`

Source:      `TOOL.md` section 6.3, `paper_final.md` section 10.4
Category:    extract
Priority:    P0
Effort:      M
Status:      open

Problem:     Shelling out to `tar` inherits its ownership semantics, its exit
             codes and its path behaviour. That is how the ownership wall
             reaches five separate tools rather than one.
Premise:     Measured in the corpus, at file and line, in two of them.
             `references/RuriOSS__rurima/tree/src/archive.c:95` builds a `tar`
             argument vector containing `-xpf`, which preserves permissions and
             ownership, and that is where its `docker pull` dies.
             `references/indigo-dc__udocker/tree/udocker/container/structure.py:285-287`
             also shells out, and survives only because it passes
             `--no-same-owner` and `--no-same-permissions`. The flags are the
             mechanism; the shell-out is not.
Approach:    Drive a tar reader at **entry level**: read a header, decide, act.
             Never call a library's `unpack()` convenience, which makes the
             ownership, permission and path decisions this component exists to
             make. The candidate crate and its measured size delta are T-0907.
Decision:    In-process over a Rust tar reader, rather than a vendored C
             libarchive. libarchive is where `short write: -20` came from
             (T-0203), it is a C dependency that complicates `crt-static`, and
             the entry-level API podbox needs is the smaller half of what it
             offers.
Prove:       `! ldd target/x86_64-unknown-linux-musl/release/podbox 2>/dev/null | grep -q archive` and `strace -f -e trace=execve podbox pull alpine:latest 2>&1 | grep -c 'execve.*"tar"' | grep -qx 0`

---

### T-0302 Ownership-neutral extraction plus the sidecar

Source:      `TOOL.md` section 6.3, `paper_final.md` section 9.1 and section 10.4
Category:    extract
Priority:    P0
Effort:      M
Status:      open

Problem:     `chown` and `lchown` to an unmapped id return **`EINVAL`**, not
             `EPERM`, and that is where GNU tar, containers/storage's layer
             applier, Apptainer's Go unpacker, pacman's `DownloadUser` and
             rurima's `tar -xpf` all stop. `/etc/shadow` is the usual first
             casualty because it is `root:shadow`, gid 42.
Premise:     ⭐ **Measured here, against a pinned image.**
             `experiments/results/whiteout-contract.txt` check B reads
             `-rw-r----- 0/42 ... etc/shadow` out of
             `alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc`.
             The wall itself is at
             `references/containers__storage/tree/pkg/archive/archive.go:798-811`:
             with `chownOpts` nil it takes the ids straight from the tar header
             and calls `idtools.SafeLchown`. The switch is at
             `references/containers__storage/tree/pkg/archive/archive.go:1104`.
Approach:    Never restore ownership by default. Record intended metadata in a
             sidecar keyed by path, and apply it only where the target id is
             mapped:

             ```json
             {"path":"etc/shadow","uid":0,"gid":42,"mode":"0640",
              "applied":{"uid":0,"gid":0},"reason":"gid 42 unmapped"}
             ```

             The sidecar buys three things `--no-same-owner` alone does not:
             faithful re-export, an honest answer to a workload that asks who
             owns a file, and a diagnostic that names the gap.
             ⛔ It changes no kernel permission check and must never be
             presented as if it does.
             The design to copy is `udocker`'s, which is ownership-neutral by
             construction rather than by recovery:
             `references/indigo-dc__udocker/tree/udocker/container/structure.py:285-287`.
Decision:    A sidecar, not `--no-same-owner` semantics alone. The alternative
             loses the intended metadata, and T-0704's interposer needs it to
             answer a `stat` truthfully.
             ⚠ **A note that corrects the corpus, not a decision.**
             `references/containers__storage/tree/pkg/archive/archive.go:804-808`
             already carries `ignoreChownErrors`, which turns exactly this
             failure into a warning, and it is reachable by configuration:
             `references/containers__storage/tree/types/options.go:473-474` and
             `references/containers__storage/tree/drivers/vfs/driver.go:59-62`.
             That does not change podbox's design and it does change a claim
             about podman, which is T-0205.
Prove:       `podbox pull alpine:latest && podbox run --rm alpine:latest test -f /etc/shadow && jq -e 'select(.path=="etc/shadow") | .uid==0 and .gid==42 and .applied.gid==0' "$(podbox inspect --format '{{.RootfsPath}}' alpine:latest)/../.meta.jsonl"`

---

### T-0303 Whiteouts are matched on the basename, never with a path glob

Source:      `TOOL.md` section 6.3; measured against `references/indigo-dc__udocker`
Category:    extract
Priority:    P0
Effort:      S
Status:      open

Problem:     A whiteout at the **root** of a layer is silently ignored by a
             selector anchored on a slash, and real OCI layers do not write a
             `./` prefix, so there is no slash to anchor on.
Premise:     ⭐ **Measured here, and the measurement contradicted the first
             reading of the code.** `experiments/70-whiteout-contract.sh`
             check A reads the first member of every layer of two pinned images
             and gets `bin/`, `bin` and `etc/`: no `./` prefix anywhere. Check D
             then runs udocker's own selector,
             `tar t --wildcards -f LAYER '*/.wh.*'`
             (`references/indigo-dc__udocker/tree/udocker/container/structure.py:242`),
             against a crafted archive with one whiteout at the layer root and
             one nested, and the root one is missed. Output in
             `experiments/results/whiteout-contract.txt`.
             ⚠ The crafted archive is deliberate: neither pinned image happens
             to carry a root-level whiteout, so passing on them would say
             nothing.
Approach:    Take the basename of every entry and test it for the `.wh.` prefix.
             Handle `.wh..wh..opq` as an opaque-directory marker separately: it
             removes the contents of the directory it sits in, which is what
             `references/indigo-dc__udocker/tree/udocker/container/structure.py:249-256`
             does by listing that directory and removing each entry.
Decision:    Basename matching over any glob. A glob is what produced the
             defect, and there is no glob that is both correct for a prefixed
             archive and a bare one.
Prove:       `./experiments/70-whiteout-contract.sh` exits 0

---

### T-0304 Refuse an entry that resolves outside the destination, including through a symlink from the same layer

Source:      `TOOL.md` section 5 M2 acceptance 3, section 6.3; `paper_final.md` section 10.4
Category:    extract
Priority:    P0
Effort:      M
Status:      open

Problem:     A crafted layer creates `evil -> /etc` as its first entry and
             writes `evil/passwd` as its second. Both entries are lexically
             inside the destination and the second lands outside it.
Premise:     Read, and half the mechanism is in the corpus at file and line.
             `references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:19-48`
             is `symlink_target_within_root`, a **purely lexical** check whose
             own comment says why it is lexical: "so it cannot be defeated by
             filesystem races". Beside it,
             `references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:10-12`
             rejects `""`, `.`, `..`, `/` and NUL as a path component.
             ⚠ **That is only half of what M2 acceptance 3 needs.** onelf
             validates a symlink entry when the symlink is created. It does not
             validate a later regular entry whose path traverses that symlink,
             which is exactly the crafted case.
Approach:    Both halves. Validate every symlink entry lexically on creation,
             as onelf does. Then resolve every entry's path against the
             **accumulated tree**, with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`
             per component where the kernel has it, and an `O_NOFOLLOW` walk
             holding a directory fd where it does not. Refuse, name the entry
             and the layer digest, and fail the extraction rather than skipping
             the entry.
Decision:    Refuse rather than sanitize. Rewriting a hostile path to something
             safe produces an image that differs from its digest with nothing
             saying so, and the caller cannot tell a repaired layer from a
             clean one.
Prove:       `./experiments/80-extract-path-safety.sh` exits 0

---

### T-0305 An absolute symlink target is rootfs-relative, not a refusal

Source:      Found while reading `references/qaidvoid__onelf` against M2
Category:    extract
Priority:    P1
Effort:      S
Status:      open

Problem:     A distro rootfs is full of legitimate absolute symlinks. Rejecting
             them, as a bundle packer reasonably does, would refuse almost every
             real image.
Premise:     ⭐ **Measured here.** `experiments/results/whiteout-contract.txt`
             check C reads
             `lrwxrwxrwx 0/0 ... var/cache/xbps -> /var/cache/xbps` out of layer
             1 of
             `voidlinux/voidlinux-musl@sha256:d5c970d0015c3aa2559a2b5a87b158839969fbb1941a9ef52cc484a5392554cd`.
             The refusal that must not be copied is
             `references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:27-29`,
             which returns false for any absolute target.
Approach:    Interpret an absolute target **relative to the rootfs**, which is
             what it will mean once the payload is inside the chroot, and apply
             T-0304's containment check to the result. `/etc/mtab ->
             /proc/self/mounts` and `/bin -> usr/bin` are then correct and
             contained; a target that still escapes after rebasing is refused.
             ⚠ The same absolute-symlink fact bites the completion layer from
             the other side: writing through `/etc/mtab` with `>` follows the
             link out of the rootfs. That is T-0405.
Decision:    Rebase rather than reject, and rebase rather than dereference at
             extraction time. Dereferencing would bake the build host's tree
             into the image.
Prove:       `podbox pull voidlinux/voidlinux-musl:latest && podbox run --rm voidlinux/voidlinux-musl:latest readlink /var/cache/xbps` prints nothing and exits non-zero, the link having been whiteouted

---

### T-0306 Re-permission between layers, or the second layer fails

Source:      `references/indigo-dc__udocker`, read against `TOOL.md` section 6.3
Category:    extract
Priority:    P1
Effort:      S
Status:      open

Problem:     Ownership-neutral extraction leaves directories with the image's
             modes and the extractor's ownership. A mode-0555 directory in
             layer 1 cannot be written into when layer 2 extracts over it, and
             the failure looks like a corrupt layer.
Premise:     Read at file and line, not measured here.
             `references/indigo-dc__udocker/tree/udocker/container/structure.py:296-303`
             runs a `find` after every layer that adds `u+x` to directories
             missing it, `u+w` and `u+r` to anything missing them, and `chgrp`
             to the caller's own gid. Its docstring at
             `references/indigo-dc__udocker/tree/udocker/container/structure.py:265-267`
             states the reason: "permissions are changed to avoid file
             permission issues when extracting the next layer".
Approach:    After each layer, walk what that layer wrote and add owner read,
             write and, for directories, execute. Record every mode changed in
             the sidecar beside the ownership, so the intended mode survives a
             re-export. Do not `chgrp`: podbox extracts as the only mapped id
             already.
Decision:    Widen only the owner bits, and only on entries this extraction
             wrote. Widening group or other bits changes what the image means
             for a payload that reads modes, and widening everything makes the
             sidecar the only record of the real image.
Prove:       `podbox pull docker.io/library/debian:bookworm-slim && podbox run --rm docker.io/library/debian:bookworm-slim true` succeeds on an image whose layers overlay one another

---

### T-0307 Hard links, symlinks and the layer order

Source:      `TOOL.md` section 6.3, `paper_final.md` section 10.4
Category:    extract
Priority:    P1
Effort:      S
Status:      open

Problem:     Dropping ownership does not discharge the rest of the OCI layer
             contract, and a runtime that treats extraction as "untar each
             layer" produces a tree that is not the image.
Premise:     Read. The contract is: apply layers in order; interpret whiteouts;
             preserve hard links and symlinks; refuse an escaping entry.
             The whiteout ordering is more precise than "after each layer":
             `references/indigo-dc__udocker/tree/udocker/container/structure.py:279`
             applies layer N's whiteouts to the accumulated tree **before**
             extracting N, which is the order that makes a layer able to both
             delete a path and recreate it.
Approach:    For each layer in manifest order: apply that layer's whiteouts to
             the accumulated tree, then extract that layer's entries. Materialize
             a hard link as a link to the already-extracted target within the
             same destination, and refuse one whose target is outside it.
             Preserve symlinks as symlinks; never dereference.
Decision:    Whiteouts before the layer's own entries, per udocker. The opposite
             order deletes what the same layer just wrote.
Prove:       `./experiments/70-whiteout-contract.sh` exits 0 and `podbox run --rm voidlinux/voidlinux-musl:latest xbps-install -Sy --dry-run bash` does not report `Symbolic link loop`
