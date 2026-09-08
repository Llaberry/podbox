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
Status:      done 2026-09-08

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

**Done 2026-09-08.** `crates/podbox-image/src/registry.rs`, driven by
`experiments/150-image-acquisition.sh`, which exits 0. The `Prove` above is its
clause 1 and the two digests are equal:
`sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b`.

Challenge-driven bearer auth rather than a table of hosts: the 401's
`WWW-Authenticate` names the realm, and the token is cached per
`<endpoint>|<scope>` rather than per host, because a token minted for one
repository does not authorise another and a host-keyed cache would send it
anyway and read the 403 as a permission problem.

⛔ **The refusal is driven, not asserted.** Clause 4 of the script runs
`podbox pull http://registry.invalid/...` under `timeout 30`: exit 124 would be
the hang this refusal exists to prevent, and podbox exits **2** with
`podbox speaks HTTPS only and will not downgrade`. A registry whose token realm
is `http://` is refused separately and is a runtime failure rather than a usage
one, because it is not the caller's input.

⚠ Two things in the entry's `Approach` are narrower in the code than the prose
suggests, and both are deliberate. Retry covers 429 and the five-hundreds only:
a 404 or a 403 retried three times is three wrong answers and a slower message.
And `Retry-After` is honoured in its seconds form; the HTTP-date form is legal
and falls back to the ordinary backoff, because a mis-parsed date is a wait of
unknown length and [RULES.md](RULES.md) section 8 forbids one.

⚠ The manifest read is bounded before it is taken, not after: `take(ceiling + 1)`
ahead of `read_to_end`, so a registry that streams forever cannot exhaust
memory.

---

### T-0202 A content-addressed store, and digest parity with docker

Source:      `TOOL.md` section 5 M1, section 6.2
Category:    image
Priority:    P0
Effort:      M
Status:      done 2026-09-08

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

**Done 2026-09-08.** `crates/podbox-image/src/store.rs` and
`crates/podbox-image/src/digest.rs`, driven by
`experiments/150-image-acquisition.sh` clauses 2 and 3, which is the `Prove`
above as a script: the second pull prints `55afa1ecc21d: Already exists` and
fetches nothing, and all 4 stored blobs hash to the name they are stored under.

⭐ **The digest recorded is the one computed over the bytes the registry
served for the reference that was asked for.** For a multi-platform tag that is
the **index** digest, not the per-platform manifest digest, and recording the
wrong one is exactly the parity this entry is about: `docker image inspect`
reports the first. The store carries both, as `digest` and `manifest_digest`.
The bytes are written to `blobs/` verbatim; re-serialising a parsed document
would produce a different digest.

⛔ **Verification is on the way in AND on the way out.**
`digest::Verifier` hashes what reached the sink rather than what was offered,
counts what actually arrived rather than trusting the descriptor's `size`, and
the staged file is renamed into `blobs/` only when both matched. `read_blob`
re-hashes on the way out, which fires when something outside podbox has edited
the store, and a test drives that by overwriting a stored blob.

⚠ **`{{.Size}}` is the compressed bytes podbox holds, and the column says
`SIZE (STORED)`.** docker's `SIZE` is the sum of the uncompressed layers; M1
does not extract, so that number has not been measured here and printing
docker's heading over a different quantity would be a number that was not
measured. It becomes the extracted size when [extract.md](extract.md) lands.

⚠ **The store is one shared directory with a version discriminator**,
`store.json`'s `podbox_store: 1`. A store a later podbox wrote is refused by
name rather than parsed as though its fields meant what they mean here, and the
index is written to a staging name inside the store and renamed over the old
one, so a process killed mid-write leaves the previous index rather than a
truncated one.

---

### T-0203 Check `statvfs` for blocks and inodes, and name the destination

Source:      `TOOL.md` section 6.2, section 8; `paper_final.md` section 8.2 and F7
Category:    image
Priority:    P0
Effort:      S
Status:      partial 2026-09-08

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
Prove:       `./experiments/140-space-precheck.sh` exits 0

**Partial, 2026-09-08.** `crates/podbox-image/src/space.rs`, called before any
download from `crates/podbox-image/src/pull.rs`.
`experiments/140-space-precheck.sh` exits 0, on two real tmpfs mounts rather
than fixtures, and the two refusals it drove are:

```
not enough space at <dest> for alpine:latest: 1012.0 KiB free, 19.7 MiB needed
  (3.7 MiB of payload plus 16.0 MiB of headroom)
not enough inodes at <dest> for alpine:latest: 5 free, 9 needed. This filesystem
  has 256.0 MiB of free space and cannot create the files anyway
```

Destination, free amount, required amount and unit, in both. **0 blobs** were
written before either refusal, which is the whole point of a precheck: a
partial store has to be cleaned up on a filesystem that is already full.

⛔ **The remaining half is the second call site.** The entry says "before any
download **and again before any extraction**", and there is no extraction until
M2. The check is a function with one caller today and gains the second in
[extract.md](extract.md) T-0301. That is the only reason this is `partial`.

⚠ **Two corrections to the entry, both found by writing it.**

1. The `Prove` named `90-space-precheck.sh`, and 90 is
   `experiments/90-nsswitch-contract.sh`. `experiments/README.md` rules that a
   number is never reused, because a citation of it has to keep meaning what it
   meant. The script is `experiments/140-space-precheck.sh` and the `Prove`
   above names it.
2. The `Approach` says `statvfs`. The code calls **`statfs(2)`**, which is the
   syscall behind `statvfs(3)` and is what
   `crates/podbox-probe/src/writable.rs` already calls, so free space has one
   read path in this tree rather than two.

⚠ **`f_files == 0` means inodes are not counted**, not that there are none.
tmpfs allocates them dynamically, and a check reading `f_ffree` as zero there
would refuse every write on a filesystem with room.

---

### T-0204 `images`, `rmi`, `tag`, and a store GC that cannot delete a running container's rootfs

Source:      `TOOL.md` section 5 M1, section 6.8
Category:    image
Priority:    P1
Effort:      M
Status:      partial 2026-09-08

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

**Partial, 2026-09-08.** `images`, `image ls`, `rmi`, `image rm`, `tag`,
`image prune` and `inspect` are implemented in `crates/podbox-cli/src/images.rs`
over `crates/podbox-image/src/store.rs`, and the lock is
`crates/podbox-probe/src/sys.rs`'s `flock(2)` on an fd opened **without**
`O_CLOEXEC`, which is the inheritance the mechanism depends on.
`experiments/160-store-gc.sh` exits 0 and drives all five clauses.

⛔ **The half that is not done is the `Prove` above**, which holds the lock with
`podbox run -d`. `run` is M3, [milestones.md](milestones.md) T-1104. The holder
in the script is `flock(1)` taking the same advisory lock on the same file, so
the refusal path is driven for real; what is not yet driven is podbox holding it
across its own exec.

What the script established:

| clause | reading |
| --- | --- |
| a shared blob under two tags | survives removing one of them |
| `rmi` under a holder | exit 125, "is in use by a running container" |
| `image prune -af` under a holder | exit 0, `skipped: alpine:latest is in use`, image kept |
| the holder released | `rmi` exits 0 and frees all 4 blobs |
| a blob path resolving outside the store | refused, and the canary survived |

⭐ **Reachability is computed over what SURVIVES, never over what was
deleted.** Computing it the other way deletes a shared layer with the first
image that goes, and clause 1 is that case.

⚠ **A defect in the harness, not in podbox, and it is worth recording because
it is the mechanism working exactly as intended.**
`flock FILE -c 'sleep N'` runs the sleep as a **child**, which inherits the
locked fd, so killing `flock` leaves the sleeper holding the lock. The first run
of clause 4 failed for that reason. The script now `exec`s the sleeper so one
process holds the fd. That inheritance is precisely what this entry buys.

⚠ **`prune -f` is accepted and does nothing**, and `--help` says so rather than
leaving it as a flag no code reads: docker's `-f` suppresses a confirmation
prompt, and podbox never prompts ([cli.md](cli.md) T-0806).

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
