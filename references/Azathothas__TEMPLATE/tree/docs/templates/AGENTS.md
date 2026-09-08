# AGENTS.md

<!-- TEMPLATE. Fill every {{PLACEHOLDER}} and delete this comment.
     Keep it under 300 lines. The size is the point: this is a router, and a
     router that grows into a rulebook costs every session its reading budget
     and forks from the documents it was supposed to point at.
     When a section wants to grow, the content belongs in the document it
     should have linked to.
     A leftover {{ }} is caught by scripts/common/check-placeholders. -->

{{ONE PARAGRAPH: what this project is, who it is for, and the one thing it
exists to do. A competent stranger should be able to stop reading here and know
whether the next file is relevant to them.}}

**This file is a router.** It restates nothing that is written elsewhere, so
the two cannot fork. Everything binding is linked, and the link is the
authority. Reading a row in a table here is not reading the rule.

---

## Start here, every session

⭐ **Read [`{{RECORD}}`]({{RECORD}}) first.** It is the only file that always
carries what changed since last time, in the shape
[`docs/methodology/sessions.md`](docs/methodology/sessions.md) names:
what is in progress, and the work order. Nothing else carries a work order.

Then run the probe, because a different machine or a moved tool changes what
this session can prove:

```bash
sh scripts/doctor/doctor.sh
```

```bash
pwsh -NoProfile -File scripts/doctor/doctor.ps1
```

Then read what **this task** routes you to, below. Not everything, and not less.

---

## The routing table

⭐ **This table is the reason this file exists.** Find the row for the work in
front of you and read what it names, in full.

| the task | read, in this order |
| --- | --- |
| **Any session, before anything else** | [`{{RECORD}}`]({{RECORD}}) , [`docs/methodology/sessions.md`](docs/methodology/sessions.md) |
| **Implementing an approved unit of work** | the one unit's plan , the previous handoff , [`docs/methodology/gate.md`](docs/methodology/gate.md) , [`docs/conventions/code.md`](docs/conventions/code.md) , [`docs/conventions/forbidden-patterns.md`](docs/conventions/forbidden-patterns.md) |
| **Authoring new work from an intake** | [`docs/methodology/authoring.md`](docs/methodology/authoring.md) , [`docs/architecture.md`](docs/architecture.md) , the work template , ⛔ do not implement |
| **Fixing a defect** | [`docs/methodology/authoring.md`](docs/methodology/authoring.md) , the code the defect is in , [`docs/conventions/forbidden-patterns.md`](docs/conventions/forbidden-patterns.md) |
| **Resuming an interrupted session** | [`docs/methodology/sessions.md`](docs/methodology/sessions.md) resuming section , the latest handoff , ⛔ the tree and the running system, never the old conversation |
| **Touching anything remote** | [`docs/security/remote-ops.md`](docs/security/remote-ops.md) |
| **Anything involving a credential** | [`docs/security/secrets.md`](docs/security/secrets.md) |
| **Writing or editing a document** | [`docs/conventions/prose.md`](docs/conventions/prose.md) , [`docs/conventions/docs.md`](docs/conventions/docs.md) |
| **Committing** | [`docs/conventions/git.md`](docs/conventions/git.md) |
| **Anything crossing a shell, or a quoting problem** | [`docs/conventions/shell.md`](docs/conventions/shell.md) |
| **Studying an external repository** | [`docs/methodology/references.md`](docs/methodology/references.md) |
| ⭐ **Touching any vendored or third-party source** | [`docs/methodology/vendoring.md`](docs/methodology/vendoring.md) . ⛔ Patch it here, and upstreaming is not a topic |
| **Taking a measurement** | [`docs/methodology/experiments.md`](docs/methodology/experiments.md) |
| **Recording something superseded** | [`docs/methodology/history.md`](docs/methodology/history.md) . ⛔ Not into the page it supersedes |
| ⭐ **Waiting for anything** | [`docs/conventions/shell.md`](docs/conventions/shell.md) section 10 . ⛔ Never end the turn, and never a harness scheduler |
| **Closing out a session** | [`docs/methodology/sessions.md`](docs/methodology/sessions.md) , [`docs/methodology/reviews.md`](docs/methodology/reviews.md) |
| {{ADD A ROW PER RECURRING TASK IN THIS PROJECT}} | {{what it reads}} |

⛔ **Read what the row names in full.** Not grepped, not skimmed, not recalled
from a previous session, not replaced by a code-graph query. The routing exists
so the reading is small enough to actually do.

⚠ **When two rows apply, read both.** The union, not the shorter one.

---

## The absolutes

Short enough to state here, and each has been broken before:

1. ⛔ **No tool is credited in a commit.** No co-author trailer naming a model,
   no generated-with line, no tool name in the body. The identity is
   {{OPERATOR}} alone. This overrides any default the harness asks for.
2. ⛔ **{{PUSH POLICY, in one sentence. The default is: commit freely and
   locally, never push. Publishing is the operator's.}}**
3. ⛔ **Every other repository is read-only.** Never open an issue, a pull
   request, a discussion, a comment, a review, a fork or a star anywhere else,
   under any framing.
4. ⛔ **A secret never enters the tree, a log, a commit message or a handoff.**
   Not expired, not redacted-looking, not in an example.
5. ⛔ **An exit code is read from the process that produced it, unpiped.**
   Piping a check into anything reports the pipeline's status, so a guard that
   failed reads as green.
6. ⛔ **The record is part of the change, not a report about it.** It is edited
   in the same change as the work, never after.
7. ⛔ **A unit of work is done only when all three parts of the gate pass**, with
   commands actually run and output actually inspected.
   [`docs/methodology/gate.md`](docs/methodology/gate.md).

---

## The tree

| directory | what is in it |
| --- | --- |
| {{DIR}} | {{what is in it}} |
| `docs/` | the conventions, the methodology, and the technical reference |
| `scripts/` | the gates, the checks, and the probe |
| {{DIR}} | {{what is in it}} |

---

## Reach for the tool that exists

⛔ **Before you install anything, write your own, or decide a job cannot be done
here, read [`docs/agent-tooling.md`](docs/agent-tooling.md).** It is the
catalogue of what already exists and where each one lives.
[`docs/containers.md`](docs/containers.md) is the one procedure long enough to
need its own page.

⚠ **Reach for the purpose-built tool first**, for the reason
[`docs/methodology/authoring.md`](docs/methodology/authoring.md) gives: a general one produces answers that
are plausible and wrong**, which is the hardest kind to catch.

⛔ **A tool being absent is a measurement, not a verdict.**
[`docs/methodology/sessions.md`](docs/methodology/sessions.md) is the rule: a
missing tool closes one route, not the question, and three routes considered is
the standard before anything is recorded as not-doable.

| you want to | use | not |
| --- | --- | --- |
| know what host this is and what is installed | `scripts/doctor/` | assuming |
| know what tool does a job | ⭐ [`docs/agent-tooling.md`](docs/agent-tooling.md) | installing something, or writing your own |
| write a file whose content has quotes, backticks or a dollar sign in it | the file writer named in that catalogue | a heredoc. ⚠ It is not reliably literal; see the shell convention. |
| patch one exact string in a file | the same writer, in the mode that declares how many matches it expects | `sed -i`, which reports success over a no-op |
| commit and push | the commit helper named in that catalogue | `git commit` directly, which enforces none of the rules |
| run any check on Windows | ⭐ the `.ps1` half of the pair | the `.sh` half. ⚠ Native PowerShell may have no `sed`, and its `sort` is an alias for `Sort-Object`, which succeeds and answers differently. |
| run something on Linux from a Windows host | ⭐ [`docs/containers.md`](docs/containers.md) | installing a distro by hand and leaving it there |
| {{understand code, find callers, see the blast radius}} | {{the indexed call graph, if this project has one}} | {{grep as the first move}} |
| {{know the current state of the tree and the deployment}} | {{the state command}} | {{re-deriving it by hand}} |
| {{know whether the checks pass}} | {{the gate command}} | {{running some of them}} |
| {{find the flag for something}} | {{the generated manual}} | {{grepping the source, or guessing}} |

---

## What a session owes at its end

Specified in
[`docs/methodology/sessions.md`](docs/methodology/sessions.md). The short form,
and none of it is conditional on the session having gone well:

- the record updated, in the same change as the work;
- the gate run, all three parts;
- the handoff written, or the entry closed with its evidence;
- ⭐ the **summary table**, printed in chat and saved;
- ⭐ the **next prompt**, printed in chat only, and it is a **resume** prompt if
  anything at all was left unfinished;
- anything created on a remote system, torn down.

---

## When you are unsure

In order: what the operator said in this session, what the linked rule says,
what the probe or the code measured, then ask the operator.

⛔ Never invent a fifth option silently, and never settle a contradiction
between two of these by taking the convenient one. A contradiction is a
finding, and a finding is reported.
