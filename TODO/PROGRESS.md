# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M1 complete. `podbox pull` fetches an image over HTTPS into a
content-addressed store, and the digest it reports is the digest
`docker image inspect` reports. `images`, `rmi`, `tag`, `image prune` and
`inspect` work; the probe cache lands beside the store and is already on the
`pull` hot path. M2, extraction, is next.**
Session of 2026-09-08, on `claude/podbox-m1-image-acquisition-8p2n3d`; see open
question 7.

[INDEX.md](INDEX.md) is the list. [RULES.md](RULES.md) is how this repository is
worked on. [reference-map.md](reference-map.md) is the corpus.
[`../docs/AGENTS.md`](../docs/AGENTS.md) is the router: a session with no memory
reads that first and it names everything else.

## The measured baseline

One machine, one day: kernel `6.18.44-fc-v24` (a Firecracker guest), rustc
1.98.1, cargo 1.98.1, GNU tar 1.35, docker 29.3.1, glibc 2.39, musl 1.2.4,
zig 0.16.0, 4 CPUs, 16 GB RAM.
⚠ `CONFIG_SECURITY_LANDLOCK` is unset on this kernel, so the M-mechanism rows of
the reconstruction cannot run here and report `SKIP`.
⚠ `CONFIG_CHECKPOINT_RESTORE` is also unset, so `kcmp(2)` answers `ENOSYS`. That
is a control of the bogus-argument discriminator, and the consequence is
[T-0102](probe.md)'s correction, which the entry carries.

| what | value | taken by |
| --- | --- | --- |
| ⭐ release binary, M1 complete, `x86_64-unknown-linux-musl` | 2,130,672 bytes | `experiments/110-bloat-delta.sh image`, T-0910 |
| ⭐ the delta M1's dependencies cost | **+1,634,488 bytes** on 496,184 | the same |
| headroom under the ceiling | 5,869,328 bytes of 8,000,000 | the same |
| `PT_INTERP` in the M1 binary | none. still static-pie | `readelf -l`, the same script |
| third-party crates in the normal graph | 88 | `cargo tree`, the same script |
| release binary, M0 complete | 496,184 bytes | `experiments/results/bloat-baseline.txt`, T-0910 |
| release binary, empty skeleton | 389,656 bytes | `cargo build --release`, T-1100 |
| ⭐ `podbox images --format '{{.Digest}}' alpine:latest` | `sha256:28bd5fe8b56d…43f8b` | `experiments/150-image-acquisition.sh` |
| ⭐ `docker image inspect`'s `RepoDigests[0]` for the same tag | **the same value** | the same script |
| blobs a fresh `alpine:latest` pull stores | 4, all hashing to their own names | the same script |
| a second pull of the same tag | 0 layers fetched, every one `Already exists` | the same script |
| `podbox pull http://…` | exit **2**, "podbox speaks HTTPS only", under a 30 s timeout | the same script |
| a pull into a 1 MiB tmpfs | refused, 0 blobs written, message names all four things | `experiments/140-space-precheck.sh` |
| a pull with 6 free inodes and 256 MiB free | refused, and the message names inodes | the same script |
| registries the challenge-driven auth answers | 4: Docker Hub, `public.ecr.aws`, `ghcr.io`, `quay.io` | driven by hand, and `140-`/`160-` now default to the second |
| `rmi` while something holds the image | exit 125, "is in use by a running container" | `experiments/160-store-gc.sh` |
| `image prune -af` under the same holder | exit 0, `skipped:` names it, image kept | the same script |
| a blob path resolving outside the store | refused by name; the canary survived | the same script |
| ⭐ boot id: host, and inside the reconstruction | **identical**, `b538b475-930e-4dd1-9dec-3098dd77212f` | `experiments/170-probe-cache.sh` |
| ⭐ rung: host, and inside the reconstruction | `namespace` and `chroot` | the same script |
| the cache key components that differ between them | `mnt_ns`, `uid_map`, `gid_map`, `setgroups`, `seccomp`, `seccomp_filters` | the same script |
| `podbox probe` rung, unconfined on this host | `namespace` | `experiments/130-probe-parity.sh` |
| `podbox probe` rung, inside `experiments/20-enter-target.sh` | `chroot` | `experiments/130-probe-parity.sh` |
| attribution rows against `experiments/results/attribute.txt` | 15 matched, 1 recorded divergence, 0 differed | `experiments/130-probe-parity.sh` |
| probes in the set | 50: 34 census, 16 attribution | `podbox probe --json` |
| `/dev/ptmx` here, unconfined | present and openable: chardev 5:2, mode 666 | `podbox probe --json \| jq .ptmx`, T-0503 |
| `/dev/ptmx` inside the reconstruction | `ENOENT` to both `stat` and `open`. ⛔ Not an answer about the target | the same |
| `kcmp(-1,-1,...)` control, on this host | `ENOSYS`: the control cannot answer | `experiments/results/attribute.txt` |
| `kcmp(-1,-1,...)` control, on the target | `ESRCH`: executed | `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26` |
| writable paths obtained inside the reconstruction | 4 of 8 probed: `/tmp`, `/dev/shm`, `/workspace`, `/state` | `podbox probe --json` |
| interposer cdylib under `+crt-static` | refused by cargo | `experiments/60-interposer-libc.sh` |
| interposer cdylib, `-crt-static`, musl target, linked by `zig cc` | 286,056 bytes, `DT_NEEDED libc.so` | `experiments/60-interposer-libc.sh`, which exits 0 |
| interposer cdylib, `-crt-static`, gnu target | 266,632 bytes | `experiments/60-interposer-libc.sh` |
| podbox's own musl cdylib into a glibc payload | **refused**, `libc.so: invalid ELF header`, rc 127 | `experiments/60-interposer-libc.sh` check B |
| musl object into a glibc payload | **refused**, `libc.so: invalid ELF header` | `experiments/80-interposer-abi.sh` |
| glibc object into a musl payload | **refused**, `__snprintf_chk: symbol not found` | `experiments/80-interposer-abi.sh` |
| a `GLIBC_2.34` import against a libc declaring `GLIBC_2.31` | refused, and the loader names the version | `experiments/80-interposer-abi.sh` |
| `libc.so.6` symbol tables | `.symtab` 0 defined, `.dynsym` 3136 | `experiments/80-interposer-abi.sh` |
| an interposer defining `execve` alone | catches 1 of 7 exec entry points | `experiments/100-interpose-symbols.sh` |
| the same seven with every entry point defined | 7 of 7 | `experiments/100-interpose-symbols.sh` |
| the adopted mechanism's exported set | 129 symbols: 113 libc entry points, 16 internal | `experiments/100-interpose-symbols.sh` |
| path-taking names a pinned debian rootfs reaches | 38, of which 3 are undefined | `experiments/100-interpose-symbols.sh` |
| a supplied `/etc/passwd` under `nsswitch: files` | read | `experiments/90-nsswitch-contract.sh` |
| the same file under a non-`files` service | **ignored**, `getpwnam` returns NULL | `experiments/90-nsswitch-contract.sh` |
| glibc gconv modules recording `DT_NEEDED libc.so.6` | 253 of 253 | `experiments/90-nsswitch-contract.sh` |
| distributions where a supplied `/etc/passwd` is read | 10 of 11 | `experiments/125-across-distributions.sh` |
| distinct `nsswitch` `passwd` shapes across those 11 | 6 | `experiments/125-across-distributions.sh` |
| a static glibc binary on `opensuse-leap-15.6` | **SIGFPE**, rc 136 | `experiments/125-across-distributions.sh` |
| gate coverage | ⛔ not recorded here. It is **self-referential**: writing the number down changes it | `scripts/check-todo.py`, on every run |
| the gate's checks, planted against | 17 checks, 18 cases; see the acceptance block below | `scripts/plant.sh` |
| corpus | 30 trees, 154 MB in a fresh clone | `scripts/common/mine-repo.sh` |
| `alpine:3.20` `etc/shadow` ownership | uid 0, gid 42 | `experiments/70-whiteout-contract.sh` |
| OCI layer member-name prefix | no `./` on any layer of either pinned image | `experiments/70-whiteout-contract.sh` |
| a slash-anchored whiteout glob against a layer-root whiteout | misses it | `experiments/70-whiteout-contract.sh` |

Acceptance, run on 2026-09-08:

```
$ ./scripts/check-todo.py
check-todo: 94 rows, 94 entries, 59 open, 5 partial, 2 blocked, 28 done
check-todo: ok
$ ./scripts/plant.sh
  plants   18 caught, 0 missed
  controls 3 quiet, 0 fired
$ cargo test --workspace
test result: ok. 127 passed; 0 failed   (10 cli, 67 image, 50 probe)
$ cargo build --release --target x86_64-unknown-linux-musl
    Finished `release` profile [optimized] target(s)
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
$ ./experiments/110-bloat-delta.sh image
  total_bytes 2130672   headroom 5869328   delta +1634488   third-party crates 88
$ ./experiments/130-probe-parity.sh
  got chroot / got namespace / 15 matched, 1 recorded divergence, 0 differed, 0 missing
$ ./experiments/140-space-precheck.sh
  refused on blocks and on inodes, 0 blobs written
$ ./experiments/150-image-acquisition.sh
  podbox and docker report the same digest; 4 blobs, 0 mismatched
  ⚠ a later re-run exited 2: Docker Hub's anonymous pull rate limit. See below
$ ./experiments/160-store-gc.sh
  rmi refused under a holder, prune named what it skipped, the canary survived
$ ./experiments/170-probe-cache.sh
  one boot id, two rungs; the confined run refused the host's cache
```

## Counts

94 entries: 59 open, 5 partial, 2 blocked, 28 done.

⚠ Seven entries were **authored and not implemented** this session, in their own
pass per `docs/AGENTS.md`'s routing table: [T-0206](image.md) to
[T-0210](image.md), [T-1204](gate.md) and [T-0807](cli.md). The last is `done`
because the review pass that found it also fixed it; the other six are open and
five of them are `L`.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

⭐ **M1, image acquisition**, and the first podbox code that talks to a network.
`crates/podbox-image` is 12 modules and is the first member to take a
`[workspace.dependencies]` pin, so the sweep of the last session stopped being
inert.

1. **The registry client.** [T-0201](image.md), `done`. Challenge-driven bearer
   auth, manifests by tag and by digest, indexes resolved to `linux/amd64`,
   blobs streamed. HTTPS only, at every layer.
2. **The store.** [T-0202](image.md), `done`. Blobs under `sha256:<hex>`,
   verified **as they are written**, renamed into `blobs/` only on success, and
   re-verified on the way out. Images keyed by the digest of the bytes the
   registry served for the reference that was asked for, which is what makes the
   milestone's parity hold.
3. **The space precheck.** [T-0203](image.md), `partial`. Blocks **and** inodes,
   before any download, with the destination, the free amount, the required
   amount and the unit in the message. The second call site is before an
   extraction that does not exist until M2.
4. **`images`, `rmi`, `tag`, `image prune`, `inspect`, and the GC lock.**
   [T-0204](image.md), `partial`. An advisory lock on an fd opened **without**
   `O_CLOEXEC`, so it survives an exec. The `run -d` half of its acceptance is
   M3.
5. **The probe cache.** [T-0111](probe.md), `done`, beside the store as its own
   `Decision` said it would be, and already on the `pull` hot path rather than
   waiting for M3.
6. **[T-1102](milestones.md), the milestone, closed against a script**:
   `experiments/150-image-acquisition.sh`.

### The specification's cache key is wrong, and one run proves it

⭐ `TOOL.md` section 6.1 says to key the probe cache on
`/proc/sys/kernel/random/boot_id`. `experiments/170-probe-cache.sh` prints both
the boot id and the rung, on the host and inside the reconstruction, in one run:

```
host      rung=namespace  boot_id=b538b475-930e-4dd1-9dec-3098dd77212f
confined  rung=chroot     boot_id=b538b475-930e-4dd1-9dec-3098dd77212f
```

Equal ids, different verdicts, because the boot id is the **kernel's**. podbox
keys on the boot id, the mount-namespace inode, the three ID-map files and the
two seccomp fields, and the confined run named all six that moved and re-probed.

### Ten defects found by driving the thing rather than reading it

⭐ Each was found by running something. Four are in podbox's own code; the rest
are in the harness this session wrote, and every one of those came out of
re-running a script against its own committed reading rather than looking at it
once.

| what looked right | what it did |
| --- | --- |
| `hex.bytes().all(|b| b.is_ascii_lowercase() && b.is_ascii_hexdigit())` | refused **every real digest**: `0`-`9` are not lowercase letters. Caught by the store's own tests, which is what they are for |
| docker's rule that a first component with a dot is a registry | read `../etc/passwd` as a registry called `..`, and the repository check never saw it. The domain has its own validator now |
| `Path::file_name` on a candidate ending in `..` | returns `None`, so the containment walk could not take the path apart at all. `..` is refused up front now, before anything resolves |
| the pull transcript | printed the **config blob** as a third `Pull complete` beside two layers, which reads as an image with three layers. docker names layers only |
| `flock FILE -c 'sleep N'` as a lock holder | the sleep is a **child** and inherits the locked fd, so killing `flock` left the lock held. That is T-0204's mechanism working exactly as designed, arriving as a defect in the harness |
| ⛔ `20-enter-target.sh --stage <dir>` | **nests on a re-run.** `cp -a src/store dest/store` with `dest/store` present produces `dest/store/store`, so a second run reads the first run's copy. The clause passed for a reason that had nothing to do with what it was testing. Staging a FILE overwrites, which hid it for as long as only files were staged |
| `150-`'s committed reading | recorded `http_refused_rc 125` against code that exits **2**. Evidence taken before a change, disagreeing with the code it is evidence for. `result-diff.sh` caught it from a FRESH CLONE |
| ⛔ `grep -c 'Pull complete' \|\| echo 0` | wrote **two** lines. `grep -c` prints 0 and exits 1 on zero matches, so the fallback fired beside the real value. That is the trap `docs/AGENTS.md` names twice, written into a new script by the session that had just read it |
| ⛔ a registry answering `HTTP 429` | `140-` reported **FAIL** for a clause that never ran and `160-` reported **SUCCESS** for the same. Both discriminate and exit 2 now. `150-` already did |
| `140-`'s inode clause | was not deterministic: a 12-inode tmpfs may or may not fit the store's own directories, so the record flipped between two honest values on re-runs |
| `140-`'s recorded transcript | carried the `mktemp` directory, so the file could **never** reproduce. Redacted, as `130-` already does |

### `zig cc` stopped being inert, and cost one measurement to wire up

⛔ **`cargo build` (debug) failed at the link and `cargo build --release`
succeeded**, on the same tree, the first time a member took the `rustls` pin:

```
libring-*.rlib(...curve25519.o): undefined reference to `__ubsan_handle_type_mismatch_v1'
```

`zig cc` with no `-O` compiles in its Debug mode, which instruments with UBSan
and emits calls to handlers in zig's own `compiler-rt`; `cc-rs` passes no `-O`
in a cargo debug profile and `-O3` in release, which is why only one broke.

⚠ **The obvious fix was tried and is worse.** Making `zig cc` the linker as
well fails on `duplicate symbol: _start`: zig's driver adds its own `crt1.o`
despite the `-nostartfiles` rustc passes. `scripts/zig-cc.sh` passes
`-fno-sanitize=undefined` instead, which makes the debug profile match the
release profile the artefact actually is, and `ZIG_SANITIZE=1` restores it.

⚠ **Changing that wrapper does not rebuild `ring`.** cargo does not track the
contents of a `CC` program, so `cargo clean -p ring` is needed after editing it.

### Docker Hub's rate limit, and what it changed

⚠ **`HTTP 429: TOOMANYREQUESTS: You have reached your unauthenticated pull rate
limit`** arrives after enough anonymous pulls from one address, and this
session's own runs reached it. It is a condition of re-running `150-`, which
must ask docker about the same tag and therefore cannot leave the Hub; the
script exits **2** and says so, which is correct and is not a failure of
anything it measures.

⭐ `140-` and `160-` compare nothing against docker, so they need a registry
rather than THE registry, and their default is now
`public.ecr.aws/docker/library/alpine:latest`. `PODBOX_TEST_IMAGE` overrides all
three. Establishing that also drove the challenge-driven auth against four
registries rather than one.

### One reading in the test output that is not a failure

⚠ **`cargo test` prints `error: Unrecognized option: 'probe-child'` many times
and exits 0.** Every probe re-executes `self_exe` with `--probe-child`, and
under `cargo test` `self_exe` is the libtest binary, which rejects the flag. The
probe records each as a `skip` with its reason, which is correct. The volume
rose this session because `podbox-image`'s tests exercise the real
`probe_cache::resolve`, which runs the whole set. ⛔ It is recorded rather than
filtered, because `docs/methodology/gate.md` says to count the test files the
runner reports against the files on disk: a green pass count beside an
unexplained error line is a file that never ran, and this one is explained.

### Four new experiments, and the numbering rule that renamed one

- `experiments/140-space-precheck.sh`, `150-image-acquisition.sh`,
  `160-store-gc.sh`, `170-probe-cache.sh`. All four exit 0, all four print
  their conditions, and all four end with `result-diff.sh`.
- ⚠ [T-0203](image.md)'s `Prove` named `90-space-precheck.sh`, and 90 is
  `experiments/90-nsswitch-contract.sh`. `experiments/README.md` rules that a
  number is never reused. The entry carries the correction and the `Prove` names
  `140-`.
- ⚠ `150-` and `160-` pull `alpine:latest`, which is a **moving tag**. `150-`
  detects that case rather than reporting it as a wrong digest: it re-pulls both
  once and says which of the two it was. `PODBOX_TEST_IMAGE` removes the race.

### What the four review passes found

⭐ Each pass asked a different question, and each found something.

1. **Is it true?** Every reference citation in the new code was opened at its
   line: `onelf`'s verify-before-exec at `main.rs:149-157` and its
   lock-through-exec at `main.rs:275-278`, dwarfs's status-code-as-byte-count at
   `filesystem_extractor.cpp:544-552`, and memfd-exec's unchecked temp-dir
   fallback at `executable.rs:580-584`. All four say what the entries say they
   say. Every number written into this file was re-read from the thing that
   produces it: the binary size, the delta, the headroom, the crate count, the
   line and file counts, the script and result counts. All matched. ⚠ One
   apparent finding was not one: a grep reported two declarations of the size
   ceiling, and the second is `scripts/plant.sh` READING the value out of its
   one home rather than declaring it.
2. **Is it consistent?** Three numbers had moved and their sentences had not:
   the root `README.md` said `podbox probe` works and nothing else does,
   `experiments/README.md` listed neither the four new scripts nor the daemon
   they need, and `.cargo/config.toml` still called the `zig cc` lines inert.
   All three are corrected. Every verb the documents name is dispatched, every
   experiment the README names exists, and the counts agree across
   [INDEX.md](INDEX.md), this file and the rows.
3. **Is it usable cold?** The whole M1 surface was driven as a first-time user
   would, including twenty error cases against an **empty store**, which is the
   state a first-time user is actually in. ⭐ **That is where the worst defect
   of the session was found**: `--format` was validated inside the loop over
   records, an empty store means zero iterations, and
   `podbox images --format '{{.Nope}}'` printed nothing and exited **0**. A
   caller's typo read as an empty result set. `--format 'table {{.Tag}}'`
   printed `table latest` and exited 0. Both are refusals now, checked before
   the store is opened. [T-0807](cli.md) carries it.
4. **What is the worst input, ordering and partial failure?** Two concurrent
   pulls into one store: both exit 0, one image, four blobs, the index parses,
   no staging file left. An interrupted pull: nothing in `blobs/`, and the next
   pull succeeds. No bearer token in the transcript or anywhere under the store.
   ⚠ **The gap this pass found is recorded rather than fixed**, because fixing
   it is not M1's scope: a process killed with SIGKILL mid-blob leaves a
   `*.partial` file that nothing sweeps, and the orderings that were NOT driven
   are the interesting ones. [T-0210](image.md) is that work, and it is `L`.

## In progress

Nothing. [T-0203](image.md) and [T-0204](image.md) are `partial` with exactly
one half named each, and each names the milestone that half lands in. The seven
entries authored this session are authored and not started, which is what
`docs/AGENTS.md`'s routing table requires of an authoring pass.

## The work order

⭐ **This is the only work order.** Do not take one from the index or from a
kickoff prompt.

1. **M2, extraction.** [T-1103](milestones.md) and [extract.md](extract.md).
   The highest-risk component, and three of its four acceptance criteria are
   already measured. ⭐ It also closes the half [T-0203](image.md) is `partial`
   for: the second `statvfs` call site, before an extraction that now exists.
   `tar`, `flate2` and `ruzstd` are pinned in `[workspace.dependencies]` and
   are still untaken by any member.
2. **M3, `run`.** [T-1104](milestones.md), [enter.md](enter.md),
   [cli.md](cli.md). ⭐ It closes three halves: [T-0107](probe.md),
   [T-0108](probe.md) and [T-0204](image.md), and all three name exactly what is
   left. It is also where `podbox probe --cached` gets its second caller.
3. Then M4, M5, M6, M7 in order. [T-0709](interpose.md) and
   [T-0410](complete.md) are both P0 and both land inside M6 and M5
   respectively; neither needs a measurement that has not been taken.

⭐ **Two of this session's own entries are P1 and belong beside M2 rather than
after M7**, because both are about M1's own gate rather than about a new
capability:

- [T-0206](image.md), the registry fixture, `L`. M1's acceptance currently
  depends on Docker Hub's anonymous quota, and this session exhausted it. A gate
  a third party can turn red is not a gate.
- [T-0210](image.md), the store's concurrency contract, `L`. One ordering of one
  case was driven; the contract is not written down, so every future change to
  the store is a change to an unstated one.
- [T-1204](gate.md), `S`. Check 17 holds M0's baseline under the ceiling and no
  longer holds the shipping binary, which is now four times larger.

⚠ [T-0207](image.md), [T-0208](image.md) and [T-0209](image.md) are P2 `L`
entries and are not in the order. Each names what it needs and none blocks a
milestone.

⚠ [T-0205](image.md) and [T-1004](packaging.md) are P3 and are not in the order.
They are worth doing when something else touches the same ground.

⚠ [T-0801](cli.md), the verb and flag parity table, and [T-0802](cli.md),
docker's exit codes unaltered, are M3's. Until they land the CLI's contract is
the one [T-0110](probe.md) settled: invalid input exits 2, a runtime failure
exits 125. The M1 verbs follow that and not docker's per-verb codes.

## Open questions for the operator

1. **`/dev/ptmx` on the target**, and it is ⭐ **one command rather than a
   research task**: `podbox probe --json | jq .ptmx`, run on the target by
   anyone who can reach it. The probe half of [T-0503](enter.md) is implemented
   and reads `present: true, usable: true` on this host and `ENOENT` inside the
   reconstruction. ⛔ **The reconstruction does not settle it**: it builds
   `/dev` from the same mount table the question doubts, so its answer is that
   table repeated back. What is still needed is a run on the target itself.
2. **A kernel with Landlock**, to run the three M-mechanism rows of
   `experiments/30-attribution-census.sh`. Any distro kernel has it. This host
   does not, so those rows report `SKIP` and the script exits 2. ⭐ It would also
   close the one row of M0's own acceptance that this host cannot exercise:
   `move_mount(-> /tmp/mm-probe)` attaches here and is `EPERM` on the target,
   so podbox's `chroot` rung was selected without the LSM ever being present.
3. **A kernel with `CONFIG_CHECKPOINT_RESTORE`**, so the `kcmp(2)` control of
   the bogus-argument discriminator can answer. Without it `podbox probe`
   reports `controls_answered: false` on every run of this host, which is
   correct and is also a permanent notice nobody can clear here. The target has
   it. [T-0102](probe.md).
4. **Whether `podbox` should refuse to install itself as `docker` where a
   working docker daemon exists.** [T-0803](cli.md) proposes refusing without an
   explicit flag. A machine with a working daemon is a machine where podbox is
   the wrong tool, and the alternative reading is that podbox should defer to it
   transparently. The entry carries the recommendation and not a ruling.
5. **Why a static glibc binary takes SIGFPE on `opensuse-leap-15.6`.**
   Measured by `experiments/125-across-distributions.sh` and recorded as a
   reading, not a diagnosis. It bears on what podbox may assume about payloads
   in an image, and the row is also the one distribution whose `nsswitch: compat`
   setting is therefore untested for the passwd question.
6. ⭐ **What `{{.Size}}` should report.** docker's `SIZE` is the sum of the
   **uncompressed** layers; M1 does not extract, so podbox prints the
   compressed bytes it holds and heads the column `SIZE (STORED)` rather than
   printing a number it has not measured under docker's heading. M2 makes the
   uncompressed figure available. The alternative is to keep reporting stored
   bytes and document the divergence permanently, and [T-0202](image.md)
   carries both readings.
7. ⚠ **The branch.** [`../docs/AGENTS.md`](../docs/AGENTS.md)'s first absolute
   and [RULES.md](RULES.md) section 2 both say `main`. The harness this session
   ran under named `claude/podbox-m1-image-acquisition-8p2n3d` and said never to
   push elsewhere without permission, and this session's operator prompt named
   no branch. `docs/AGENTS.md`'s closing section orders the operator's word
   first, and the only branch the operator named this session is that one, so
   work is on it. ⛔ **The previous session resolved the same contradiction the
   other way**, because its prompt said `main` in so many words. That the answer
   flips with the prompt is the finding, and a standing ruling would end it.
