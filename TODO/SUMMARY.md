# Session summary, 2026-09-08 (M2, extraction)

⭐ Saved beside the record so it survives the chat scrolling away.
[PROGRESS.md](PROGRESS.md) is the record and carries the work order. This file
carries only what one session moved.

⚠ **The Changes row excludes this file, deliberately.** A diffstat written into
a file that is part of the diff is stale the moment it is committed, and the
last session corrected the same row twice for that reason. The command in the
row excludes `SUMMARY.md` and is therefore stable under any further edit to it.

**Task:** M2, extraction: [milestones.md](milestones.md) T-1103 and, under it,
[extract.md](extract.md) T-0301 to T-0307. It also closes the half
[image.md](image.md) T-0203 was `partial` for, and [gate.md](gate.md) T-1205,
whose collision blocks M2's own acceptance script on its first day.

| row | before | after | from |
| --- | --- | --- | --- |
| Commits | `106aa6d` | 3 commits | `git log 106aa6d..HEAD --oneline \| wc -l` |
| Changes | | 27 files, +3,900 / -60 | `git diff --shortstat 106aa6d..HEAD -- . ':!TODO/SUMMARY.md'` |
| podbox implementation code | 9,098 lines, 27 files | **12,286 lines, 36 files** | `wc -l crates/podbox-{probe,image,cli,extract}/src/*.rs` |
| ⭐ Release binary | 2,130,672 bytes | **2,282,224 bytes**, +151,552 | `experiments/110-bloat-delta.sh extract` |
| Headroom under the ceiling | 5,869,328 | 5,717,776 of 8,000,000 | the same |
| `PT_INTERP` | none | **still none** | `readelf -l`, the same script |
| Third-party crates in the artefact | 88 | 95 (`tar`, `flate2`, `ruzstd` and their trees) | `cargo tree`, the same script |
| TODO entries | 95 | **96**, one authored from a defect found at the close | `check-todo.py` |
| Entry statuses | 60 open, 5 partial, 2 blocked, 28 done | **52 open, 5 partial, 2 blocked, 37 done** | `check-todo.py` |
| ⭐ Gate checks | 17 | **18** | `scripts/check-todo.py` |
| ⭐ Plant cases | 18 caught, 0 missed | **20 caught, 0 missed**, 3 controls quiet | `scripts/plant.sh` |
| Rust tests | 127 | **173** (10 cli, 67 image, 50 probe, **46 extract**) | `cargo test --workspace` |
| Experiment scripts | 18 | 19 | `ls experiments/*.sh` |
| Committed results | 41 | 43 | `git ls-files experiments/results/` |
| Checks | | gate 0, plant 0, markers 0, fmt 0, clippy 0 with **0 warnings**, musl build 0, `PT_INTERP` 0, `70-` 0, `110-` 0, `220-` 0 | each read unpiped |
| ⚠ `cargo test --workspace` | 127 green | 173 tests, **4 runs of 6 green** | 2 red on one M1 test, [T-0211](image.md). ⚠ M1's own tip measured **8 of 8 green**, so M2 shifted the timing of a race it did not create: the defect and the fork that triggers it are both in M1's code and M2 touched neither |
| Health | clean | clean, 0 uncommitted | `git status` |
| Debt cleared | | [T-0203](image.md)'s second call site; [T-1205](gate.md)'s four collisions | |
| Debt introduced | | none. ⚠ [T-1204](gate.md)'s gap widened, and it was already open | |

## M2

```
$ podbox pull alpine:latest && podbox extract alpine:latest
55afa1ecc21d: Extracted (515 entries, 0 whiteouts)
podbox: 1 layers, 515 entries, 0 removed by whiteouts, 8.3 MiB uncompressed
podbox: 1 of 515 entries carry an id this machine cannot apply

$ jq -e 'select(.path=="etc/shadow") | .uid==0 and .gid==42 and .applied.gid==0' \
    "$(podbox inspect --format '{{.RootfsPath}}' alpine:latest)/../.meta.jsonl"
true

$ podbox extract voidlinux/voidlinux-musl:latest
91c43422d24e: Extracted (4329 entries, 0 whiteouts)
bfc5f30ae528: Extracted (335 entries, 1 whiteouts)
podbox: 2 layers, 4664 entries, 1 removed by whiteouts, 75.1 MiB uncompressed
```

⭐ **The one entry of 515 that carries an unmappable id is `etc/shadow`, gid
42** , which is exactly what `whiteout-contract.txt` check B predicted, found by
extracting rather than by being told.

## What is worth carrying forward

1. ⛔ **A defect the experiment found and no unit test could.** A refused
   extraction left a directory and a sidecar, `is_extracted` asked only whether
   those existed, and the next `podbox extract` served the half-extracted tree
   as complete with exit 0. Every decision was right and the seam was not. Two
   defences now, each with its own test.
2. ⛔ **The `O_NOFOLLOW` fallback was dead code here.** This kernel has
   `openat2`, so the pre-5.6 path had refused nothing. Both mechanisms are now
   driven on one kernel and asserted to agree.
3. ⛔ **Four `Prove` clauses could not have passed as written**: one `&&` chain
   collapsing a three-state exit, one `grep -c` into a pipeline, and two
   asserting through a verb that does not exist yet.
4. ⚠ **A premise of this tree was wrong.** docker's `.Size` here is the
   compressed content, not the uncompressed layers, and equals podbox's blob
   total to the byte. [T-0202](image.md) carries the correction.
5. ⛔ **An intermittent test failure was run to ground, not re-run.**
   `Store::hold` opens the image lock without `O_CLOEXEC` on purpose, so
   **every** child forked while it is held inherits it, not only the payload it
   was opened for, and the lock outlives its holder. Reproduced deliberately
   after eleven clean runs. Authored as [T-0211](image.md), not fixed, because
   authoring is its own pass.
6. ⚠ **`main` is two milestones stale.** See [PROGRESS.md](PROGRESS.md) open
   question 6: it is the first time the branch contradiction has had a
   measurable cost, and one sentence from the operator ends it.
