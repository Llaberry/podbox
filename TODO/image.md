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

⭐ **Three registries, not one.** The challenge-driven auth was driven against
`public.ecr.aws`, `ghcr.io` and `quay.io` as well as Docker Hub, and all four
answer. That is what a table of hosts would not have survived, and it is why
`experiments/140-space-precheck.sh` and `experiments/160-store-gc.sh` could be
moved off the Hub when its anonymous rate limit turned a check about disk space
into a check about somebody else's quota.

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
`SIZE (STORED)`.** M1 did not extract, so the uncompressed number had not been
measured here, and printing docker's heading over a different quantity would be
a number that was not measured.

⭐ **CORRECTED 2026-09-08 BY M2, AND THE PREMISE WAS WRONG.** This paragraph
said "docker's `SIZE` is the sum of the uncompressed layers". **Measured on
this host, against the same image by the same digest, it is not.**

| | bytes |
| --- | --- |
| `docker image inspect alpine:latest --format '{{.Size}}'` | 3,857,242 |
| every blob podbox holds for it, summed | **3,857,242** |
| podbox `{{.Size}}` (layers and config, no manifests) | 3,847,002 |
| the layer's real uncompressed length, from gzip `ISIZE` | 8,697,856 |
| `docker history` for the same layer | ~9.07 MB |

docker 29.3.1 here runs the **containerd** image store
(`io.containerd.snapshotter.v1`), and its `.Size` is the size of the content in
the content store, which is compressed. It matches podbox's blob total **to the
byte**; the 10,240 difference against podbox's own `{{.Size}}` is the manifest
and index blobs, which podbox does not count as image payload.

⛔ **So the divergence this entry documented does not exist on this docker, and
the open question that asked what to do about it was asking about a claim
rather than a reading.** ⚠ It is one host and one docker: the classic (non
containerd) image store reports the uncompressed total, and this is recorded as
a reading from this machine rather than as docker's definition.

⚠ **And "uncompressed size" is three different numbers**, which is why podbox
names which one it prints. The tar STREAM length (8,697,856 here, read exactly
from gzip's trailer) is what `podbox extract` reports and what the space
precheck needs; the APPLIED filesystem size is what `docker history` shows; and
`du` over the extracted tree is a third. Reporting any of them as "the"
uncompressed size without saying which would be the same defect this paragraph
was written to avoid.

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
Status:      done 2026-09-08

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

**Done, 2026-09-08.** `crates/podbox-image/src/space.rs`, called before any
download from `crates/podbox-image/src/pull.rs` and before any extraction from
`crates/podbox-extract/src/lib.rs`.
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

⭐ **CLOSED, 2026-09-08, by M2.** The second call site exists:
`crates/podbox-extract/src/lib.rs` calls the same `space::require` before the
first layer is read. That was the only reason this was `partial`.

⛔ **It is before the layer LOOP, not inside it, and the placement is the
point.** An image with no layers, or one whose layers are all already present,
runs that loop zero times, and a guard inside it would never be reached by the
caller who has nothing. The same rule caught a real defect in M1's
`--format` validation, where a template checked inside the loop over records
was never checked at all against an empty store.

⚠ **What the precheck needs is the UNCOMPRESSED size, and the manifest carries
the compressed one.** gzip's trailer carries `ISIZE`, the uncompressed length
modulo 2^32, so a gzip layer's figure is **read rather than guessed**: measured
on 2026-09-08, alpine's single layer is 3,846,391 bytes compressed and
8,697,856 uncompressed. A zstd layer, or a gzip trailer that has plainly
wrapped, falls back to a multiplier, and `Extracted::uncompressed_estimated`
carries which of the two it was so a caller printing the number can label it.
⛔ The inode figure IS an estimate in every case, and says so: the real count
is the number of entries across every layer, which is not known until the
layers have been read, and reading them is what this check exists to happen
before.

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

⚠ **"Before any download" means before any LAYER.** The manifest is fetched
first, because the manifest is what says how large the layers are, and it is a
few kilobytes against their megabytes. The consequence is visible in the script:
every clause needs the registry to answer once, so a registry that refuses is a
`SKIP` with exit 2 rather than a failed check.

⚠ **The inode clause had to be made deterministic.** Mounting straight into a
12-inode tmpfs may or may not leave room for the store's own five directories,
so the refusal came sometimes from this check and sometimes from `mkdir`. Both
are honest, and the record flipped between two values on re-runs. The store is
now built while inodes are plentiful and the remainder consumed afterwards, and
the free count at the moment of the pull is recorded beside the verdict.

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

---

### T-0206 A registry fixture, so the acceptance stops depending on somebody else's quota

Source:      Found while re-running `experiments/150-image-acquisition.sh` against its own committed reading
Category:    image
Priority:    P1
Effort:      L
Status:      open

Problem:     M1's acceptance pulls from Docker Hub, and Docker Hub answers
             `HTTP 429: TOOMANYREQUESTS: You have reached your unauthenticated
             pull rate limit` after enough anonymous pulls from one address.
             ⛔ A gate that a third party can turn red is not a gate. It is
             worse than a missing one, because it teaches a session to read
             `exit 2` as noise, and `exit 2` is the state this project uses for
             "could not run".
Premise:     ⭐ **Measured on 2026-09-08, in this repository.** The session that
             implemented M1 exhausted the anonymous quota with its own runs.
             `experiments/results/image-acquisition.txt` is a real reading and
             the script that took it now exits 2 rather than 1 when the quota is
             spent, which is correct and is also a gate nobody can run on demand.
             ⚠ Two of the four scripts were moved off the Hub as a stopgap:
             `experiments/140-space-precheck.sh` and
             `experiments/160-store-gc.sh` compare nothing against docker and
             default to `public.ecr.aws`. `experiments/150-image-acquisition.sh`
             cannot follow them, because its whole question is whether podbox's
             digest equals the one `docker image inspect` reports for the same
             tag.
Approach:    Serve the OCI distribution endpoints podbox uses from the store
             podbox already has, on loopback, and point both podbox and docker
             at it. The store is content-addressed and holds the manifest bytes
             verbatim, which is exactly what `GET /v2/<name>/manifests/<ref>`
             has to return, so the fixture is a reader over
             `crates/podbox-image/src/store.rs` rather than a second store.
             ⚠ Four endpoints, no more: `GET /v2/`, manifests by tag and by
             digest, and blobs. No push, no catalog, no pagination.
             ⛔ HTTPS, with a certificate the fixture generates and both clients
             are pointed at. A fixture that speaks plain HTTP would be the one
             thing `TODO/image.md` T-0201 refuses, wired into the acceptance.
Decision:    A fixture in this tree over `registry:2` from Docker Hub. Pulling
             the registry image to escape the pull limit is circular, and the
             licence determination for a new tree is work
             [reference-map.md](reference-map.md) requires before it is used.
             ⚠ The rejected alternative is authenticating to the Hub: it needs a
             credential, and `docs/security/secrets.md` keeps credentials out of
             this tree, so the acceptance would then run only where somebody has
             one.
Prove:       `./experiments/180-registry-fixture.sh` exits 0 with the network to registry-1.docker.io blocked

---

### T-0207 Fetch layers with bounded concurrency, and measure what it buys

Source:      `TOOL.md` section 6.2; `docs/conventions/forbidden-patterns.md`, the resources table
Category:    image
Priority:    P2
Effort:      L
Status:      open

Problem:     `crates/podbox-image/src/pull.rs` fetches layers one after another.
             `docs/conventions/forbidden-patterns.md` names "a sequential
             awaited loop over independent IO" and what it caused: wall-time
             blowups as the data grows. alpine has one layer and hides it; a
             fifteen-layer image on a link with latency does not.
Premise:     ⭐ **Read in this tree, at file and line.**
             `crates/podbox-image/src/pull.rs` loops over
             `manifest.layers.iter().chain(once(&manifest.config))` and calls
             `client.blob` inside it, so every layer waits for its predecessor.
             ⚠ **Not yet measured.** The claim that this costs wall time here is
             a reading of the code, not a number, and taking the number is part
             of this entry rather than a prerequisite for it.
Approach:    A bounded pool of worker threads over the descriptor list, with the
             bound a named constant and not a per-machine guess. Each worker
             stages, verifies and commits through the existing store functions,
             so there is one write path and not two.
             ⛔ The transcript stays in manifest order however the fetches
             interleave. A progress display whose order depends on scheduling is
             a display that reports the machine's mood.
             ⛔ A failure in one worker cancels the rest and removes every
             staged file, rather than leaving a partial store behind.
Decision:    Threads over an async runtime. `TODO/deps.md` T-0906 ruled a
             blocking client on a measured size delta, and adding an async
             runtime to parallelise four downloads reopens a decision that was
             closed against a number.
             ⚠ The genuine fork, with a recommendation: whether the bound is
             fixed or scales with the CPU count. Recommend **fixed**, because
             the constraint is the registry's willingness to serve, not this
             machine's cores, and a CPU-scaled bound on a 96-core builder is how
             a client earns a rate limit.
Prove:       `./experiments/190-parallel-layers.sh` exits 0 and records the wall time of both shapes against a multi-layer image

---

### T-0208 `--platform`, and a store that can hold two variants of one tag

Source:      `TOOL.md` section 6.2; `docs/conventions/forbidden-patterns.md`, the correctness table
Category:    image
Priority:    P2
Effort:      L
Status:      open

Problem:     podbox resolves an index to `linux/amd64` and records the platform
             it stored, but nothing can ask for another one, and the store keys
             an image by repository and tag with no variant in the key. Pulling
             `linux/arm64` would therefore either be refused or would overwrite
             the amd64 record under the same name.
Premise:     ⭐ **Read at file and line, and the corpus names the failure.**
             `crates/podbox-image/src/oci.rs` hard-codes `OS` and `ARCH`, and
             `crates/podbox-image/src/store.rs`'s `put_record` retains on
             `repository` and `tag` alone. `docs/conventions/forbidden-patterns.md`
             carries the incident: `podman run --platform linux/riscv64 alpine`
             retags the shared local `alpine:latest` to the riscv64 image, so
             the next plain `podman run alpine` fails with `Exec format error`
             and reads as an unrelated breakage.
             ⚠ podbox records `platform` on every record today, so the
             information needed to key by it is already stored and unused.
Approach:    `--platform <os>/<arch>[/<variant>]` on `pull`, defaulting to the
             host's. The store key becomes repository, tag **and** platform.
             `podbox images` gains the variant in its table only where more than
             one is held, so the common output does not change.
             ⛔ A reference that resolves to two stored records and no
             `--platform` is a named refusal listing both, never a pick.
Decision:    Key by platform rather than refusing a second variant. The refusal
             is smaller and is the wrong shape: podbox's audience is automated,
             and a multi-architecture builder is exactly the caller that needs
             two variants at once.
             ⚠ Rejected: defaulting to `linux/amd64` on every host. It is what
             the code does now and it is wrong the moment podbox runs on arm64,
             which is most of the machines its audience rents.
Prove:       `podbox pull --platform linux/arm64 alpine:latest && podbox images --format '{{.Platform}} {{.Digest}}' alpine:latest | sort | uniq -c | grep -qx ' *1 linux/amd64 .*' `

---

### T-0209 Registry authentication, without a credential ever entering this tree

Source:      `TOOL.md` section 6.2, section 11.1
Category:    image
Priority:    P2
Effort:      L
Status:      open

Problem:     Every request podbox makes is anonymous. A private registry answers
             401 and podbox has nothing to answer with, so the whole class of
             image an agent sandbox actually runs is unreachable.
Premise:     Read. `crates/podbox-image/src/registry.rs` answers a `Bearer`
             challenge with no credential, which is what the anonymous flow
             needs and is all it does. ⚠ The challenge parser, the realm
             handling and the per-scope token cache are already there and are
             the parts this entry does not have to write.
Approach:    Read `~/.docker/config.json` and `$XDG_RUNTIME_DIR/containers/auth.json`,
             both of which the audience's machines already have, and send Basic
             to the token realm named by the challenge. `podbox login` writes
             the same file docker writes.
             ⛔ **No credential is logged, put in an error, or written into a
             result file**, and `registry.rs`'s `redact` already covers URLs.
             `docs/security/secrets.md` binds.
             ⛔ A credential helper (`credsStore`) is executed only when the
             config names one, never guessed, and a helper that fails is a named
             refusal rather than a silent fall back to anonymous. Falling back
             turns a permission problem into a 404 about a repository that
             exists.
Decision:    Read docker's and podman's files rather than inventing a third.
             podbox answers to both names, and a runtime that needs its own
             credential file has not replaced either.
             ⚠ The genuine fork, with a recommendation: whether `podbox login`
             writes a credential at all, given it lands in a file. Recommend
             **yes, and only through the credential helper where one is
             configured**, because refusing to write is not the same as the
             credential not existing: it just moves it to a shell history.
Prove:       `./experiments/200-registry-auth.sh` exits 0 against the fixture of T-0206 with a required credential

---

### T-0210 The store's concurrency contract, written down and driven

Source:      Found while reviewing M1 adversarially; `docs/conventions/code.md`, "assume the worst case per feature"
Category:    image
Priority:    P1
Effort:      L
Status:      open

Problem:     The store takes one exclusive lock around each index
             read-modify-write and nothing else. What happens when two podboxes
             pull different images at once, or one prunes while another pulls,
             is not written down anywhere, so every future change to the store
             is a change to an unstated contract.
Premise:     ⭐ **Partly measured, and the measurement is the reason this is P1
             rather than P3.** Two concurrent `podbox pull` runs of the same
             reference into one store were driven on 2026-09-08: both exited 0,
             the index parsed, one image and four blobs resulted, and no staging
             file was left. ⚠ That is one ordering of one case. The cases NOT
             driven are the interesting ones: a `prune` between another
             process's space precheck and its first blob write, and two pulls of
             DIFFERENT images racing on the same index.
             ⚠ A second gap is read rather than measured:
             `crates/podbox-image/src/store.rs` names a staging file after the
             process id, so a process killed with SIGKILL mid-blob leaves a
             `*.partial` file that nothing ever removes.
Approach:    Write the contract into the module header as invariants, then drive
             each one: what a reader may assume while a writer runs, what a
             `prune` may delete while a `pull` is in flight, and what a crashed
             process may leave. Then close the gaps the driving finds. A sweep
             of orphaned staging files on store open is one of them, and it is
             the "sweep that heals drift the happy path let slip" that
             `docs/conventions/code.md` asks for.
             ⛔ The sweep removes only files this store's own staging directory
             holds, resolved through `crates/podbox-image/src/contain.rs`.
Decision:    A stress experiment rather than a unit test. The failure is a race,
             and a race that only a mock can produce is a race the mock's author
             imagined. ⚠ The suite keeps its deterministic tests for the pieces;
             the contract is proved against real concurrent processes.
Prove:       `./experiments/210-store-concurrency.sh` exits 0 with 8 concurrent workers over 3 references, and the store verifies afterwards

---

### T-0211 An image lock outlives its holder whenever anything forks

Source:      Found by `cargo test --workspace` failing intermittently at the close of M2, then reproduced deliberately
Category:    image
Priority:    P1
Effort:      S
Status:      open

Problem:     `Store::hold` opens the image lock **without** `O_CLOEXEC`, which
             is [T-0204](image.md)'s mechanism and is right: the guard has to
             survive the exec so a concurrent GC cannot delete a rootfs a
             running payload is using. ⛔ The consequence nothing accounts for
             is that the fd is inherited by **every** child forked while the
             lock is held, not only by the payload it was opened for. Any such
             child keeps the `flock` alive for its whole lifetime, so
             `Store::in_use` reports an image as held after its holder has
             released it, and `rmi` and `prune` refuse an image nothing is
             using.
Premise:     ⭐ **Measured on 2026-09-08, twice: once by accident and once on
             purpose.**

             It first appeared as an intermittent failure of
             `store::tests::an_image_a_container_holds_is_refused_by_rmi_and_skipped_by_prune`
             at `crates/podbox-image/src/store.rs:792`, the assertion
             `!s.in_use(&r).unwrap()` immediately after `drop(held)`. It did not
             reproduce in eleven consecutive runs of that test alone or of the
             whole `podbox-image` suite, which is what a race looks like: the
             other thread has to fork inside the window.

             ⛔ **"Flake" is not a root cause**, so it was reproduced
             deliberately: hold the lock, `clone_fork` a child that sleeps,
             drop the lock, and ask.

             | when | `in_use` |
             | --- | --- |
             | after `drop(held)`, forked child alive | **true** |
             | after that child exits | **false** |

             ⚠ The forking thread in the test binary is the probe: M0's design
             is one freshly forked child per probe, and `cargo test` runs tests
             in parallel threads of one process.
             ⛔ **It is not confined to tests.** `probe_cache::measure` forks
             the same way, so any future call that measures the probe while an
             image lock is held leaks that lock into 50 short-lived children.
             Today's `pull` takes the probe answer before it holds anything,
             which is why this has never been seen outside the suite.
Approach:    Keep the fd inheritable **only across the exec it exists for**, and
             not across every unrelated fork. Open the lock `O_CLOEXEC` like
             every other fd in this tree, and clear `FD_CLOEXEC` with
             `fcntl(F_SETFD, 0)` on that one descriptor immediately before the
             `execve` that hands the container its rootfs, which is where
             [enter.md](enter.md) M3 does the exec.
             ⚠ That inverts the default rather than adding a special case: an
             fd that escapes into an unrelated child is the accident, and the
             payload's inheritance is the deliberate act.
             ⛔ The test that found this is the plant: it has to be made to fail
             on demand rather than once a week. A case that forks a child while
             the lock is held, asserts `in_use` is false after the drop, and is
             therefore red before the fix and green after.
Decision:    Fix the inheritance, not the test. Marking the test `#[serial]` or
             giving it its own process would hide a defect that is real outside
             the suite; the suite found something and the finding is the point.
             ⚠ `fcntl(2)` is not yet in `crates/podbox-probe/src/sys.rs` and is
             two lines there.
Prove:       `cargo test -p podbox-image a_fork_while_the_lock_is_held_does_not_extend_it` passes, and it fails with `O_CLOEXEC` removed from `Store::hold`
