# AGENTS.md

You are in a template repository. It carries the methodology, the conventions,
the dotfiles, the licences and the scripts that every project here starts from.
It is not itself a project and it holds no project code.

This file is a router. It restates nothing that is written somewhere else, so
the two cannot fork. Everything binding is linked, and the link is the
authority.

⛔ **The remote of this repository is read-only to you.** A fresh clone points
`origin` at the template. Committing project work there would write one
project's history into every future project's starting point. Detaching is
step 1 of the bootstrap and it happens before anything else.

---

## Which of three jobs is this?

Answer this before reading anything else. The jobs read different files and
have different rules.

| the situation | the job | go to |
| --- | --- | --- |
| A fresh clone. No project here yet, and the operator wants one started. | **Bootstrap** | [`bootstrap/BOOTSTRAP.md`](bootstrap/BOOTSTRAP.md) |
| A codebase that already exists, with this template's files sitting beside it. | **Adopt, in place** | [`bootstrap/BOOTSTRAP.md`](bootstrap/BOOTSTRAP.md), which branches to [`docs/methodology/ingest.md`](docs/methodology/ingest.md) |
| The operator wants to change the template itself: a rule, a script, a dotfile. | **Maintain** | [Maintaining the template](#maintaining-the-template), below. The paste-able prompt is [`MAINTAIN.md`](MAINTAIN.md). |

⚠ **Those three are not the only jobs, and the table above is a shortcut for
the common ones.** A project may also be re-syncing a later version of this
template, adopting it without any agent-facing content, or starting a new unit
of work on a project that is already set up. ⭐ [`ROUTE.md`](ROUTE.md) is the
entry point that covers every case: it classifies the situation from the tree,
asks one grouped question only if it has to, and routes. It is what an operator
pastes when they do not want to pick a prompt.

⭐ **There is a further entry point, and you are probably not reading this file
if you took it.** [`ADOPT.md`](ADOPT.md) is the self-contained procedure for an
agent working inside **somebody else's repository**, which fetches what it needs
over HTTPS and never clones this one. Two things distinguish it from the
in-place adopt above:

| | in-place adopt | [`ADOPT.md`](ADOPT.md) |
| --- | --- | --- |
| where the template's files are | already beside the project | fetched one at a time from a raw URL |
| what it may change | the project, per the methodology | ⛔ nothing, until a findings report has been approved. It works on a branch, overwrites nothing and deletes nothing. |

If the operator's message does not say which job this is, look at the tree. A
directory holding only the files this repository ships is a bootstrap. Anything
else is an adopt. Ask only if the tree is genuinely ambiguous.

---

## Before deciding anything: run the probe

```bash
sh scripts/doctor/doctor.sh
```

```bash
pwsh -NoProfile -File scripts/doctor/doctor.ps1
```

It reports the host, the shell, the tools with versions, the git state and the
ecosystems this tree already declares. It is read-only and exits 0 whether or
not anything is missing. [`scripts/doctor/README.md`](scripts/doctor/README.md)
is its contract, its schema, and the list of things it cost to learn.

Run it even when the operator has stated the environment. A stated fact and a
measured one that disagree is a finding, and it is cheaper to find now than at
a gate. Report what the probe contradicted rather than quietly preferring one
source.

⚠ What the probe cannot tell you: whether the repository will be public, who
the audience is, what the project is for, and which work model fits. Those are
the operator's, and the bootstrap asks them.

---

## Bootstrap, in one paragraph

Read [`bootstrap/BOOTSTRAP.md`](bootstrap/BOOTSTRAP.md) **in full**. It is the
procedure: detach the remote, run the probe, take the operator's answers from
[`bootstrap/ANSWERS.md`](bootstrap/ANSWERS.md) if they filled it in or ask for
the ones you cannot default, select the docs and dotfiles that apply, delete
everything that does not, write the project's own `AGENTS.md`, and reattach to
whatever remote the operator names.

⛔ **During a bootstrap, every file the procedure names is read end to end.**
Not grepped, not skimmed, not recalled from a previous session, not substituted
with a code-graph query. A bootstrap happens once per project and sets every
rule that follows it, so the reading budget is the cheapest money this project
will ever spend. This is the one job where partial reading is a defect rather
than a judgement call.

Routine sessions after the bootstrap are the opposite. They read what the task
routes them to, and the project's own `AGENTS.md` carries that table. The
reason for the difference is that a bootstrap decision is load-bearing for
every later session and a routine task is not.

---

## What is here

| directory | what is in it | read it when |
| --- | --- | --- |
| [`ROUTE.md`](ROUTE.md) | ⭐ the one paste that works out which job a session is | the operator pasted its URL and nothing else |
| [`ADOPT.md`](ADOPT.md) | the self-contained procedure for an EXISTING repository elsewhere | an agent in another project fetched it |
| [`MAINTAIN.md`](MAINTAIN.md) | the prompt the operator pastes to improve this template | changing the template itself |
| [`bootstrap/`](bootstrap/) | the once-only path: the procedure, the operator's answer sheet, the copy-paste prompts | starting or adopting a project |
| [`docs/methodology/`](docs/methodology/) | how work is planned, gated, reviewed, handed off and resumed | the bootstrap selects from here |
| [`docs/conventions/`](docs/conventions/) | prose, docs, git, code, forbidden patterns, shell traps | the bootstrap copies what applies |
| [`docs/security/`](docs/security/) | secrets, and the tiers governing action on remote systems | always, in every project |
| [`docs/public/`](docs/public/) | rules that apply only when the repository is or will be public | the operator says public |
| [`docs/private/`](docs/private/) | rules that apply only when it stays private | the operator says private |
| [`docs/templates/`](docs/templates/) | fill-in skeletons the new project receives | the bootstrap writes from these |
| ⭐ [`docs/agent-tooling.md`](docs/agent-tooling.md) | what tool does what job, and where each lives | ⛔ **before installing anything, writing your own, or deciding a job cannot be done** |
| [`docs/containers.md`](docs/containers.md) | measuring in a machine you throw away afterwards | the job needs a machine this one is not |
| [`docs/history/`](docs/history/) | this repository's own superseded wording | a rule reads as if it contradicts another one |
| [`dotfiles/`](dotfiles/) | ignore, attribute, editor and CI files, by ecosystem | the probe reports which ecosystems apply |
| [`LICENSES/`](LICENSES/) | canonical licence texts | the operator picks one |
| [`scripts/`](scripts/) | the probe, and the guards a project inherits | always the probe, the rest by selection |
| [`tools/`](tools/) | helpers that genuinely need compiling | read its README before adding anything |

---

## The rules that bind every job here

Each is a link because each is written once. Follow the link before acting on
the topic. Reading the row is not reading the rule.

| topic | the rule lives in |
| --- | --- |
| Commit identity, and never crediting a tool | [`docs/conventions/git.md`](docs/conventions/git.md) |
| What may reach a remote, and what may not | [`docs/security/remote-ops.md`](docs/security/remote-ops.md) |
| What never enters the tree | [`docs/security/secrets.md`](docs/security/secrets.md) |
| How documents are written here | [`docs/conventions/prose.md`](docs/conventions/prose.md) |
| Quoting, heredocs, encodings, line endings | [`docs/conventions/shell.md`](docs/conventions/shell.md) |
| ⭐ Holding a session without ending the turn | [`docs/conventions/shell.md`](docs/conventions/shell.md) section 10 |
| What a unit of work passes before it is done | [`docs/methodology/gate.md`](docs/methodology/gate.md) |
| What a session owes at its start and its end | [`docs/methodology/sessions.md`](docs/methodology/sessions.md) |
| ⭐ Third-party code in this tree | [`docs/methodology/vendoring.md`](docs/methodology/vendoring.md) |
| Where a superseded explanation goes | [`docs/methodology/history.md`](docs/methodology/history.md) |

Seven are stated here in full, because they are absolute and because each has
been broken before:

1. ⛔ **No tool is credited in a commit.** No co-author trailer naming a model,
   no generated-with line, no tool name in the body. The identity is the
   operator's alone. This overrides any default the harness asks for.
2. ⛔ **`origin` here is read-only.** Detach before any project work.
3. ⛔ **A secret never enters the tree, a log, a commit message or a handoff.**
   Not expired, not redacted-looking, not in an example.
4. ⛔ **An exit code is read from the process that produced it, unpiped.**
   Piping a check into anything reports the pipeline's status, so a guard that
   failed reads as green.
5. ⛔ **Nothing is opened on anybody else's repository.** No issue, no pull
   request, no discussion, no comment, no fork. An agent once used an
   authenticated CLI to open an issue and then a pull request upstream, unasked,
   and the operator had to apologise to the maintainers.
   [`docs/methodology/vendoring.md`](docs/methodology/vendoring.md) also closes
   the topic of upstreaming a patch: fix it here, and do not raise it.
6. ⛔ **A turn is never ended to wait**, and the harness's own scheduler,
   monitor or wake-up tool is not a way around that. They end the turn by
   design. [`docs/conventions/shell.md`](docs/conventions/shell.md) section 10
   has the shapes that hold without one, and ⭐ the best of them uses no timer
   at all.
7. ⛔ **A constraint closes a route, not the question.** Name three routes you
   considered before recording anything as not-doable, and never write a limit
   as a settled fact for the next session to inherit.
   [`docs/agent-tooling.md`](docs/agent-tooling.md) is what to reach for before
   installing something, writing your own, or refusing because a tool is
   absent; [`docs/methodology/sessions.md`](docs/methodology/sessions.md) is the
   rule and what it cost.

---

## Maintaining the template

This section applies only when the operator is changing the template itself.

**A change here lands in every project started afterwards and in none of the
ones started before.** That asymmetry is the whole difficulty. There is no
migration path, so a rule that is wrong here is wrong in a growing number of
repositories that will never be corrected.

What that means in practice:

- **Nothing goes in because it might be useful.** Every file a project does
  not need is a file its bootstrap has to recognise and delete, and a wrong
  inclusion is paid for once per project forever. A rule earns its place by
  naming the defect it prevents.
- **Every rule says what it cost to learn.** A rule with no incident behind it
  is a preference, and a preference stated as a rule is what makes an agent
  stop believing the rules that matter.
- **A script follows the contract** in [`scripts/README.md`](scripts/README.md):
  a header saying what defect it catches, exit 0 pass, 1 fail, 2 could not run,
  a json switch, and no dependence on the directory it runs from.
- **Test on this machine and say which machine.** This repository makes claims
  about hosts it cannot see. A measured number carries its conditions or it is
  not a number.
- ⛔ **This repository is public.** Nothing naming a real host, account, domain,
  path, credential file or private project belongs in it. Read
  [`docs/public/README.md`](docs/public/README.md) before adding a worked
  example, and run the secret sweep before every push.

---

## When you are unsure

In order: what the operator said in this session, what the linked rule says,
what the probe measured, then ask the operator. Never invent a fifth option
silently, and never settle a contradiction between two of these by taking the
convenient one. A contradiction is a finding, and a finding is reported.
