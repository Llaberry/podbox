# gate

`docs/methodology/gate.md` and `docs/methodology/experiments.md`. What makes a
check an assertion rather than a decoration, and what makes a number a
measurement rather than a property of one host.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

⛔ A check lands with the plant that proves it can fail, in the same change.

---

### T-1201 The gate reaches every file this project wrote

Source:      `docs/methodology/gate.md`; `docs/conventions/docs.md:80-92`
Category:    gate
Priority:    P0
Effort:      S
Status:      done 2026-09-08

Problem:     The first commit of this repository shipped a README and eight
             crate doc comments naming `TODO/`, `TODO/probe.md`,
             `TODO/interpose.md` and six more, none of which were in the tree.
             The check that would have caught it read `TODO/` alone, so it saw
             none of it, and the check itself was in the missing directory.
Premise:     ⭐ **Measured on this tree.** Two defect classes the gate could not
             see, both found by extending it:
             the citation form that broke the repository carried **no line
             number**, so a `path:line` matcher could never match it; and
             `os.path.isfile` asks this disk rather than a fresh clone, so an
             untracked file reads as present to whoever ran it last and absent
             to everybody else.
             ⛔ **The coverage counts are recorded nowhere, and that is a
             finding rather than an omission.** They are self-referential: this
             file's own citations are among the things counted, so writing the
             number down changes it. Measured while writing this entry, twice:
             `todo_links` moved from 261 to 262 because recording 261 added a
             link. `./scripts/check-todo.py` prints the reading on every run,
             and no document copies it.
             What is fixed rather than measured, and safe to state: bare path
             citations were an **unchecked class** before this entry, and
             `tree_citations` counted **1**.
Approach:    Checks 11 to 15 of `scripts/check-todo.py`. Citations and links are
             resolved across every tracked file this project wrote, against
             `git ls-files` rather than the filesystem. `references/` and the
             verbatim methodology copy are excluded because their text is
             somebody else's; `docs/AGENTS.md` is the one file under `docs/`
             that is written here and is checked like anything else.
             ⚠ Two exemptions, both narrow and both visible in the source: a
             path under `experiments/results/`, which is where a measurement not
             yet taken will land, and a line carrying the `known-absent` token,
             which is how a document names something deliberately not here.
             `git grep -n known-absent` lists every use.
Decision:    Resolve against git, not the disk. A check that agrees with
             whoever ran it last and disagrees with a fresh clone puts the
             disagreement where the one person who could fix it is the one being
             told nothing is wrong.
Prove:       `./scripts/check-todo.py && ./scripts/plant.sh`

**Done.** `./scripts/check-todo.py` exits 0 and prints a coverage line; the two
plants for these checks, cases 11 and 14 in `scripts/plant.sh`, both go red.

---

### T-1202 Every check is planted against, and a plant that stops reaching its subject says so

Source:      `docs/methodology/gate.md`; `docs/methodology/reviews.md`
Category:    gate
Priority:    P0
Effort:      M
Status:      done 2026-09-08

Problem:     A check that quietly matches nothing exits 0 exactly like a check
             whose assertions all passed, and the second is what a reader
             assumes they are looking at. M-1 verified six checks by breaking
             the tree by hand. That verification lived in a transcript, so it
             was worth nothing to the next session.
Premise:     ⭐ **Measured, and it found a real one on its first run.**
             `scripts/plant.sh` plants a defect per case and asserts each makes
             the gate red **with that defect's own message**, and that the
             message was not already there. On its first run, over the fifteen
             cases it then had, case 6 landed its mutation and the gate
             stayed green: each corpus tree is named twice in
             `TODO/reference-map.md`, in the licence table and again in the
             verdicts table, so deleting one row left the other and the check
             never lost the tree. The plant now removes every mention.
             Result on 2026-09-08, after [T-1205](gate.md) added check 18 and its
             two cases: 20 plants caught, 0 missed, 3 controls
             quiet, 0 fired, over eighteen checks. ⛔ **Every check but one has
             a case, and the harness says which on every run.** Check 16, the
             coverage floor, has none: planting it means making a check examine
             nothing, which requires editing the gate's own matchers rather than
             the tree. Listing passing cases without naming the check that has
             none is the same vacuity this entry exists to remove.
             ⚠ The counts here are not held by anything and will move again.
             `./scripts/plant.sh` prints the current pair on every run, and it
             is the answer; this line is a reading from one day.
             ⭐ **A second case rotted and guard 1 caught that too.** Case 3
             named `Status:      open` literally and stopped landing the moment
             `TODO/probe.md` had no open entry left, which is what closing M0
             did. It reported "the mutation did not land" rather than a silent
             green, and the case is status-agnostic now. Cases 4 and 10 took the
             same treatment for the counts, for the same reason.
Approach:    Four guards, each because the harness shape without it reports
             success while planting nothing:
             1. **the mutation must land**, asserted by hashing the file list
                before and after with `git hash-object`. A `sed` that matched
                nothing otherwise reads as a check that cannot fire;
             2. ⛔ **restore from a copy, never `git checkout --`.** Checkout
                restores from the **index**, so a staged plant survives it.
                Measured on 2026-09-08 in a scratch repository: with the plant
                staged, `git checkout -- f` left the plant in the worktree;
             3. **one file list**, iterated by both the backup and the restore,
                so a case that learns to touch a new file cannot leave it
                behind for every later case to measure;
             4. **controls counted apart from plants.** A control stays quiet, a
                plant goes red; adding them reports more caught than were.
Decision:    Assert the message, not the exit code. A gate already red for
             another reason exits 1 either way, so a case watching only the exit
             code passes vacuously the moment anything else breaks.
Prove:       `./scripts/plant.sh`

**Done.** Exit 0: 20 plants caught, 0 missed; 3 controls quiet, 0 fired.
⚠ The pair moves whenever a check lands. `./scripts/plant.sh` prints it, and
that is the answer; this line is a reading from the day it was taken.

---

### T-1203 A measurement taken on one host is a property of that host

Source:      `docs/methodology/experiments.md`; `TOOL.md` section 5 M5
Category:    gate
Priority:    P1
Effort:      M
Status:      done 2026-09-08

Problem:     Every number in `experiments/results/` was taken on one machine
             with one libc and one `nsswitch.conf`. An artefact that starts on
             every distribution and does the right thing on none of them reads
             as success to a single-host smoke test.
Premise:     ⭐ **Measured here, and it is why this entry exists.**
             `experiments/results/nsswitch-contract.txt` check B reads three
             pinned images: two name `files` and one ships no `nsswitch.conf`
             at all. Same question, three different answers, and podbox's
             behaviour has to be right on all three.
             ⭐ **And the runner has now taken the reading.** All eleven rows
             pulled and ran, four musl and seven glibc, alpine 3.10 through
             fedora 42. `experiments/results/across-distributions.txt`:

             ```
             alpine-3.22          musl   present-no-passwd-line  seen
             alpine-3.20          musl   present-no-passwd-line  seen
             alpine-3.10          musl   absent                  seen
             voidlinux-musl       musl   files                   seen
             debian-11            glibc  files                   seen
             debian-12            glibc  files                   seen
             ubuntu-20.04         glibc  files                   seen
             rockylinux-8         glibc  sss files systemd       seen
             opensuse-leap-15.6   glibc  compat                  probe-crashed-rc136
             fedora-42            glibc  files systemd           seen
             archlinux-latest     glibc  files systemd           seen
             ```

             ⚠ **Six distinct shapes of `nsswitch` across eleven rows**:
             `files`, `files systemd`, `sss files systemd`, `compat`, a file
             present with no `passwd` line, and no file at all. That is the
             whole argument for T-0410 probing rather than assuming.
             ⛔ **And one row where a static glibc binary does not run at all.**
             `rc136` is 128 plus 8, SIGFPE, on `opensuse-leap-15.6`, from a
             probe built on this host's glibc 2.39 and static-linked. That is a
             reading about static portability and not about `nsswitch`, which is
             why the runner reports `probe-crashed` rather than `not-seen`. It
             also means the `compat` row is untested for the passwd question:
             the probe died before answering.
Approach:    One runner, one command, one row per pinned distribution. The
             subject is a script mounted read only, the checkout is mounted read
             only, and every row gets the same bytes, so a difference between
             rows is a difference between distributions and nothing else.
             Four rules that make it a measurement rather than a survey:
             1. the manifest **digest** is what makes a number from three months
                ago comparable to one from today. A rolling tag measures a
                different thing each week and says so nowhere;
             2. ⚠ a row that could not be pulled reads `no-pull`, is counted
                apart from a row whose command ran and failed, and does not on
                its own make the run exit 1. "Unreachable" and "broken" need
                different next moves;
             3. ⛔ a run where **nothing** ran exits 2. Every row unreachable
                reads exactly like every row agreeing;
             4. fully qualify the reference before pulling. An unqualified name
                resolves through an engine's own shortname aliases to a
                different registry, where a Docker Hub digest does not exist,
                and that arrives as `manifest unknown`, which reads as a broken
                pin.
             ⚠ The subject must copy itself out of the read-only mount before
             running, and needs `HOME` set to a writable path: several of these
             images have no writable home for a non-root uid, and that failure
             looks like the subject's.
Decision:    A matrix, not a wider single-host test. The reading that matters is
             not "it fails on alpine 3.10"; it is an artefact that starts
             everywhere and works nowhere, which one host cannot show.
Prove:       `./experiments/125-across-distributions.sh` exits 0 or 1, never 2, and writes one transcript per row into `experiments/results/`

**Done.** Exit 0, eleven rows ran, eleven transcripts in
`experiments/results/across/`. Four defects in the instrument were found by the
first run and fixed in place: a combined `ls` glob that reported `unknown` for
every glibc row, a `tr -s ' '` that did not squeeze the tab voidlinux uses, a
`nsswitch` reading that folded "absent" into "present and silent", and a probe
crash reported as "not seen".

---

### T-1204 Check 17 holds the newest committed reading under the ceiling, not only the baseline

Source:      Found by reading check 17 against the tree after M1 landed
Category:    gate
Priority:    P1
Effort:      S
Status:      open

Problem:     Check 17 reads `experiments/results/bloat-baseline.txt` and asserts
             its `total_bytes` is under the ceiling. That file is deliberately
             the "before": M0's artefact, 496,184 bytes. It is not the shipping
             binary any more, so the number the gate holds is not the number the
             ceiling exists to hold.
Premise:     ⭐ **Measured on 2026-09-08.** M1 moved the artefact from 496,184 to
             2,130,672 bytes, and `experiments/results/bloat-image.txt` records
             it. `scripts/check-todo.py` went green throughout, because the file
             it reads did not change. ⚠ The ceiling is still enforced at BUILD
             time by `experiments/110-bloat-delta.sh`, which exits 1 over it, so
             nothing is currently unprotected: what is missing is the half that
             runs on a clone with no toolchain, which is the half check 17 was
             written to be.
Approach:    Read every `experiments/results/bloat-*.txt` rather than the
             baseline alone, and assert each `total_bytes` is under the ceiling.
             ⛔ Still no build: the check reads committed evidence, so it keeps
             working on a fresh clone with no cargo, which is
             `scripts/check-todo.py`'s own constraint.
             ⚠ The baseline keeps its own separate assertion, because a missing
             baseline means there is no "before" and `TODO/deps.md` T-0910 rules
             that a dependency without one has not landed.
Decision:    Every reading, not "the newest by date". A date inside a file is a
             string this script would have to parse and rank, and a stale file
             somebody forgot to delete would then silently outrank a real one.
             Asserting all of them needs no ordering and fails on the same file
             either way.
Prove:       `./scripts/plant.sh` reports 19 caught, 0 missed, with the new case planting a `total_bytes` over the ceiling into `experiments/results/bloat-image.txt`

---

### T-1205 The gate holds experiment numbers unique, because four Proves already collide

Source:      Found by sweeping every `Prove` in `TODO/` against `experiments/` at the close of M1
Category:    gate
Priority:    P1
Effort:      S
Status:      done 2026-09-08

Problem:     `experiments/README.md` rules that a number is never reused,
             because a citation of `30-` has to keep meaning what it meant.
             Nothing enforces it, and four entries already name a number that is
             taken. Each will be discovered by whoever implements it, mid-flight,
             exactly as [T-0203](image.md)'s was.
Premise:     ⭐ **Measured on 2026-09-08**, by matching every
             `experiments/<n>-<name>.sh` named in a `Prove` against the files on
             disk:

             | the `Prove` named | the number was already | renumbered to |
             | --- | --- | --- |
             | `80-extract-path-safety.sh` ([T-1103](milestones.md), [T-0304](extract.md)) | `80-interposer-abi.sh` | `220-` | <!-- known-absent -->
             | `100-lifecycle-loop.sh` ([T-1105](milestones.md), [T-0607](supervise.md)) | `100-interpose-symbols.sh` | `230-` | <!-- known-absent -->
             | `130-distro-sweep.sh` ([T-1106](milestones.md)) | `130-probe-parity.sh` | `240-` | <!-- known-absent -->
             | `140-negative-tests.sh` ([T-1109](milestones.md)) | `140-space-precheck.sh` | `250-` | <!-- known-absent -->

             ⚠ Four more name a free number and are fine: `120-`, `85-`, `95-`,
             and the four M1 authored at `180-` to `210-`.
             ⛔ [T-1103](milestones.md)'s is the one that bit first: it is M2's
             own acceptance, and M2 is the session that closed this entry.
             ⭐ **The check reproduced the sweep.** Written before the
             renumbering and run against the tree that carried all four, it
             reported exactly these four numbers and named the same
             `file:line` on each. A hand sweep and a check that agree is the
             check having been tested against a real defect rather than a
             planted one.
             ⚠ The renumbering is an append, not a gap fill, and the new numbers
             keep `experiments/README.md`'s "order they run": M2 is `220-`, M4
             `230-`, M5 `240-`, M6 `250-`.
Approach:    A check in `scripts/check-todo.py` that collects the leading number
             of every `experiments/<n>-*.sh` named anywhere this project wrote,
             plus every such file on disk, and fails when one number carries two
             names. ⛔ It reads text and the filesystem listing only, so it keeps
             working on a clone with no toolchain, which is that script's own
             constraint.
             ⛔ The check and its plant land in the same change:
             `docs/AGENTS.md` and [T-1202](gate.md). The plant renames a number
             into a collision and asserts the gate goes red with this check's own
             message.
             ⚠ Renumbering the four above is part of this entry, in the entries
             that name them, and each keeps its title.
Decision:    Enforce uniqueness rather than dropping the rule. The rule exists
             because a number in a closed entry is a citation somebody has
             already written down, and `experiments/README.md` says so; a rule
             worth writing and not worth checking is the kind that stops being
             believed.
Prove:       `./scripts/plant.sh` reports 20 caught, 0 missed, with the two new cases planting a duplicate experiment number

**Done, 2026-09-08.** Check 18 of `scripts/check-todo.py`, and cases 18a and
18b of `scripts/plant.sh`, in this change. `./scripts/plant.sh` exits 0 with
**20 caught, 0 missed, 3 controls quiet**.

⭐ **The check was written before the renumbering and run against the tree that
still carried all four collisions.** It reported exactly the four this entry
records and named the same `file:line` on each. A hand sweep and a check that
agree independently is the check having been tested against a real defect as
well as a planted one, which is the half `scripts/plant.sh` cannot supply.

⚠ **Two cases, not one, because the halves fail apart.** A number can be
claimed by a document promising a script (all four real collisions were this
shape) or by a second file arriving on disk. A case for one leaves the other
unseen, which is this harness's own founding defect.

⛔ **The planted number is read out of the listing at run time and never
written into `scripts/plant.sh`.** The gate reads that file like any other, so
a literal taken number there would put a second name on it and redden the clean
tree, exactly as two literal citations did on 2026-09-08 and as `CEILING_NUM`
would. The same trap, a third time, in the same file.

⚠ **A line carrying the `known-absent` token is skipped**, as check 14 skips
it. This entry's own Premise table names the four old numbers beside their new
ones, and a check that read that table would report the defect the table exists
to describe.
