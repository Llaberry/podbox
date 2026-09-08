# RULES

How this repository is worked on. `docs/` is the methodology and it binds;
this file is what is specific to podbox.

[INDEX.md](INDEX.md) is the list. [PROGRESS.md](PROGRESS.md) is the work order
and the only place that carries one. [reference-map.md](reference-map.md) is
the corpus and its licence determinations.

## 1. Starting a session

1. Read [PROGRESS.md](PROGRESS.md). Its state line, its counts and its "start
   here next" are the whole handover.
2. Run the gate before touching anything, so a failure found later is yours:

   ```sh
   ./scripts/check-todo.py
   ```

3. Read the entry you are about to work, and the reference lines it cites.
   ⭐ **Do not re-read the corpus.** The entries carry what to do and which
   reference to open at which line. That is what M-1 was paid for.

## 2. The branch

⛔ **`main`, always.** Never a `claude/*` or other agent-named branch unless a
human says otherwise in that session. Commit and push to `main`.

```sh
git push -u origin main
```

On a network failure, retry with backoff: 2 s, 4 s, 8 s, 16 s, then stop and
say so.

## 3. Ending a session

Per `docs/methodology/sessions.md`:

1. [PROGRESS.md](PROGRESS.md) **rewritten**, carrying the state line, the
   measured baseline, the counts, what this session did, what is in progress,
   the work order, and the open questions. It carries no history.
2. The summary table, in chat and saved.
3. The next session's prompt, in chat only.

⛔ **The record is part of the change.** The entry, the index and
`PROGRESS.md` are edited in the same commit as the work, never after it.

## 4. The counts are never touched by hand

⛔ Closing one entry moves the totals line, one priority row, that row's total
and the All row.

```sh
./scripts/todo-count.py --set T-0101 done   # moves the row AND the entry
./scripts/todo-count.py                     # re-derives every count
./scripts/check-todo.py                     # asserts it independently
```

`scripts/check-todo.py` is the gate. It also asserts that every cited
`path:line` in `TODO/` resolves, so a citation into a reference that moved
fails the build rather than misleading the next session.

## 5. How an entry closes

An entry closes **in place**, in its own file, with its `Prove` command
actually run and the output recorded underneath.

⛔ **Nothing closes as "won't fix", "upstream's problem" or "out of scope".**
A blocked entry stays `blocked`, keeps its title, and names the blocker and
what would clear it.

⛔ **A disproved premise keeps its title.** The correction goes underneath.
Never a silent edit of the `Premise` line: the title is how the entry has
always been referred to.

## 6. Measurement

Per `docs/methodology/experiments.md`, and podbox has one addition of its own.

- Every number ships with the script that took it, in `experiments/`, numbered,
  with pinned inputs and the conditions printed on the way out.
- Exit codes are uniform: **0** the measurement ran and matched, **1** it ran
  and something under test failed, **2** it could not run.
- ⛔ **A negative result is a result and gets committed.**
  `experiments/results/interposer-libc.txt` exits 2 on this host and is
  committed for exactly that reason.
- ⛔ **"Could not run" must never read as "denied".** Both mistakes were live
  in the research harness this project inherits, and podbox's own probe has the
  same failure available to it.

## 7. Vendoring

`docs/methodology/vendoring.md` binds, and its rule is not open.

⛔ **Fix it here, now, in this tree.** Never open an issue, pull request,
discussion, comment, review or fork on anybody else's repository, under any
framing. Never write a characterisation of an upstream project or its
maintainers: write the technical fact and stop.

Every patch to vendored code carries the command that reproduces the defect it
fixes, so a future release can be checked against it rather than judged.

⛔ **The licence determination is made before the tree is used.**
[reference-map.md](reference-map.md) carries all thirty, including the three
that are not resolved and are therefore not vendored.

## 8. Resource and liveness guards

⚠ Long autonomous sessions die in three ways, and podbox inherits all three as
**product requirements**, not just session hygiene.

| | the session | podbox |
| --- | --- | --- |
| **disk** | check blocks **and** inodes before a clone, a pull or a build. On failure delete build artefacts and caches: deletes still succeed while writes fail | `statvfs` before download and before extraction, and the error names the destination. T-0203 |
| **hangs** | every command touching the network, a registry, a container or another process gets a `timeout` | bounded waits on a pidfd or a readiness fd, never a sleep. T-0602 |
| **prompts** | never run an interactive command: `-y`, `--noconfirm`, `--yes`, `--non-interactive`, `DEBIAN_FRONTEND=noninteractive`, `GIT_TERMINAL_PROMPT=0`, `</dev/null` | the CLI never prompts. A container runtime that blocks on stdin is unusable by the audience it is for. T-0806 |

⚠ Writable space is a fixed allowance, so `df` misleads: "Avail 0" beside a low
"Used" means the allowance is spent, not that the machine is broken.

## 9. Prose

`docs/conventions/prose.md` binds. The three that are broken most often here:

- ⛔ **No narrative.** Not in a document, not in a commit message, not in an
  entry. No "as we discovered", no session diary, no defensive framing.
- ⛔ **No fabricated number.** A dash where the value is unknown. An estimate is
  labelled as one, in the same sentence, every time it appears.
- ⭐ **Write in place.** Amend the document. Never append a corrections section
  or a dated box under the old text.

## 10. Three deep review passes

⭐ Before anything is called done, three passes, **each asking a different
question**. One pass repeated three times is one pass.

1. Is each claim **true**, checked against the source at the captured commit or
   against a run?
2. Is it **internally consistent**: do the cross-references resolve, do two
   sections disagree?
3. Is it **usable cold** by somebody with no memory of this work?

⭐ **Verify, do not accept.** A claim from a previous session, from an issue, or
from the operator describes a tree that may have moved. Open the file at the
captured commit. ⭐ **A disagreement between the claim and the code is the
finding**, and it is worth more than either source. Four of them are already
recorded here: T-0702's premise, [reference-map.md](reference-map.md)'s
`userland-execve` row, T-0302's `ignore_chown_errors` note, and T-0703's
entry-point count.

## 11. Settled decisions, not to be relitigated

| decision | where it was made |
| --- | --- |
| Rust, `x86_64-unknown-linux-musl`, `crt-static` | `TOOL.md` section 3, measured by `experiments/40-language-selection.sh` |
| The lilipod patch is not a seed | `TOOL.md` section 3.3, and its GPL-3.0 settles it independently |
| Nothing is opened on anybody else's repository | `docs/methodology/vendoring.md`, and the incident behind it |
| 0BSD for this repository | `TOOL.md` section 0.5 |
| The corpus is tracked in the tree, not on a side branch | [reference-map.md](reference-map.md), because the gate resolves citations |
| The default answer to a dependency is no | `TOOL.md` section 3.5, and every candidate is an entry in [deps.md](deps.md) |
