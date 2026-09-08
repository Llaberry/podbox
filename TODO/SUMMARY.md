# Session summary, 2026-09-08 (M1, image acquisition)

⭐ Saved beside the record so it survives the chat scrolling away.
[PROGRESS.md](PROGRESS.md) is the record and carries the work order. This file
carries only what one session moved.

⚠ **The Changes row excludes this file, deliberately.** A diffstat written into
a file that is part of the diff is stale the moment it is committed, and the
last session corrected the same row twice for that reason. The command in the
row excludes `SUMMARY.md` and is therefore stable under any further edit to it.

**Task:** M1, image acquisition: [milestones.md](milestones.md) T-1102 and,
under it, [image.md](image.md) T-0201 to T-0204. Then [probe.md](probe.md)
T-0111, the probe cache, which lands beside the store and not before it.

| row | before | after | from |
| --- | --- | --- | --- |
| Commits | `a4ab727` | 9 commits | `git log a4ab727..HEAD --oneline \| wc -l` |
| Changes | | 44 files, +7,784 / -330 | `git diff --shortstat a4ab727..HEAD -- . ':!TODO/SUMMARY.md'` |
| podbox implementation code | 4,116 lines, 11 files | **9,098 lines, 27 files** | `wc -l crates/podbox-{probe,image,cli}/src/*.rs` |
| ⭐ Release binary | 496,184 bytes | **2,130,672 bytes**, +1,634,488 | `experiments/110-bloat-delta.sh image` |
| Headroom under the ceiling | 7,503,816 | 5,869,328 of 8,000,000 | the same |
| `PT_INTERP` | none | **still none** | `readelf -l`, the same script |
| Third-party crates in the artefact | **0** | 88 | `cargo tree`, the same script |
| TODO entries | 87 | **94**, seven authored and not implemented | `check-todo.py` |
| Entry statuses | 59 open, 3 partial, 2 blocked, 23 done | **59 open, 5 partial, 2 blocked, 28 done** | `check-todo.py` |
| Gate checks | 17 | 17, unchanged | `scripts/check-todo.py` |
| Plant cases | 18 caught, 0 missed | 18 caught, 0 missed, 3 controls quiet | `scripts/plant.sh` |
| Rust tests | 44 | **127** (10 cli, 67 image, 50 probe) | `cargo test --workspace` |
| Experiment scripts | 14 | 18 | `ls experiments/*.sh` |
| Committed results | 36 | 41 | `git ls-files experiments/results/` |
| Checks | | gate 0, plant 0, markers 0, fmt 0, clippy 0 with **0 warnings**, tests 0, musl build 0, `PT_INTERP` 0, `110-` 0, `130-` 0, `140-` 0, `150-` 0, `160-` 0, `170-` 0 | each read unpiped |
| Health | clean | clean, 0 uncommitted | `git status` |
| Debt cleared | | none was outstanding | |
| Debt introduced | | none | |

## M1

```
$ podbox pull alpine:latest && podbox images --format '{{.Digest}}' alpine:latest
sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
$ docker image inspect alpine:latest --format '{{index .RepoDigests 0}}' | cut -d@ -f2
sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
```

`experiments/150-image-acquisition.sh` is that acceptance as one command, and it
also carries [T-0201](image.md)'s and [T-0202](image.md)'s `Prove` commands.

⭐ **The digest recorded is the one computed over the bytes the registry served
for the reference that was asked for.** For a multi-platform tag that is the
**index** digest, not the per-platform manifest digest. Recording the wrong one
is exactly the parity this milestone is accepted on, and the store carries both.

## The specification's probe cache key does not work, in one run

⭐ `TOOL.md` section 6.1 says to key on `/proc/sys/kernel/random/boot_id`.
`experiments/170-probe-cache.sh` prints the boot id and the rung in both places:

```
host      rung=namespace  boot_id=b538b475-930e-4dd1-9dec-3098dd77212f
confined  rung=chroot     boot_id=b538b475-930e-4dd1-9dec-3098dd77212f
```

Equal ids, different verdicts, because the boot id is the **kernel's**. podbox
keys on the boot id, the mount-namespace inode, the three ID-map files and the
two seccomp fields, and the confined run named all six that moved and re-probed.
⛔ A cache keyed on the boot id alone serves the host's `namespace` to a
`chroot` process, which is the exact lie podbox exists to refuse.

## Eleven defects found by driving things rather than reading them

Four are in podbox's own code. The other six are in the harness this session
wrote, and five of those came out of re-running each script against its own
committed reading rather than looking at it once.

1. ⛔ `hex.bytes().all(|b| b.is_ascii_lowercase() && b.is_ascii_hexdigit())`
   refused **every real digest**: `0`-`9` are not lowercase letters. Caught by
   the store's own tests, which is what they are for.
2. docker's rule that a first component carrying a dot is a registry read
   `../etc/passwd` as a registry called `..`, and the repository check never saw
   it. The domain has its own validator now.
3. `Path::file_name` returns `None` for a path ending in `..`, so the
   containment walk could not take such a path apart at all. `..` is refused up
   front now, before anything resolves.
4. The pull transcript printed the **config blob** as a third `Pull complete`
   beside two layers, which reads as an image with three layers. docker's
   transcript names layers only.
5. `flock FILE -c 'sleep N'` runs the sleep as a **child**, which inherits the
   locked fd, so killing `flock` left the lock held. ⭐ That is
   [T-0204](image.md)'s mechanism working exactly as designed, arriving as a
   defect in the harness.
6. ⛔ `experiments/20-enter-target.sh --stage <dir>` **nests on a re-run**.
   `cp -a src/store dest/store` with `dest/store` present produces
   `dest/store/store`, so the second run reads the first run's copy. The clause
   passed for a reason that had nothing to do with what it was testing. Staging
   a FILE overwrites, which hid it for as long as only files were staged.
7. `150-`'s committed reading recorded `http_refused_rc 125` against code that
   exits **2**. Evidence taken before a change, disagreeing with the code it is
   evidence for. `result-diff.sh` caught it, from a **fresh clone**.
8. ⛔ `grep -c 'Pull complete' || echo 0` wrote **two** lines. `grep -c` prints
   0 and exits 1 on zero matches, so the fallback fired beside the real value.
   That is the trap `docs/AGENTS.md` names twice, written into a new script by
   the session that had just read it.
9. ⛔ A registry answering `HTTP 429` made `140-` report **FAIL** for a clause
   that never ran, and `160-` report **SUCCESS** for the same. Both discriminate
   and exit 2 now; `150-` already did.
10. `140-` recorded the `mktemp` directory into its transcript, so the file
    could **never** reproduce, and its inode clause was not deterministic. Both
    fixed, and all four scripts now reproduce their committed readings.
11. ⭐ **The worst of them, and the last found.** `--format` was validated
    inside the loop over records. An empty store means zero iterations, so
    `podbox images --format '{{.Nope}}'` printed nothing and exited **0**: a
    caller's typo reading as an empty result set. `--format 'table {{.Tag}}'`
    printed `table latest` and exited 0. Both are refusals now, checked before
    the store is opened. [T-0807](cli.md).

## The toolchain, which stopped being inert

⛔ **`cargo build` failed and `cargo build --release` succeeded**, on the same
tree, the moment a member took the `rustls` pin:
`undefined reference to '__ubsan_handle_type_mismatch_v1'`. `zig cc` with no
`-O` compiles in its Debug mode and emits calls to handlers in its own
`compiler-rt`; `cc-rs` passes no `-O` in a debug profile and `-O3` in release.

⚠ **Making `zig cc` the linker as well was tried and is worse**: zig's driver
adds its own `crt1.o` despite rustc's `-nostartfiles`, and the link dies on
`duplicate symbol: _start`. `scripts/zig-cc.sh` passes `-fno-sanitize=undefined`
instead, and `ZIG_SANITIZE=1` restores it.

⚠ **cargo does not track the contents of a `CC` program**, so `cargo clean -p ring`
is needed after editing that wrapper.

## New experiments

- `140-space-precheck.sh`: blocks and inodes, on two real tmpfs mounts rather
  than fixtures. Both refusals name the destination, the free amount, the
  required amount and the unit, and **0 blobs** were written before either.
- `150-image-acquisition.sh`: M1's acceptance. ⚠ It detects a moved tag rather
  than reporting it as a wrong digest.
- `160-store-gc.sh`: a GC under a holder, and the containment check against a
  blob path that resolves outside the store.
- `170-probe-cache.sh`: the cache, and the key the specification got wrong.
- ⚠ [T-0203](image.md)'s `Prove` named `90-space-precheck.sh`, and 90 is
  `experiments/90-nsswitch-contract.sh`. `experiments/README.md` rules that a
  number is never reused. The entry carries the correction.

## Docker Hub's rate limit

⚠ `HTTP 429: TOOMANYREQUESTS` arrives after enough anonymous pulls from one
address, and this session's own runs reached it. `150-` must ask docker about
the same tag and cannot leave the Hub, so it exits **2** and says so when the
quota is spent, which is correct rather than a failure. `140-` and `160-`
compare nothing against docker and now default to
`public.ecr.aws/docker/library/alpine:latest`. Establishing that drove the
challenge-driven auth against **four registries** rather than one: Docker Hub,
`public.ecr.aws`, `ghcr.io` and `quay.io`.

## The four review passes

1. **Is it true?** Four reference citations opened at their lines; all four say
   what the entries say they say. Every number in [PROGRESS.md](PROGRESS.md)
   re-read from the thing that produces it; all matched. ⚠ One apparent finding
   was not one: a grep reported two declarations of the size ceiling, and the
   second is `scripts/plant.sh` READING the value out of its one home.
2. **Is it consistent?** Three sentences had outlived their numbers and are
   corrected. Every verb the documents name is dispatched, every experiment the
   README names exists, the counts agree in three places.
3. **Is it usable cold?** Twenty error cases against an **empty store**, which
   is the state a first-time user is in. ⭐ It found defect 11 above, the worst
   of the session.
4. **What is the worst input, ordering and partial failure?** Two concurrent
   pulls into one store: both exit 0, one image, four blobs, index parses, no
   staging left. An interrupted pull leaves nothing in `blobs/`. No bearer token
   in the transcript or under the store. ⚠ The gap it found is recorded rather
   than fixed, because fixing it is not M1's scope: SIGKILL mid-blob leaves a
   `*.partial` nothing sweeps. [T-0210](image.md).

## Seven entries authored, none implemented

Per `docs/AGENTS.md`'s routing table, authoring is its own pass.

| entry | size | what it is |
| --- | --- | --- |
| [T-0206](image.md) | L | a registry fixture, so M1's acceptance stops depending on Docker Hub's quota |
| [T-0207](image.md) | L | bounded-concurrency layer fetch, with the wall time measured both ways |
| [T-0208](image.md) | L | `--platform`, and a store keyed by variant so two do not overwrite each other |
| [T-0209](image.md) | L | registry authentication, with no credential entering this tree |
| [T-0210](image.md) | L | the store's concurrency contract, written down and driven by a stress run |
| [T-1204](gate.md) | S | check 17 holds the newest committed reading, not only M0's baseline |
| [T-0807](cli.md) | S | ⭐ `done`: the review pass that found it also fixed it |

## What did not move

- **`experiments/30-attribution-census.sh` still exits 2.** This kernel has no
  Landlock, so the three M rows cannot run.
- ⚠ **`controls_answered` is false on every run of this host**, because
  `kcmp(2)` is not built into this kernel. Correct, and not clearable here.
- ⚠ **`cargo test` prints `error: Unrecognized option: 'probe-child'` many times
  and exits 0.** Under `cargo test` the probe re-executes the libtest binary,
  which rejects the flag, and each row becomes a `skip` with its reason. The
  volume rose because `podbox-image`'s tests exercise the real
  `probe_cache::resolve`. Recorded rather than filtered, so a future session
  does not chase it.
- ⚠ **`{{.Size}}` is the compressed bytes podbox holds**, headed
  `SIZE (STORED)`. docker's is the uncompressed total, which M1 has not
  measured. [PROGRESS.md](PROGRESS.md) open question 6.
- ⚠ One number in the last session's own record was quoted wrong:
  `PROGRESS.md`'s acceptance block said `37 passed` where the total was 44,
  which is one crate's line quoted as the total. Measured in a worktree at
  `a4ab727` rather than taken from either document.
