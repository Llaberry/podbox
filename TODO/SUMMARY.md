# Session summary, 2026-09-08

⭐ Saved beside the record so it survives the chat scrolling away.
[PROGRESS.md](PROGRESS.md) is the record and carries the work order. This file
carries only what one session moved.

**Task:** read the six issues filed against this repository, validate rather
than accept, adopt what survives, replace `docs/README.md` with a standalone <!-- known-absent -->
`docs/AGENTS.md`, and review three times.

| row | before | after | from |
| --- | --- | --- | --- |
| Commits | `e358544` | `831f991`, 8 commits | `git log e358544..HEAD` |
| Work | 8 assigned items | **8 completed, 0 deferred, 0 failed** | the task list |
| Changes | | 39 files, +2936 / -170 | `git diff --shortstat`, excluding the artefacts untracked in `831f991` |
| Size, this project's own files | 8,306 lines | 9,325 lines | `git ls-files` less `references/` and `docs/` |
| TODO entries | 81 | 86 | `check-todo.py` |
| Entry statuses | 76 open, 3 blocked, 2 done | 79 open, 2 blocked, 5 done | `check-todo.py` |
| Gate checks | 10 | 16 | `scripts/check-todo.py` |
| Gate checks with a plant | ⛔ **0 committed** | 15 of 16, and the 16th is named | `scripts/plant.sh` |
| Experiment scripts | 8 | 12 | `ls experiments/*.sh` |
| Committed results | 2 | 17 | `git ls-files experiments/results/` |
| Checks | gate 0, no plant harness | gate 0, plant 0, markers 0, fmt 0, clippy 0, musl build 0, `PT_INTERP` 0 | each read unpiped |
| Cost | | 13 image pulls, about 1.5 GB. No money | `docker pull`, all digest-pinned |
| Health | clean | clean, 0 uncommitted, pushed to `main` | `git status` |
| Debt cleared | the corpus was read and unproven | ⭐ question B answered; T-0702 unblocked | `experiments/results/interposer-abi.txt` |
| Debt introduced | | ⚠ git objects 27 MB to 88 MB, recorded in PROGRESS.md | `du -sh .git` on a fresh clone |

## What did not move

- **No podbox implementation code exists.** That was true at the start and is
  true now. This session produced measurements, entries and checks.
- **`experiments/60-interposer-libc.sh` still exits 2.** Its remaining arm needs
  a musl cross toolchain carrying its own `libgcc_s`. ⚠ It no longer blocks
  anything: `80-interposer-abi.sh` answered the design question another way.
- **`experiments/30-attribution-census.sh` still exits 2.** This kernel has no
  Landlock. Not attempted this session.
- **The `compat` `nsswitch` row is untested** for the passwd question, because
  the probe crashed on that distribution before answering.

## Verified on a fresh clone

`git clone`, then: the gate, the plant harness, the marker check, the musl build
and `readelf`, the interposer build for both libcs, and five experiments. All at
their expected exit codes, `60-` at 2 for the reason it names.

## What was adopted from the six issues, and what was not

⭐ Nothing was adopted on the filer's word. The verdict table is in
[PROGRESS.md](PROGRESS.md). Two claims did not reproduce and are recorded as
not reproduced, one of which led to a requirement the specification does not
carry ([T-0410](complete.md)).
