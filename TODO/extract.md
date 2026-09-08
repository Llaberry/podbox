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
Status:      done 2026-09-08

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
Prove:       `./experiments/220-extract-path-safety.sh` exits 0, and
             `strace -f -e trace=execve -o /tmp/e.txt podbox extract --force alpine:latest; grep -c 'execve.*"tar"' /tmp/e.txt` prints 0

**Done, 2026-09-08.** `crates/podbox-extract/src/apply.rs` drives
`tar::Archive::entries()` and makes every decision itself. 515 entries out of a
real `alpine`, 4,664 out of a two-layer `voidlinux-musl`.

⭐ **Measured on 2026-09-08, both halves.** The shipped binary reports
`statically linked` and carries no `archive` anything; a full extraction issues
**one** `execve`, which is podbox itself. The `Prove` above is rewritten because
the original piped `grep -c` into `grep -qx 0`, and `docs/AGENTS.md` names that
exact trap twice: `grep -c` exits 1 on zero matches, so the pipeline's status
was `grep`'s and the clause could not fail the way it was written.

⚠ **A finding about the crate, not about podbox.** `tar::Builder` REFUSES to
write a member whose path contains `..` ("paths in archives must not have
`..`"), and `tar::Archive`'s reader hands that same member straight to the
caller. The writer's validation is not the reader's, so the hostile archives in
`experiments/220-extract-path-safety.sh` and in the crate's own tests are built
at the header level. Taking a Rust tar crate does not make this safe by itself,
which is what [T-0304](extract.md) is for.

---

### T-0302 Ownership-neutral extraction plus the sidecar

Source:      `TOOL.md` section 6.3, `paper_final.md` section 9.1 and section 10.4
Category:    extract
Priority:    P0
Effort:      M
Status:      done 2026-09-08

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
Prove:       `podbox pull alpine:latest && podbox extract alpine:latest && jq -e 'select(.path=="etc/shadow") | .uid==0 and .gid==42 and .applied.gid==0' "$(podbox inspect --format '{{.RootfsPath}}' alpine:latest)/../.meta.jsonl"`

**Done, 2026-09-08.** `crates/podbox-extract/src/sidecar.rs`, written per entry
by `crates/podbox-extract/src/apply.rs`. The `Prove` above runs and exits 0
against a real `alpine`, and the row it reads is, verbatim:

```
{"path":"etc/shadow","uid":0,"gid":42,"mode":"0640","applied":{"uid":0,"gid":0},"reason":"gid 42 unmapped"}
```

⭐ **One entry of 515 carried an id this machine could not apply, and it is
`etc/shadow`.** The measurement in
`experiments/results/whiteout-contract.txt` check B predicted exactly that file
and exactly that gid, and the extraction found it without being told.
`voidlinux-musl` has three: `usr/bin/wall` and `usr/bin/write` at gid 5, and
`usr/bin/xbps-uchroot` at gid 101.

⛔ **The `Prove` no longer runs `podbox run --rm`**, which is M3, because
`podbox extract` is the verb that puts the rootfs there and this entry is about
the sidecar rather than about entering the image. [T-1103](milestones.md)
carries the clauses that genuinely need M3 and is `partial` for them.

⛔ **The honest half is asserted too.** A test reading only the sidecar would
pass on an implementation that claimed an ownership it never applied, so
`the_sidecar_does_not_claim_an_ownership_the_kernel_did_not_apply` checks the
file on disk and asserts its gid is the extractor's and NOT 42.

---

### T-0303 Whiteouts are matched on the basename, never with a path glob

Source:      `TOOL.md` section 6.3; measured against `references/indigo-dc__udocker`
Category:    extract
Priority:    P0
Effort:      S
Status:      done 2026-09-08

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

**Done, 2026-09-08.** `crates/podbox-extract/src/whiteout.rs`.

⭐ **Mutation-proved against the defect it exists for.** Replacing the basename
test with udocker's own slash-anchored selector (`path.contains("/.wh.")`) turns
three tests red, and the one that names it is
`a_root_level_whiteout_is_found_and_a_glob_would_miss_it`. The mutation
reproduces check D's measured reading exactly: the nested whiteout is found and
the root-level one is missed.

⚠ **Two cases the entry did not name, both settled here.** `.wh.` with nothing
after it names no file and is left as an ordinary entry, because turning it into
a removal of `""` would remove the directory the marker sits in; and a whiteout
whose own name carries a trailing slash is still a removal, because the
basename must not depend on the directory marker.

---

### T-0304 Refuse an entry that resolves outside the destination, including through a symlink from the same layer

Source:      `TOOL.md` section 5 M2 acceptance 3, section 6.3; `paper_final.md` section 10.4
Category:    extract
Priority:    P0
Effort:      M
Status:      done 2026-09-08

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
Prove:       `./experiments/220-extract-path-safety.sh` exits 0

**Done, 2026-09-08.** `crates/podbox-extract/src/safety.rs`, and the script
exits 0 with six checks. `experiments/results/extract-path-safety.txt` is the
reading; the refusal it captures names the layer digest and the entry:

```
refusing layer sha256:43a4608399...: the entry "evil/passwd" is not extractable
because resolving it against what the layers have written so far leaves the
destination ("evil": ELOOP).
```

⭐ **BOTH MECHANISMS ARE DRIVEN, ON ONE KERNEL, AND THAT IS DELIBERATE.**
Measured on 2026-09-08: this host is 6.18.44 and `openat2(2)` is present, so an
ordinary run never enters the `O_NOFOLLOW` walk and the walk,  which is what
every kernel before Linux 5.6 runs,  would have shipped having refused nothing.
`safety::Resolve` exists only so the tests can force it, and
`the_walk_and_openat2_refuse_the_same_traversal` asserts the two give the same
answer to the same attack AND both still admit a legitimate path.

⛔ **Mutation-proved.** Removing `RESOLVE_NO_SYMLINKS` and `O_NOFOLLOW` turns
three tests red, reporting "the traversal was not refused".

⚠ **The refusal is asserted on the FILESYSTEM, not on the exit code.** Check F
of the script and every refusal test look for the file the hostile layer tried
to write. A refusal that still wrote it would pass a check that read only the
status.

---

### T-0305 An absolute symlink target is rootfs-relative, not a refusal

Source:      Found while reading `references/qaidvoid__onelf` against M2
Category:    extract
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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
Prove:       `podbox pull voidlinux/voidlinux-musl:latest && podbox extract voidlinux/voidlinux-musl:latest && ! test -L "$(podbox inspect --format '{{.RootfsPath}}' voidlinux/voidlinux-musl:latest)/var/cache/xbps"`

**Done, 2026-09-08.** `safety::rebase_symlink_target`, and driven against the
image the entry names. `var/cache/xbps` is **gone** from the extracted tree,
layer 2's single whiteout having removed it, which is the reading the entry
predicted. **543 symlinks survive**, including `etc/mtab -> /proc/self/mounts`
(absolute, rebased and allowed), `usr/sbin -> bin` (relative) and
`var/lock -> ../run/lock` (a legitimate climb).

⛔ **Rebasing decides whether a link is ALLOWED; it never decides what is
stored.** The link on disk carries the target the image wrote, verbatim, and
check E of `experiments/220-extract-path-safety.sh` asserts exactly that.
Storing the rebased form would bake this host's idea of the rootfs into the
image, which is the same mistake as dereferencing and is what this entry's
Decision refuses.

⚠ The `Prove` is rewritten: the original ran `readlink` under `podbox run`,
which is M3, and asserted "prints nothing and exits non-zero",  a shape that
also passes when `run` itself is missing. It now asks the filesystem.

---

### T-0306 Re-permission between layers, or the second layer fails

Source:      `references/indigo-dc__udocker`, read against `TOOL.md` section 6.3
Category:    extract
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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
Prove:       `cargo test -p podbox-extract a_read_only_directory_in_one_layer_can_be_written_into_by_the_next` passes, and `podbox extract voidlinux/voidlinux-musl:latest` succeeds on a two-layer image

**Done, 2026-09-08.** The pass runs after each layer, over what **that layer
wrote**, and adds owner read, write and execute to directories.

⭐ **The case is crafted, because it has to be.** A two-layer real image does
not reliably contain a mode-0555 directory in layer 1 that layer 2 writes into,
so passing on one would say nothing,  the same argument
[T-0303](extract.md)'s premise makes about root-level whiteouts. The crafted
pair is exactly that, and it fails without the pass.

⛔ **Owner bits only**, asserted:
`re_permissioning_widens_the_owner_and_leaves_group_and_other_alone` checks that
a 0555 directory ends 0755 and that the group and other bits are untouched.
Widening those would change what the image means for a payload that reads modes.
⛔ **No `chgrp`**, which is udocker's third action: podbox extracts as the only
mapped id already.

⚠ The `Prove` no longer runs `podbox run --rm` (M3) against
`debian:bookworm-slim`. The real two-layer image it now names is one
[T-1103](milestones.md) already pulls, so the acceptance costs no extra
registry traffic,  and Docker Hub's anonymous quota is what
[T-0206](image.md) exists for.

---

### T-0307 Hard links, symlinks and the layer order

Source:      `TOOL.md` section 6.3, `paper_final.md` section 10.4
Category:    extract
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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
Prove:       `./experiments/70-whiteout-contract.sh` exits 0 and `cargo test -p podbox-extract a_layer_may_delete_a_path_and_recreate_it_in_the_same_layer` passes

**Done, 2026-09-08.** `crates/podbox-extract/src/remove.rs` collects a layer's
whiteouts and applies them to the accumulated tree **before** that layer's own
entries, per
`references/indigo-dc__udocker/tree/udocker/container/structure.py:279`.

⭐ **The ordering is asserted by the one case that distinguishes the two
orders**: a layer that whiteouts `f` and writes `f`. Under udocker's order the
new `f` survives; under "whiteouts after the layer" it is deleted by the same
layer that wrote it. Also driven: an opaque marker empties its directory, keeps
the directory, and does not remove a file the same layer writes afterwards.

⚠ **It costs a second pass over each layer stream**, because a tar is a stream
and the whiteouts are scattered through it. The alternative is holding a
decompressed layer in memory, which for a distro base image is hundreds of
megabytes. Stated in the module header rather than left for whoever profiles it.

⛔ A hard link is materialised within the destination and one whose target is
outside it is **refused**, driven both ways. A hard link out of the rootfs is a
file the payload can write through, which the containment check would otherwise
never see because the escape is in the link target rather than in the path.

⚠ The `Prove` no longer runs `xbps-install` under `podbox run` (M3). The
`Symbolic link loop` it watched for is the T-0305 case, and the extracted tree
is asserted directly there.
