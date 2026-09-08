# image

`crates/podbox-image`. `TOOL.md` section 6.2, milestone M1.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

The registry plane is ordinary HTTPS and file I/O and works on this runtime.
⛔ **Do not gold-plate it.** Three things about this environment change it, and
one of them cost a prior session real time with an error message that named
neither the cause nor the directory.

---

### T-0201 Registry client, HTTPS only, with no plain-HTTP fallback

Source:      `TOOL.md` section 6.2, section 11.1
Category:    image
Priority:    P0
Effort:      M
Status:      open

Problem:     tcp/80 egress is broken on the studied runtime. Anything that falls
             back to plain HTTP **hangs** rather than failing, so a fallback
             that exists to improve reliability makes the failure untimed and
             undiagnosable.
Premise:     Read, from a single-session observation of the target. It is
             consistent with every distro fixup in section 6.4 being a protocol fixup
             rather than a mirror fixup.
Approach:    Implement the OCI distribution endpoints actually used: token auth,
             manifest by tag and by digest, manifest lists for
             `linux/amd64`, and blob fetch. HTTPS only. A registry that offers
             only `http://` is a named refusal, not a downgrade.
             Every request carries a timeout and a bounded retry, per
             [RULES.md](RULES.md) section 8.
Decision:    A blocking client. podbox has no reason to be async, and an async
             runtime is a large dependency with nothing to do here. The crate
             sweep for the client and its TLS is T-0905 and T-0906, and neither
             lands without a measured size delta.
Prove:       `podbox pull alpine:latest && podbox images --format '{{.Digest}}' alpine:latest | grep -qx "$(docker image inspect alpine:latest --format '{{index .RepoDigests 0}}' | cut -d@ -f2)"`

---

### T-0202 A content-addressed store, and digest parity with docker

Source:      `TOOL.md` section 5 M1, section 6.2
Category:    image
Priority:    P0
Effort:      M
Status:      open

Problem:     Without content addressing, `pull` twice costs twice and nothing
             can be verified. Without digest parity with docker, an agent that
             checks a digest against a published one gets a different answer and
             cannot tell why.
Premise:     Read. M1's acceptance is exactly that parity.
Approach:    Store blobs under `sha256:<hex>`, verify every blob against its
             descriptor digest **as it is written** rather than afterwards, and
             key images by manifest digest. Reject a blob whose computed digest
             differs, naming both.
             ⭐ Verify before use, not after download. The reasoning is in the
             corpus at
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:149-157`,
             which checks a payload's content hash before an in-memory exec "so
             an in-memory exec never runs unverified bytes". The same holds for
             a layer about to be extracted as root.
Decision:    One store shared by images and containers, with a lock, rather than
             a store per container. The GC race that decides this is T-0204.
Prove:       `podbox pull alpine:latest && podbox pull alpine:latest 2>&1 | grep -qi 'already' && sha256sum "$(podbox inspect --format '{{.Store}}' alpine:latest)"/blobs/sha256/* >/dev/null`

---

### T-0203 Check `statvfs` for blocks and inodes, and name the destination

Source:      `TOOL.md` section 6.2, section 8; `paper_final.md` section 8.2 and F7
Category:    image
Priority:    P0
Effort:      S
Status:      open

Problem:     A payload that overshoots the default temporary directory produces
             an error naming neither space nor the directory. On the studied
             runtime `/tmp` is 64 MiB, which almost no image fits in.
Premise:     ⭐ Read at file and line, and the reason the message is useless is
             visible there.
             `references/mhx__dwarfs/tree/src/utility/filesystem_extractor.cpp:544-552`
             takes `rv` from `write_range_data`, which returns a libarchive
             `la_ssize_t`, then formats it into `"short write: {} != {}"` as
             though it were a byte count. `-20` is `ARCHIVE_WARN`, a status
             code; the underlying `archive_errno` is `ENOSPC`. The message
             mentions neither the errno nor the path.
             ⚠ dwarfs is a split licence, MIT for the read path and GPL-3.0 for
             the write path ([reference-map.md](reference-map.md)), and is read
             only here.
Approach:    Before any download and again before any extraction, `statvfs` the
             destination and check **blocks and inodes both**. On failure, name
             the destination, the free amount, the required amount and the unit.
             Choose the destination by free space among the writable paths
             T-0104 probed, rather than taking `TMPDIR`.
             ⚠ The same trap is in the corpus as a shipped default:
             `references/VHSgunzo__memfd-exec/tree/src/executable.rs:580-584`
             falls back to `env::temp_dir()`, then `/dev/shm`, then
             `$HOME/.cache`, with no space check on any of them.
Decision:    Refuse up front rather than streaming until `ENOSPC`. A partial
             extraction has to be cleaned up on a filesystem that is already
             full, which is the state in which cleanup is least likely to work.
Prove:       `./experiments/90-space-precheck.sh` exits 0

---

### T-0204 `images`, `rmi`, `tag`, and a store GC that cannot delete a running container's rootfs

Source:      `TOOL.md` section 5 M1, section 6.8
Category:    image
Priority:    P1
Effort:      M
Status:      open

Problem:     A GC that runs while a container is using an extraction deletes the
             tree out from under it, and the payload's failure names a missing
             file rather than a concurrent deletion.
Premise:     Read at file and line. The mechanism that closes it is
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:275-278`:
             the cache lock guard is held **through exec**, with its fd left
             inheritable, "so a concurrent process's GC cannot delete the
             package while it is still in use".
             ⚠ The opposite failure is also in the corpus. `ruri` tracker issue
             #59 records that its `-U` unmount reached **outside** the container
             when the container directory was under a FUSE mount. A cleanup verb
             that resolves outside its own subtree is the same class of defect.
Approach:    Hold a lock fd on each in-use rootfs, inherited across the exec, and
             have `prune` and `rmi` skip anything locked and say which. Refuse
             any `rm` or `prune` whose resolved target is outside the store, and
             resolve it with the same containment check as T-0304.
Decision:    An inheritable lock fd rather than a pid file. A pid file is stale
             the moment a process dies unexpectedly, and the check that clears a
             stale one is the race this is closing.
Prove:       `podbox run -d --name gc-probe alpine:latest sleep 30 && ! podbox image prune -af 2>&1 | grep -q "$(podbox inspect --format '{{.Image}}' gc-probe)" && podbox rm -f gc-probe`

---

### T-0205 Re-test podman with `vfs` and `ignore_chown_errors` before repeating "no path exists"

Source:      Found while reading `references/containers__storage` against `paper_final.md` section 6
Category:    image
Priority:    P3
Effort:      S
Status:      open

Problem:     The corpus states that podman's layer application has no path on
             this runtime. The code carries a configuration option that turns
             exactly that failure into a warning, and nobody has run the
             combination.
Premise:     ⭐ **Read at file and line, and it disagrees with the claim.**
             `references/containers__storage/tree/pkg/archive/archive.go:804-808`:

             ```go
             if ignoreChownErrors {
                     fmt.Fprintf(os.Stderr, "Chown error detected. Ignoring due to ignoreChownErrors flag: %v\n", err)
             } else {
                     return err
             }
             ```

             It is reachable by configuration, not only internally:
             `references/containers__storage/tree/types/options.go:473-474`
             builds `<driver>.ignore_chown_errors` from `storage.conf`, and both
             drivers accept it:
             `references/containers__storage/tree/drivers/vfs/driver.go:59-62`.
             Beside that, `paper_final.md` F6 measured that
             `--storage-driver vfs` **initializes** on podman 4.3.1.
             ⚠ Nothing here is a result. The combination has not been run.
Approach:    Run it in the reconstruction and record the outcome either way.
             ⛔ A negative result is a result and gets committed: if it still
             fails, the reason is worth having, because "no path exists" would
             then be a measured claim rather than an inherited one.
Decision:    This changes nothing about podbox's design and it changes a
             comparative claim podbox's own documentation would otherwise
             repeat. That is why it is P3 and not dropped.
Prove:       `./experiments/95-podman-vfs-ignorechown.sh` exits 0 or 1, never 2, and its output is committed to `experiments/results/`
