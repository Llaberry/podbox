# Session summary, 2026-09-08 (M0, the sweep, and the debt)

⭐ Saved beside the record so it survives the chat scrolling away.
[PROGRESS.md](PROGRESS.md) is the record and carries the work order. This file
carries only what one session moved.

**Task:** M0, the probe: [milestones.md](milestones.md) T-1101 and, under it,
[probe.md](probe.md) T-0101 to T-0110. Then [deps.md](deps.md) T-0910, the
`cargo bloat` baseline wired into the gate. Then, on the operator's rulings
mid-session: the rest of the dependency sweep, and the git-object debt.

| row | before | after | from |
| --- | --- | --- | --- |
| Commits | `e8ed921` | `a05bae4`, 9 commits | `git log e8ed921..HEAD` |
| Changes | | 59 files, +7301 / -315 | `git diff --shortstat e8ed921..HEAD` |
| podbox implementation code | ⛔ **none existed** | 4,116 lines of Rust across 11 files | `wc -l crates/podbox-probe/src/*.rs crates/podbox-cli/src/main.rs` |
| Release binary | 389,656 bytes (empty skeleton) | 496,184 bytes, **0 third-party crates** | `experiments/110-bloat-delta.sh baseline` |
| TODO entries | 86 | 86 | `check-todo.py` |
| Entry statuses | 79 open, 0 partial, 2 blocked, 5 done | **58 open, 3 partial, 2 blocked, 23 done** | `check-todo.py` |
| Gate checks | 16 | 17 | `scripts/check-todo.py` |
| Plant cases | 15 caught, 0 missed | 18 caught, 0 missed, 3 controls quiet | `scripts/plant.sh` |
| Rust tests | ⛔ **none existed** | 44 passing | `cargo test --workspace` |
| Experiment scripts | 12 | 14 | `ls experiments/*.sh` |
| Committed results | 17 | 36 | `git ls-files experiments/results/` |
| Dependency sweep | 0 of 10 entries closed | **10 of 10** | [deps.md](deps.md) |
| ⭐ git objects, fresh clone | 51 MB, 9,445 objects | **29 MB, 7,931 objects** | `git count-objects -vH` after `git clone` |
| Checks | | gate 0, plant 0, markers 0, fmt 0, clippy 0, tests 0, musl build 0, `PT_INTERP` 0, `110-` 0, `130-` 0, `60-` **0 (was 2)**, `30-` 2, bootstrap 0 | each read unpiped |
| Health | clean | clean, 0 uncommitted, pushed to `main` | `git status` |
| Debt cleared | ⛔ the reconstruction had never run here; the artefacts were in history | ⭐ both | `experiments/results/`, `git count-objects` |
| Debt introduced | | none | |

## Four operator rulings, recorded so an unattended session does not re-ask

[RULES.md](RULES.md) section 11 carries all four.

| question | ruling |
| --- | --- |
| The commit trailer, where `docs/conventions/git.md` section 1 and the harness disagree | git.md wins: no trailer names a model, a vendor or a tool. This session's 9 commits carry none |
| The git-object debt | rewrite history and force push, overriding git.md section 5. Done and verified |
| [T-0803](cli.md), the `docker` name where a daemon is reachable | refuse without an explicit flag |
| How far the dependency sweep may go | measure, and land what the measurement favours |

## M0

`podbox probe` selects `chroot` inside `experiments/20-enter-target.sh` and
`namespace` unconfined, and 15 of 16 attribution rows match
`experiments/results/attribute.txt` exactly.
`experiments/130-probe-parity.sh` is that acceptance as one command.

⭐ **The one divergence is the reference recording a control's absence as a
denial.** `kcmp(2)` needs `CONFIG_CHECKPOINT_RESTORE`; `ENOSYS` is the kernel
saying the control is not there, which is not the control answering.

## Six defects found by running things rather than reading them

1. `experiments/20-enter-target.sh` named `$REPO/verification/`, which this tree
   does not have. **The reconstruction had never run here.**
2. `experiments/results/attribute.txt`, which the milestone compares against,
   **did not exist**.
3. [T-0102](probe.md)'s `Prove` asserted one host's answer as every host's.
4. ⭐ **The probe's verdict channel was lost when a caller had fd 1 closed.**
   `pipe2(2)` puts a pipe end on fd 1 and `dup2(1, 1)` returns without closing,
   so the unconditional `close` after it shut the channel every child answers
   through. Every verdict would have become a `skip`: honest, and useless.
   Found by driving the CLI, not by a test.
5. ⭐ **A dependency nothing calls measures as zero**, because `lto` deletes it.
   A sweep reporting "this crate is free" has measured nothing.
6. `Cargo.toml` repeated the sweep's byte counts and one was stale against its
   own entry within the hour.

## Four defects found in the reference instrument

Recorded in `crates/podbox-probe/src/probes.rs`'s module header with the reason
each was not ported as-is: a propagation change that escapes the prober, two
mounts left attached, scratch files overwritten, and a failed precondition
reported as the row's denial.

## The dependency sweep

Landed as pins: `rustls` with the host bundle and `webpki-roots` as the
fallback, `ureq`, `tar` + `flate2` (`rust_backend`) + `ruzstd`, `sha2`,
`serde_json`. Refused with a number each: `clap`, `goblin`, `oci-spec`,
`seccompiler`, the `landlock` crate, `rustix`, `libc`.

⛔ **`rustls` is pure Rust and its crypto provider is not.** `ring` ships 17 `.c`
files and 90 `.S` files behind a `build.rs`. It does not break `crt-static`, and
it costs a C cross-compiler.

## New infrastructure

- `scripts/common/bootstrap-env.sh`: one idempotent, non-interactive script that
  brings a fresh container up to what the gate, the experiments and the builds
  need, **including starting the docker daemon**. Its checksum guard, its
  install path and its daemon restart were each driven rather than assumed.
- `scripts/zig-cc.sh` and `scripts/zig-ar.sh`, wired into `.cargo/config.toml`.
  ⭐ They also closed an open question three sessions old:
  `experiments/60-interposer-libc.sh` **exits 0** for the first time, and its
  check B now reproduces the cross-libc refusal with podbox's own Rust cdylib
  rather than only with the C reference interposer.

## What did not move

- **`experiments/30-attribution-census.sh` still exits 2.** This kernel has no
  Landlock, so the three M rows cannot run.
- ⚠ **The `chroot` rung was selected without an LSM ever being present.** On the
  target `move_mount` is `EPERM` from Landlock; here it attaches. The rung came
  out right for the reasons it should have, and the M half is untested locally.
- ⚠ **`controls_answered` is false on every run of this host**, because
  `kcmp(2)` is not built into this kernel. Correct, and not clearable here.
- ⛔ **`TOOL.md` section 6.1's `$store/probe.json` cache has no entry and was
  not written.** There is no store until M1, and authoring is a separate pass
  from implementing.
