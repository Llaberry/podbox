# AGENTS.md

podbox is a container runtime for Linux environments that hand a process uid 0
and then refuse almost every operation containers are built on: AI-agent
sandboxes, hardened CI executors, locked-down HPC nodes. A process there holds
every capability bit and `Seccomp: 2`, and cannot `unshare`, `mount`,
`pivot_root`, `ptrace`, `mknod`, `setuid` to a non-zero id, or `chown` to an
unmapped one. podbox answers to `docker` and `podman` on PATH, takes the same
verbs, flags and exit codes, and where it cannot honour something it says so in
one line. Its audience is automated, which is why the honesty rules are the
product rather than a disclaimer: an agent cannot notice that its container was
a chroot the way a person might.

**This file is the router, and it is the only file you have to be pointed at.**
It restates nothing written elsewhere, so the two cannot fork. Everything
binding is linked, and the link is the authority. Reading a row in a table here
is not reading the rule.

---

## Start here, every session

⭐ **One command first, and it returns immediately:**

```sh
./scripts/dev.sh          # environment and build, in the BACKGROUND
```

⛔ **Do not wait for it.** It brings the machine up and compiles the binary
behind the reading below, which needs no toolchain. Measured on 2026-09-09 in
`experiments/results/session-startup.txt`: a cold compile is **29 s and 87
crates**, and the reading is **6,524 words**. ⚠ That second number moves every
session, because `PROGRESS.md` is rewritten every session;
`experiments/310-session-startup.sh` clause 4 is what recomputes it. A session that reads first and
builds second pays both; a session that runs this first pays only the reading.

⭐ **Then read [`../TODO/PROGRESS.md`](../TODO/PROGRESS.md).** It is the only
file that carries what changed since last time and what to do next, in the
shape [`methodology/sessions.md`](methodology/sessions.md) names. Nothing else
carries a work order.

Then run the gate, so that anything it finds later is yours:

```sh
./scripts/check-todo.py
./scripts/dev.sh status   # ready, stale, or failed with the log
```

⚠ `./scripts/dev.sh build` after a source change, and `./scripts/dev.sh check`
before a commit: fmt, clippy, build, tests, the gate and the marker check, each
read from the process that produced it.

Then read what **this task** routes you to, below. Not everything, and not less.

⛔ **Do not re-read the corpus.** `references/` was read under
[`methodology/references.md`](methodology/references.md): three passes plus the
issue and pull-request tracker, per tree. Every entry in `TODO/` carries what to
do and which reference to open at which line. Re-reading it is the most
expensive way to learn what an entry already tells you.

---

## The routing table

⭐ **This table is the reason this file exists.** Find the row for the work in
front of you and read what it names, in full.

| the task | read, in this order |
| --- | --- |
| **Any session, before anything else** | [`../TODO/PROGRESS.md`](../TODO/PROGRESS.md) , [`methodology/sessions.md`](methodology/sessions.md) |
| **What podbox has to be, and what it may not claim** | `references/Azathothas__container-research/tree/TOOL.md` sections 3, 4 and 6 . ⚠ The runtime `paper_final.md` describes is a FLOOR, not a specification: probe everything, hard-code nothing |
| **Implementing an entry** | the entry in `TODO/<category>.md` , [`methodology/gate.md`](methodology/gate.md) , [`conventions/code.md`](conventions/code.md) , [`conventions/forbidden-patterns.md`](conventions/forbidden-patterns.md) |
| **Authoring new work** | [`methodology/authoring.md`](methodology/authoring.md) , [`../TODO/RULES.md`](../TODO/RULES.md) , ⛔ do not implement in the same pass |
| **Fixing a defect** | [`methodology/authoring.md`](methodology/authoring.md) , the code the defect is in , [`conventions/forbidden-patterns.md`](conventions/forbidden-patterns.md) |
| **Taking a measurement** | [`methodology/experiments.md`](methodology/experiments.md) , an existing script in `experiments/` as the shape |
| ⭐ **Adding or changing a check in the gate** | [`methodology/gate.md`](methodology/gate.md) , then `scripts/plant.sh`. ⛔ A check and its plant land in the same change |
| **Studying an external repository** | [`methodology/references.md`](methodology/references.md) , [`../TODO/reference-map.md`](../TODO/reference-map.md) |
| ⭐ **Touching any vendored or third-party source** | [`methodology/vendoring.md`](methodology/vendoring.md) , [`../THIRD_PARTY.md`](../THIRD_PARTY.md) . ⛔ Patch it here, and upstreaming is not a topic |
| **Writing or editing a document** | [`conventions/prose.md`](conventions/prose.md) , [`conventions/docs.md`](conventions/docs.md) |
| **Committing** | [`conventions/git.md`](conventions/git.md) |
| **Anything crossing a shell, or a quoting problem** | [`conventions/shell.md`](conventions/shell.md) |
| **Touching anything remote** | [`security/remote-ops.md`](security/remote-ops.md) |
| **Anything involving a credential** | [`security/secrets.md`](security/secrets.md) |
| **Recording something superseded** | [`methodology/history.md`](methodology/history.md) . ⛔ Not into the page it supersedes |
| **Choosing or changing the work model** | [`methodology/choosing-a-work-model.md`](methodology/choosing-a-work-model.md) , [`methodology/work-todo.md`](methodology/work-todo.md) , [`methodology/work-stages.md`](methodology/work-stages.md) |
| **Closing out a session** | [`methodology/sessions.md`](methodology/sessions.md) , [`methodology/reviews.md`](methodology/reviews.md) |

⛔ **Read what the row names in full.** Not grepped, not skimmed, not recalled
from a previous session. The routing exists so the reading is small enough to
actually do.

⚠ **When two rows apply, read both.** The union, not the shorter one.

---

## The absolutes

Short enough to state here, and each has been paid for:

1. ⛔ **Work on `main`.** Never a `claude/*` or otherwise agent-named branch.
2. ⛔ **Every other repository is read-only.** Never open an issue, a pull
   request, a discussion, a comment, a review or a fork anywhere else, under
   any framing. Never write a characterisation of an upstream project or its
   maintainers. A defect in vendored code is fixed here, in this tree, now.
3. ⛔ **No fabricated numbers.** A dash where a value is unknown. An estimate is
   labelled as one, in the same sentence, every time.
4. ⛔ **Every measurement ships with the script that took it**, in
   `experiments/`, with pinned inputs, printed conditions and a meaningful exit
   code: 0 it ran and matched, 1 it ran and did not, 2 it could not run. **A
   negative result is a result and gets committed.**
5. ⛔ **Nothing closes as "won't fix", "upstream's problem" or "out of scope".**
   A blocked entry stays open, names its blocker, and names what would clear it.
6. ⛔ **Write in place.** Amend a document; never append a corrections section.
   A disproved premise keeps its title and takes the correction underneath it.
7. ⛔ **`TODO/` is updated in the same change as the work**, never after it.
8. ⛔ **An exit code is read from the process that produced it, unpiped.**
   Piping a check into anything reports the pipeline's status, so a guard that
   failed reads as green.
9. ⛔ **A secret never enters the tree, a log or a commit message.** Not
   expired, not redacted-looking, not in an example.

---

## Verify, do not accept

⭐ **This is the rule that produced most of what is in `TODO/`.** A claim from
an earlier session, an issue, a specification or the operator describes a tree
that may have moved. Open the file at the captured commit and look.

⛔ **A disagreement between the claim and the code is the finding**, not an
obstacle to the task. `TODO/PROGRESS.md` carries the ones found so far; each
names the file and line that settles it.

⚠ The same rule applies to a measurement somebody else took. A count taken on
another host is a reading from that host: reproduce the **mechanism**, and
expect the number to differ.

---

## The tree

| directory | what is in it |
| --- | --- |
| [`../TODO/`](../TODO/) | the work. `INDEX.md` is every entry and the counts, `PROGRESS.md` the order, `RULES.md` how an entry closes, `reference-map.md` the corpus and its licence determinations |
| [`../crates/`](../crates/) | the workspace of `TOOL.md` section 4.3 |
| [`../references/`](../references/) | the corpus: pinned trees with their trackers. Read once, under `methodology/references.md` |
| [`../experiments/`](../experiments/) | the reconstruction of the target runtime, and this project's own measurements. `results/` is the evidence and is tracked |
| [`../scripts/`](../scripts/) | `dev.sh` first: the background build a session opens with. Then the gate, the count writer, the plant harness, the corpus fetcher, the environment bootstrap, and `zig-cc.sh`, which is the C cross-compiler `.cargo/config.toml` names |
| [`.`](.) | ⛔ the methodology. **Binding, not advisory.** Copied verbatim from [`Azathothas/TEMPLATE`](https://github.com/Azathothas/TEMPLATE), except this file |

---

## What this environment is, and what it does to you

⭐ **Run this first, every session.** The container is new each time and carries
none of what the last one installed:

```sh
./scripts/common/bootstrap-env.sh          # install what is missing
./scripts/common/bootstrap-env.sh --check  # report only, change nothing
```

It is idempotent, never prompts, times everything, checks blocks **and** inodes
before writing, verifies what it downloads against a pinned checksum, and
**starts the docker daemon**. The table below is what it exists to handle.

⚠ **These are not general truths.** They are what this machine does, and each
one has cost a session at least once.

| | |
| --- | --- |
| **dockerd** | installed and **not running between turns**. `nohup dockerd >/tmp/dockerd.log 2>&1 &`, then wait for `docker info`. |
| **GitHub reads** | `https://api.gh.pkgforge.dev/<GH_API_PATH>` for read-only API paths. ⚠ Send curl's own user agent: a browser-like or empty one gets HTTP 420. |
| **other fetches** | the direct URL first, then `https://api.rv.pkgforge.dev/<ORIGINAL_URL>`. `raw.githubusercontent.com` usually works directly; `github.com/.../raw/...` usually does not. |
| **TLS** | outbound HTTPS is intercepted. ⛔ Never disable verification and never unset the proxy. A `curl` inside a docker build cannot verify a chain the host trusts, so copy from a pinned image stage instead of fetching in a `RUN`. |
| **disk** | writable space is a fixed allowance, so `df` misleads: `Avail 0` with low `Used` means the allowance is spent, not that the machine is broken. Check blocks **and inodes** before a clone, an image pull or a build. Deletes still succeed while writes fail. |
| **hangs** | every command touching the network, a registry, a container or another process gets a `timeout`. Never wait with no upper bound. |
| **prompts** | never run an interactive command. `-y`, `--noconfirm`, `--non-interactive`, `DEBIAN_FRONTEND=noninteractive`, `GIT_TERMINAL_PROMPT=0`, `</dev/null`. A prompt with no terminal behind it is a hang, and a hang costs the session. |

⛔ **podbox inherits the last three as requirements, not just you.** A runtime
whose audience is automated may not block on input, may not wait unbounded, and
may not assume a writable path has room.

Two shell traps that have each cost time here, both in
[`conventions/shell.md`](conventions/shell.md) and both worth naming twice:
`grep -c` exits 1 on zero matches and breaks an `&&` chain; and under
`set -o pipefail`, `cmd | head -1` returns `cmd`'s SIGPIPE status, so a
trailing `|| echo absent` fires beside the real value.

---

## The gate, and why it is trusted

```sh
./scripts/check-todo.py    # the reader. Twenty checks. Must exit 0 at every commit
./scripts/todo-count.py    # the writer. Re-derives the counts; --set moves row and entry together
./scripts/plant.sh         # breaks each check on purpose and asserts it goes red
./scripts/common/check-markers.sh
./scripts/common/bootstrap-env.sh   # the environment the four above assume
```

⭐ **`plant.sh` is what makes the gate an assertion rather than a decoration.**
A check that quietly matches nothing exits 0 exactly like a check whose
assertions all passed. It plants each defect, asserts the gate goes red **with
that defect's own message**, and asserts the message was not already there.
Its first run found a case whose mutation landed and never reached its subject.

⛔ **A new check arrives with its plant, in the same change.** A check nobody
has seen fail is not a check.

---

## Known gaps, stated rather than hidden

⚠ Nine markdown links inside the verbatim methodology copy point at template
infrastructure podbox did not adopt. They are recorded here and their count is
held by check 13 of the gate, so a tenth cannot arrive unnoticed. Following one
is a dead end, not a missing file you should go and write:

| in | points at |
| --- | --- |
| `docs/conventions/docs.md:142` | `scripts/common/check-changelog.sh` | <!-- known-absent -->
| `docs/conventions/prose.md:243` | `scripts/README.md` | <!-- known-absent -->
| `docs/conventions/shell.md:182` | `dotfiles/common/gitattributes` | <!-- known-absent -->
| `docs/conventions/shell.md:320` | `scripts/common/check-control-bytes.sh` | <!-- known-absent -->
| `docs/conventions/shell.md:327` | a section number, not a path | <!-- known-absent -->
| `docs/methodology/choosing-a-work-model.md:63` | `docs/templates/` | <!-- known-absent -->
| `docs/methodology/sessions.md:18` | `scripts/doctor/` | <!-- known-absent -->
| `docs/security/remote-ops.md:118` | `scripts/common/check-remote-items.sh` | <!-- known-absent -->
| `docs/security/secrets.md:114` | `docs/public/README.md` | <!-- known-absent -->

⚠ `scripts/common/check-markers.sh` is **modified** from its upstream copy, to
exclude `references/` and `docs/`. [`../THIRD_PARTY.md`](../THIRD_PARTY.md)
carries the patch record and the command that reproduces the defect it fixes
against the pristine copy in the corpus.

---

## What a session owes at its end

Specified in [`methodology/sessions.md`](methodology/sessions.md). The short
form, and none of it is conditional on the session having gone well:

- [`../TODO/PROGRESS.md`](../TODO/PROGRESS.md) rewritten: the state line, the
  measured baseline, the counts, what this session did, what is in progress,
  the work order, the open questions. ⛔ It carries no history; a superseded
  explanation goes where [`methodology/history.md`](methodology/history.md)
  says;
- the gate run and green, and `scripts/plant.sh` run if any check changed;
- `TODO/` updated in the same change as the work, and everything committed and
  pushed to `main`;
- ⭐ the **summary table**, printed in chat and saved;
- ⭐ the **next prompt**, printed in chat only, and it is a **resume** prompt if
  anything at all was left unfinished.

---

## When you are unsure

In order: what the operator said in this session, what the linked rule says,
what a probe or the code measured, then ask the operator.

⛔ Never invent a fifth option silently, and never settle a contradiction
between two of these by taking the convenient one. A contradiction is a
finding, and a finding is reported.
