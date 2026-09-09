# supervise

`crates/podbox-supervise`. `TOOL.md` section 6.6, milestone M4.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

Running state is launcher state, in the launcher. The prior art's own lifecycle
capture contains a failed run because it decided "running" by sleeping three
seconds and looking, and M4's acceptance is twenty consecutive passes because
one pass proves nothing about a race.

---

### T-0601 One pidfd per direct child, `waitid` for status

Source:      `TOOL.md` section 4.2, section 6.6; `paper_final.md` section 10.6
Category:    supervise
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     Membership inferred from the filesystem races against exit, depends
             on other processes' root links being readable, and trusts a path any
             writable payload can create. Scanning `/proc/*/root/<marker>` is all
             three at once.
Premise:     Read.
Approach:    Hold a pidfd per direct child from the moment it is created. Exit
             status via `waitid`, wakeups via the pidfd in a poll set. Keep the
             container table in the launcher, persisted to the store, and never
             reconstruct it from `/proc`.
             ⛔ Say what a pidfd is not: it addresses one process, it does not
             contain descendants, and it is not a PID namespace. Grandchildren
             that reparent are outside podbox's reach and `inspect` says so
             rather than implying containment.
Decision:    A pidfd rather than a pid plus a start time. The pid is reusable
             and the start-time check is a heuristic; a pidfd cannot address a
             process that has been reaped.
Prove:       `podbox run -d --name pidfdprobe alpine:latest sleep 5 && podbox wait pidfdprobe | grep -qx 0 && podbox rm pidfdprobe`

**Done, 2026-09-09.** `crates/podbox-supervise/src/launcher.rs`. One launcher per
container holds a pidfd on its payload from the moment the payload exists,
`ppoll`s it for readiness and reaps it with `waitid(P_PIDFD)`, so the status
cannot be redirected by pid reuse between the readiness and the reap. The
container table is `crates/podbox-supervise/src/table.rs` and nothing in it is
reconstructed from `/proc`.

⛔ **Said, not implied**, and `podbox inspect --format '{{.Contains}}'` prints it:
a pidfd addresses ONE process. It does not contain descendants and it is not a
PID namespace, so a grandchild that reparents is outside podbox's reach.

⚠ **A defect the `waitid` path cost, and it is a fixed offset rather than a
race:** `si_status` was read at byte 20 of the `siginfo_t`, which is `si_uid`. A
SIGTERMed payload reported exit **128** instead of 143. On a 64-bit architecture
the three leading ints are followed by four bytes of padding and `_sifields`
begins at 16, so `si_status` is at 24. `crates/podbox-probe/src/sys.rs` carries
the reasoning beside the read.

---

### T-0602 Never decide "running" by sleeping and looking

Source:      `TOOL.md` section 6.6; `paper_final.md` section 10.6
Category:    supervise
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     A fixed sleep is a scheduling assumption. The prior art's own
             capture of this lifecycle contains a failed run for exactly that
             reason, and it passed on other days.
Premise:     Read, in the account of that capture. The general rule is in the
             binding methodology: `docs/methodology/authoring.md:143-148` states
             that an acceptance "waits on the condition, never on a guessed
             duration", and that "both of these will happen" is the same
             assumption as "this will happen in N seconds".
Approach:    Wait on the pidfd, or on a readiness fd the child writes to and the
             parent reads. `start` returns when the child has reached a defined
             state, not when a timer expires. Every wait has an upper bound and
             a distinct outcome for the bound being reached, per
             [RULES.md](RULES.md) section 8.
             ⛔ podbox inherits this as a product requirement, not just as
             session hygiene: a container runtime that hangs waiting is unusable
             by the audience it is built for.
Decision:    A readiness fd over polling `ps`. Polling reintroduces the interval
             the entry exists to remove.
Prove:       `for i in $(seq 20); do podbox run -d --name loop$i alpine:latest sleep 1 && podbox ps -q | grep -q . && podbox stop loop$i && podbox rm loop$i || exit 1; done`

**Done, 2026-09-09**, and it took three failing runs and a door sweep to get
there. `./experiments/230-lifecycle-loop.sh 20` reports **20 of 20 consecutive
passes**.

⭐ **What is in.** Readiness is an `O_CLOEXEC` pipe that the payload's `execve`
closes, so `start` returns when the exec has SUCCEEDED rather than when a timer
expired; the launcher waits on `ppoll` over the payload's pidfd and its control
socket; `stop` and `wait` block on the launcher through that socket with a read
timeout. ⛔ There is no `sleep` anywhere in the path, and
`experiments/230-lifecycle-loop.sh` has none either.

⛔ **What the acceptance found before it passed, and this is the entry's whole
point.** The twenty-pass loop failed three runs out of three:

| run | consecutive passes | failed at |
| --- | --- | --- |
| 20 iterations | **10 of 20** | `stop` |
| 20 iterations, again | **9 of 20** | `stop` |
| 3 iterations | **1 of 3** | `stop` |

Always the same message: `no launcher is listening for <id>: No such file or
directory`. The container table still says running at that moment, so the
launcher's own lock was still held and its control socket was not there.
⚠ Reproduced by hand three times in a row against a warm store and it passed
every time, so what the loop adds is a cold store, an `rm` of the previous
container immediately before, and a `pull` at the start.

⭐ **THE CAUSE, found by the door sweep and not by the loop: `stop` connects
TWICE.** Once to send `SIGTERM`, once to wait for the exit. A payload that dies
quickly lets the launcher reap it, remove its control socket and exit **in
between the two connects**, so the second one answers `ENOENT` and `stop`
reported the fastest possible success as "the container is not running". ⛔ The
failure was more likely the faster the container stopped, which is why a
hand-driven loop with a person's pauses in it passed every time.

⭐ **The fix is that a launcher which is already gone is a container that already
stopped, and `Ended` says which of three things happened**: the launcher
answered with a code, there is no launcher left to answer, or the bound was
reached. Only the last means podbox does not know, and collapsing the first two
is what made this a failure. `podbox wait` had the same shape and takes the same
fix, re-reading the table because the launcher's last act is to write the code
into it.

⚠ **Two candidates were written down before the sweep and BOTH were wrong**,
which is worth keeping: `contain::within` re-appends the tail after resolving the
nearest existing ancestor, so `remove_dir_all` can never widen to the containers
directory; and `flock` is held on an open file description, so `reconcile`
opening and closing its own fd cannot release a launcher's lock. Reading the code
for what else reaches the socket is what found it.

---

### T-0603 `PR_SET_PDEATHSIG` fires on the creating thread's exit

Source:      `TOOL.md` section 6.6; `paper_final.md` section 10.6
Category:    supervise
Priority:    P1
Effort:      S
Status:      done 2026-09-09

Problem:     It does not fire on an ancestor's death. In a threaded supervisor
             this produces surprising early exits that look like crashes, and a
             wrapper such as `timeout(1)` around the launcher is enough to kill a
             detached child unexpectedly.
Premise:     Read. It is a kernel property, not a runtime property, and it is one
             of the four rows behind `TOOL.md` section 3's language decision: a Go
             supervisor cannot control which thread it is on.
             `references/89luca89__lilipod/tree/pkg/procutils/proc_utils.go:48-56`
             sets `Pdeathsig: syscall.SIGTERM` in the same `SysProcAttr` as the
             `Credential` that fails, which is the shape the account describes.
Approach:    podbox is single-threaded on the path that clones, chroots and
             execs (`TOOL.md` section 4.2), so the creating thread is the process. Keep
             it that way: spawn threads only for log pumps and the notification
             supervisor, and assert it in a test rather than in a comment.
             Where `PDEATHSIG` is used, lock the spawning thread.
Decision:    Keep the spawn path single-threaded rather than pinning a thread.
             Pinning works and it is a constraint every future contributor has to
             know; a single-threaded path is a constraint the type system can be
             made to hold.
Prove:       `podbox run -d --name pdprobe alpine:latest sleep 30 && grep -qx 1 /proc/$(podbox inspect --format '{{.Pid}}' pdprobe)/status.threads 2>/dev/null || test "$(ls /proc/$(pgrep -f 'podbox run' | head -1)/task | wc -l)" -le 3`

**Done, 2026-09-09.** The spawn path is single-threaded and nothing on it spawns
a thread: the launcher forks with `clone_fork`, the payload's `chroot` and
`execve` happen in that child, and `PR_SET_PDEATHSIG` is not used at all, so the
"fires on the creating thread's exit" trap has no way to bite. The launcher also
`setsid`s, so it outlives the shell that started it and holds no controlling
terminal.

⚠ **Asserted in two places rather than commented, and the first version of the
first one was wrong.** A unit test read `/proc/self/task` before and after the
spawn path and failed about one run in five, because `cargo test` runs tests in
THREADS of one process and that count moves for reasons that have nothing to do
with podbox: a test whose name claimed more than it checked. It now reads the
two crates' own sources and asserts neither spawns a thread at all, with the
needles assembled at run time so the assertion does not match itself. ⭐ The
runtime half is clause 3 of `experiments/230-lifecycle-loop.sh`, which reads
**1** thread off a real detached launcher.

---

### T-0604 Running state is launcher state

Source:      `TOOL.md` section 6.6
Category:    supervise
Priority:    P1
Effort:      M
Status:      done 2026-09-09

Problem:     Two launchers, or a launcher restart, must not disagree about what
             is running, and neither may read the answer out of a directory a
             payload can write to.
Premise:     Read.
Approach:    A single container table in the store, written by the launcher,
             locked with the same inheritable lock fd as T-0204. On start, a
             launcher reconciles the table against its pidfds and marks anything
             it cannot address as exited, recording when it noticed. It never
             infers membership from a marker file.
Decision:    Reconcile on start rather than trust the table. A launcher killed
             with `SIGKILL` leaves the table saying "running", and the only
             honest answer after that is "this process exited while podbox was
             not watching".
Prove:       `podbox run -d --name stateprobe alpine:latest sleep 30 && kill -9 "$(podbox inspect --format '{{.Pid}}' stateprobe)" && podbox ps -a --format '{{.Status}}' --filter name=stateprobe | grep -qi exited && podbox rm stateprobe`

**Done, 2026-09-09.** Clause 2 of `experiments/230-lifecycle-loop.sh` drives it:
a detached container whose LAUNCHER is `SIGKILL`ed reads `dead`, `inspect
--format '{{.ExitCode}}'` prints a dash, a time is recorded for when it was
noticed, and `podbox wait` on it exits **125** rather than printing a number.

⭐ **The mechanism.** A launcher holds an exclusive `flock` on its container's
own lock file for its whole life, so `table::reconcile` decides whether a
launcher is gone by TRYING TO TAKE IT: a lock this process can take is a launcher
that is not there. ⛔ Never by pid, and never with a start-time check beside it: a
pid is reused and the check that clears a stale pid file is the race being
closed. Such a container becomes `dead`, with the time it was NOTICED and ⛔ **no
exit code at all**; `podbox wait` on it refuses rather than printing a number,
and `inspect --format '{{.ExitCode}}'` prints a dash.

⚠ **The payload outlives its launcher and that is not hidden.** Killing the
launcher does not kill the container's process, so the clause kills it too on the
way out; podbox has no PID namespace and `inspect --format '{{.Contains}}'` says
exactly that.

---

### T-0605 Capture logs at spawn, from the descriptors opened in step 2

Source:      `TOOL.md` section 6.6, section 6.5
Category:    supervise
Priority:    P2
Effort:      S
Status:      done 2026-09-09

Problem:     A log sink opened after the chroot cannot reach the store, which is
             outside the new root. Opening it afterwards is the same mistake as
             step 2 of section 6.5 in a different place.
Premise:     Read.
Approach:    Open the log sinks in step 2 of the entry sequence, before the root
             change, and hand the descriptors to the child. `logs` reads the
             store file; `attach` is a tail of the same file plus a signal proxy
             and is marked Degraded for that reason.
             `--log-driver` accepts `json-file` only, and any other value is a
             named refusal rather than a silent substitution.
Decision:    A file in the store rather than a pipe pumped by the launcher. A
             pipe loses everything written after the launcher dies, and the
             launcher is the process most likely to be killed by an outer
             `timeout`.
Prove:       `podbox run --rm --name logprobe alpine:latest sh -c 'echo out; echo err >&2' >/dev/null 2>&1; podbox logs logprobe 2>&1 | grep -q out`

**Done, 2026-09-09.** Clause 4 of `experiments/230-lifecycle-loop.sh`:
`podbox run -d ... sh -c 'echo out; echo err >&2'` then `podbox logs` reads back
`out err`, so both streams reached one file opened before the chroot.

The log sink is opened in the launcher BEFORE anything changes root, and its
descriptors are handed to the payload as fds 1 and 2 through the same
`Plan.fds.pass` the entry sequence already had; stdin is `/dev/null`, opened in
the same place. `podbox logs` reads the file out of the store. ⛔ A sink opened
after the chroot cannot reach the store, which is outside the new root, and that
is step 2 of `TOOL.md` section 6.5 made in a different place.

⚠ `--log-driver` is not implemented at all rather than accepted and ignored, and
`attach` is a `None` row in the parity table for the same reason.

⚠ The two streams are interleaved into one file rather than kept apart, which
is what `json-file` does and is why `logs` has no `--tail` or stream selector.

---

### T-0606 The notification tier: probe three legs, refuse the tier, never fall back per call

Source:      `TOOL.md` section 4.1, section 6.6; `references/multikernel__sandlock`
Category:    supervise
Priority:    P0
Effort:      L
Status:      blocked

Problem:     `supervise` is the only rung whose failure is silent by default. Its
             listener keeps working after its argument-reading channel dies, and
             a supervisor whose per-syscall fallback is "continue" then reports
             success for mediation it never performed.
Premise:     ⭐ **Read at file and line, and the whole chain is traceable in one
             tree.** In `references/multikernel__sandlock`:

             1. `tree/crates/sandlock-core/src/seccomp/notif.rs:1015-1034`
                `read_child_mem_vm()` calls `libc::process_vm_readv` at
                `:1026`. That is the only read channel; there is no
                `/proc/pid/mem` fallback anywhere in the tree.
             2. `tree/crates/sandlock-core/src/seccomp/notif.rs:1070-1081`
                `read_child_mem()` propagates the failure.
             3. `tree/crates/sandlock-core/src/chroot/dispatch.rs:213-231`
                `read_path()` returns `None` on any read failure.
             4. `tree/crates/sandlock-core/src/cow/dispatch.rs:203-206` turns
                that `None` into `NotifAction::Continue`, and the same shape
                appears at `:200`, `:219`, `:226`, `:238`, `:245`, `:253`,
                `:276` and `:293`.
             5. `Continue` lets the kernel run the syscall unmediated, so
                nothing is recorded.
             6. `tree/crates/sandlock-cli/src/main.rs:740-747` then prints
                `sandlock: dry-run: no filesystem changes`.

             On a runtime where `process_vm_readv` is filtered, every step 4
             fires and step 6 reports no changes while the files change.
             ⭐ **Its own audit examined every one of those sites and missed
             this.** `tree/crates/sandlock-core/src/cow/dispatch.rs:6-24` is a
             module header written by that audit, titled "Continue safety (issue
             #27)", arguing each `Continue` is safe because "we're not making a
             security decision whose validity depends on the kernel re-reading
             the same memory we read". That is correct about security and silent
             about reporting.
             ⭐ **And its tracker settles a second question `TOOL.md` does not
             raise.** Issue #27 is open. `seccomp_unotify(2)`'s own manual warns
             the mechanism must not be used to make security policy decisions,
             because it is race-prone. The fix that survived four rounds of
             argument (PRs #28, #29, #33, #35) requires `PTRACE_SEIZE` plus
             `PTRACE_INTERRUPT` on every thread of every process in the sandbox
             before sending `Continue`, and **`ptrace` is filtered on this
             runtime**. So `supervise` here has no read channel and no way to be
             made race-safe if it had one. PR #29 also dropped path strings from
             the policy surface entirely, on the ruling that "path-based access
             control belongs in static Landlock rules".
Approach:    Probe all three legs before selecting the tier: the listener, the
             `SECCOMP_IOCTL_NOTIF_ADDFD` injection, and a working channel for
             reading the child's arguments. Refuse the tier if any is missing.
             ⛔ Never fall back per call. A per-call fallback is what produces a
             false report, and it is indistinguishable from success in the
             output.
Decision:    Refuse the whole tier when any leg is missing, rather than offering
             a reduced `supervise`. A reduced one is what the corpus measures
             failing: the listener keeps working, so the mode reports as active,
             and only the mediation is gone. There is no output that
             distinguishes it from the working case, which is why the refusal has
             to happen before the tier is entered.
Status note: **blocked** on `supervise` being reachable at all. On the target
             both read channels are denied, so the tier is refused there and the
             probe is what has to ship. The entry stays open because the probe is
             M0 work and the refusal has to be a measured verdict rather than an
             assumption about a machine.
Prove:       `podbox probe --json | jq -e '.tiers.supervise.legs | length == 3 and (map(select(.ok == false)) | length == 0 or (.[0].refused == true))'`

---

### T-0607 The lifecycle, twenty times, twenty passes

Source:      `TOOL.md` section 5 M4, section 9
Category:    supervise
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     One pass proves nothing about a race, and the race is the reason
             this milestone exists.
Premise:     Read, from the account of a capture containing a failed run beside
             a passing one.
Approach:    `create`, detached `start`, `ps` shows it running, `exec` prints a
             marker, `stop`, `rm`. Twenty consecutive iterations, all passing,
             with no sleep anywhere in the loop or in the implementation under
             it. A single failure fails the milestone; retrying it is what turned
             a race into a published pass elsewhere.
Decision:    Twenty rather than a timed soak. A count is reproducible on a
             different machine and a duration is not, and
             `docs/methodology/authoring.md:143-148` rules out the duration form.
Prove:       `./experiments/230-lifecycle-loop.sh 20` exits 0

**Done, 2026-09-09.** `./experiments/230-lifecycle-loop.sh 20` reports **20 of
20 consecutive passes**, and the three clauses after it run for the first time.

⭐ **This entry earned its existence before it passed.** Three runs failed at
10 of 20, 9 of 20 and 1 of 3, always at `stop`, and the cause was a real race in
`stop` itself rather than in the loop: [T-0602](supervise.md) carries it. A
single pass would have shipped that defect, and a retry would have published it.

⚠ **The script never retries and never continues past a failure**, so the number
it reports is how many iterations happened BEFORE the first one, which is the
only number this entry is about.

---

### T-0608 A detached container that reads `exited` with no launcher, seen twice and not reproduced

Source:      Observed on 2026-09-09 while writing `experiments/250-negative-tests.sh`
Category:    supervise
Priority:    P1
Effort:      M
Status:      done

Problem:     ⛔ **`podbox run -d` returned a container id, and one second later
             `inspect` read `exited`, pid `0`, launcher pid `0`, exit code `0`,
             for a payload that was `sleep 20`.** `logs` was empty. Whatever
             happened, the record says the container ran and ended cleanly, and
             it did not: a `sleep 20` that exits 0 in under a second is not the
             payload's answer, and reporting it as one is the class of lie
             [T-0604](supervise.md) exists to refuse.
Premise:     ⚠ **SEEN TWICE AND NOT REPRODUCED, and that is written down rather
             than smoothed into a diagnosis.** Both sightings were in the same
             shell, against `public.ecr.aws/docker/library/alpine:3.20`, in a
             fresh store, while `experiments/240-distro-sweep.sh` had all four
             CPUs. Immediately afterwards:

             - six consecutive `run -d` of the same image in a fresh store read
               `running` with a launcher pid: **0 bad of 6**;
             - the same image and `ghcr.io/pkgforge-dev/archlinux:latest`, one
               each: both `running`;
             - `experiments/230-lifecycle-loop.sh 3` passed **3 of 3**, and its
               own T-0604 clause reports `a launcher pid was recorded: yes`.

             ⛔ No cause is claimed. Two candidates are worth ruling out first
             and neither has been: the readiness pipe's bound being reached under
             load and reported as an exit rather than as "podbox does not know",
             and `reconcile` running between `create` and `start` and writing a
             terminal state over a container that had not started yet. ⚠ The
             second would explain the `0` exit code, because `reconcile`'s
             `dead` path leaves no code and this record had one.
Approach:    Reproduce it before changing anything.
             1. a loop like `230-lifecycle-loop.sh`'s, but with the machine
                LOADED, because both sightings were under a full four-CPU build.
                ⚠ Load is the one condition the passing runs did not share;
             2. record, for every iteration, the state, the pid, the launcher
                pid and the exit code, and stop on the first that is terminal
                within a second;
             3. only then read `start` and `reconcile` against what the
                reproduction shows.
             ⛔ Do not "fix" the two candidates above on the strength of the
             observation. M4's own race was found by enumerating the doors after
             two written-down causes were both wrong
             ([T-0602](supervise.md)), and the entry keeps them for that reason.
Decision:    An entry rather than a note in `PROGRESS.md`, because an
             intermittent that nobody is assigned to reproduce is one that is
             rediscovered instead of fixed. ⚠ P1 and not P0: it was not seen in
             any acceptance run, and the acceptance is what a release gates on.
Prove:       `./experiments/340-detached-stdio.sh 40` reproduced it on iteration 1 before the fix and reports 40 of 40 after it


**Done 2026-09-09.** `crates/podbox-supervise/src/launcher.rs`, and ⛔ **neither
of the two candidates above was it.** The entry refused to act on either and
that was right.

⭐ **THE LAUNCHER KEPT THE CALLER'S STDOUT.** `clone` copies every descriptor and
the launcher never execs, so it inherited the caller's stdin, stdout and stderr
and held them for the container's whole life. `podbox run -d` prints a container
id, so every caller captures it -- `id="$(podbox run -d ...)"` -- and command
substitution reads that pipe **to EOF**. The id was written in the first second;
the shell was released when the container ended.

⚠ **The sighting was accurate and the timing was the other way round.** "`run -d`
returned an id and one second later `inspect` read `exited`, pid 0, launcher pid
0, code 0" is what a caller sees when it is released at the END: the launcher had
already written the terminal record, and `pid` and `launcher_pid` are `None` on
that path by design.

Measured on 2026-09-09, before the fix:

```
$ id=$(podbox run -d --name subst <arch> /bin/sleep 20)   # took 20 s
$ ls -l /proc/<launcher>/fd/1
  l-wx------ 1 root root 64 ... 1 -> pipe:[93367]
```

⛔ And with stdout redirected to a file rather than captured, the same command
returned in under a second, which is why eight `run -d` in a row had looked fine:
**the instrument was the caller's own stdout**, and a shell that is not reading
the pipe never waits on it.

⚠ **THE LOAD WAS NOT THE CAUSE**, and the entry's warning about it was still
worth having: it reproduces on an idle machine in one iteration. Both sightings
happened to be under a four-CPU build because that is when somebody was watching.
`experiments/340-detached-stdio.sh` keeps the load anyway -- a scheduler under
pressure is where a handoff bug would hide, and removing the one condition the
entry named would be answering a different question.

The fix is one `dup2` triple after `setsid()`: the launcher's own 0, 1 and 2 go
to `/dev/null`, which is what daemonising has always meant. ⛔ It **refuses**
rather than shrugging where `/dev/null` cannot be opened, because a launcher that
cannot let go of the caller's stdio will hang it, and hanging silently is exactly
this defect.

⚠ **The payload's own stdio was never the problem** and is unchanged: `0` is
`/dev/null` and `1` and `2` are the container log, handed over before the chroot
([T-0605](#t-0605-the-container-log-and-what-podbox-may-claim-about-it)).

Prove, run 2026-09-09:

```
$ ./experiments/340-detached-stdio.sh 40
  before the fix: ⛔ iteration 1 REPRODUCED IT
  after:          detached starts that returned promptly and read running: 40 of 40
                  the longest `run -d` took: 1s, against a bound of 5s
```
