# Session summary, 2026-09-08 (M0)

⭐ Saved beside the record so it survives the chat scrolling away.
[PROGRESS.md](PROGRESS.md) is the record and carries the work order. This file
carries only what one session moved.

**Task:** M0, the probe. [milestones.md](milestones.md) T-1101 and, under it,
[probe.md](probe.md) T-0101 through T-0110. Then [deps.md](deps.md) T-0910, the
`cargo bloat` baseline wired into the gate. Review three times.

| row | before | after | from |
| --- | --- | --- | --- |
| Commits | `beecb8b` | `62cbfd8`, 4 commits | `git log beecb8b..HEAD` |
| Work | 12 assigned items | **10 completed, 2 partial, 0 failed** | T-0101 to T-0110, T-1101, T-0910 |
| Changes | | 32 files, +5145 / -209 | `git diff --shortstat beecb8b..HEAD` |
| podbox implementation code | ⛔ **none existed** | 3,952 lines of Rust across 11 files | `wc -l crates/podbox-probe/src/*.rs crates/podbox-cli/src/main.rs` |
| Dependencies | 0 | **0**, and that is the T-0901 measurement | `cargo tree`, `[workspace.dependencies]` |
| Release binary | 389,656 bytes (empty skeleton) | 496,184 bytes | `experiments/110-bloat-delta.sh baseline` |
| TODO entries | 86 | 86 | `check-todo.py` |
| Entry statuses | 79 open, 0 partial, 2 blocked, 5 done | 67 open, 2 partial, 2 blocked, 15 done | `check-todo.py` |
| Gate checks | 16 | 17 | `scripts/check-todo.py` |
| Plant cases | 15 caught, 0 missed | 18 caught, 0 missed, 3 controls quiet | `scripts/plant.sh` |
| Rust tests | ⛔ **none existed** | 41 passing | `cargo test --workspace` |
| Experiment scripts | 12 | 14 | `ls experiments/*.sh` |
| Committed results | 17 | 22 | `git ls-files experiments/results/` |
| Checks | | gate 0, plant 0, markers 0, fmt 0, clippy 0, tests 0, musl build 0, `PT_INTERP` 0, `110-` 0, `130-` 0, `30-` 2 | each read unpiped |
| Health | clean | clean, 0 uncommitted, pushed to `main` | `git status` |
| Debt cleared | ⛔ the reconstruction had never run here | ⭐ `20-enter-target.sh` fixed; `attribute.txt` taken | `experiments/results/attribute.txt` |
| Debt introduced | | none | |

## The acceptance, in one line each

| clause of T-1101 | result |
| --- | --- |
| `podbox probe` inside `20-enter-target.sh` selects `chroot` | ✅ |
| the same binary unconfined selects `namespace` | ✅ |
| every per-probe verdict matches `attribute.txt` row for row | ✅ 15 matched, 1 recorded divergence, 0 differed, 0 missing |

⭐ The one divergence is the reference recording a control's **absence** as a
denial. `kcmp(2)` needs `CONFIG_CHECKPOINT_RESTORE`; `ENOSYS` is the kernel
saying the control is not there, which is not the control answering. That is
[T-0109](probe.md)'s rule, and podbox reports it as a third state.

## Three defects found in this tree, each blocking the acceptance

⭐ All three were found by trying to run the acceptance rather than by reading
about it, which is `docs/methodology/gate.md` part (b) earning its place.

1. **`experiments/20-enter-target.sh` had never run here.** It named
   `$REPO/verification/{confine,probe,cprobe}`, and podbox tracks that
   repository under `references/`. Every invocation died at `cd`.
2. **`experiments/results/attribute.txt` did not exist.** The milestone's
   acceptance names it and nothing had produced it.
3. **[T-0102](probe.md)'s `Prove` asserted one host's answer as every host's.**
   The rule underneath survives and is what podbox implements.

## Four defects found in the reference instrument

Each is recorded in `crates/podbox-probe/src/probes.rs`'s module header with the
reason it was not ported as-is: the propagation change that escapes the prober,
two mounts left attached, scratch files overwritten, and a failed precondition
reported as the row's denial.

## What the three review passes found

- **Pass 1 (true?)** 46 syscall numbers and 19 flag constants checked against
  the kernel headers, every reference citation opened at its line, four guards
  mutation-proved. Two ABI cross-checks added as tests.
- **Pass 2 (consistent?)** Five statements had outlived their numbers:
  `docs/AGENTS.md`, `scripts/plant.sh`, [T-1202](gate.md)'s note, the root
  `README.md`, and `experiments/README.md`.
- **Pass 3 (usable cold?)** ⭐ The one real defect of the three: with fd 1
  closed at process start, `pipe2(2)` puts a pipe end on fd 1 and the child's
  `dup2(1, 1)` plus an unconditional `close` shut the channel every probe
  answers through. **Every verdict would have become a `skip`: honest, and
  useless.** Found by driving the CLI, not by a test.

## What did not move

- **`experiments/60-interposer-libc.sh` still exits 2.** It needs a musl cross
  toolchain carrying its own `libgcc_s`. ⚠ It blocks nothing.
- **`experiments/30-attribution-census.sh` still exits 2.** This kernel has no
  Landlock, so the three M rows cannot run.
- ⚠ **The `chroot` rung was selected without an LSM ever being present.** On the
  target, `move_mount` is `EPERM` from Landlock; here it attaches. The rung came
  out right for the reasons it should have, but the M half of the ladder is
  untested locally. A distro kernel closes it.
- ⛔ **`TOOL.md` section 6.1's `$store/probe.json` cache has no entry and was
  not written.** There is no store until M1. Authoring is a separate pass from
  implementing, so the next session should author it before M1's store lands.
