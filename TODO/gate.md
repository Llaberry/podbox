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
             `scripts/plant.sh` plants fifteen defects and asserts each makes the
             gate red **with that defect's own message**, and that the message
             was not already there. Case 6 landed its mutation and the gate
             stayed green: each corpus tree is named twice in
             `TODO/reference-map.md`, in the licence table and again in the
             verdicts table, so deleting one row left the other and the check
             never lost the tree. The plant now removes every mention.
             Result on 2026-09-08: 18 plants caught, 0 missed, 3 controls
             quiet, 0 fired, over seventeen checks. ⛔ **Every check but one has
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

**Done.** Exit 0: 15 plants caught, 0 missed; 3 controls quiet, 0 fired.

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
