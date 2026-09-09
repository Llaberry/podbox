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
Status:      partial 2026-09-09

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

**Partial, 2026-09-09. The mechanism is in and the acceptance does not pass.**

⭐ **What is in.** Readiness is an `O_CLOEXEC` pipe that the payload's `execve`
closes, so `start` returns when the exec has SUCCEEDED rather than when a timer
expired; the launcher waits on `ppoll` over the payload's pidfd and its control
socket; `stop` and `wait` block on the launcher through that socket with a read
timeout. ⛔ There is no `sleep` anywhere in the path, and
`experiments/230-lifecycle-loop.sh` has none either.

⛔ **What does not pass, and it is this entry's own `Prove`.** The twenty-pass
loop fails, three runs out of three:

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

⚠ **Two candidates, neither established.** `podbox_supervise::remove` calls
`remove_dir_all` on the container's own directory, resolved through
`crates/podbox-image/src/contain.rs`; if that resolution ever answers with an
ancestor for a leaf that has gone, it would take the containers directory whole.
And `table::reconcile` takes and drops an exclusive lock on every running
container's lock file on every `ps`, `exec`, `stop` and `rm`, which is a write to
a file a launcher is holding.

⛔ **Not retried and not published as a pass.** [T-0607](supervise.md)'s
Decision: retrying one failure is how a race becomes a published pass, and the
number that matters is how many iterations happened before the first failure.

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

⚠ **Asserted in two places rather than commented.** A unit test in
`crates/podbox-supervise/src/launcher.rs` asserts this process grows no thread
across the spawn path, and clause 3 of `experiments/230-lifecycle-loop.sh` reads
`/proc/<launcher>/task` of a real detached launcher and requires exactly one.
⛔ That clause has not run: the loop fails before reaching it.

---

### T-0604 Running state is launcher state

Source:      `TOOL.md` section 6.6
Category:    supervise
Priority:    P1
Effort:      M
Status:      partial 2026-09-09

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

**Partial, 2026-09-09. Implemented, and its clause has not run.**

⭐ **The mechanism.** A launcher holds an exclusive `flock` on its container's
own lock file for its whole life, so `table::reconcile` decides whether a
launcher is gone by TRYING TO TAKE IT: a lock this process can take is a launcher
that is not there. ⛔ Never by pid, and never with a start-time check beside it: a
pid is reused and the check that clears a stale pid file is the race being
closed. Such a container becomes `dead`, with the time it was NOTICED and ⛔ **no
exit code at all**; `podbox wait` on it refuses rather than printing a number,
and `inspect --format '{{.ExitCode}}'` prints a dash.

⛔ **Clause 2 of `experiments/230-lifecycle-loop.sh` drives exactly this and has
not run**, because the loop fails before reaching it ([T-0602](supervise.md)).
Until it does, this is a mechanism that is written down and not one that has been
measured.

---

### T-0605 Capture logs at spawn, from the descriptors opened in step 2

Source:      `TOOL.md` section 6.6, section 6.5
Category:    supervise
Priority:    P2
Effort:      S
Status:      partial 2026-09-09

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

**Partial, 2026-09-09. Implemented, and its clause has not run.**

The log sink is opened in the launcher BEFORE anything changes root, and its
descriptors are handed to the payload as fds 1 and 2 through the same
`Plan.fds.pass` the entry sequence already had; stdin is `/dev/null`, opened in
the same place. `podbox logs` reads the file out of the store. ⛔ A sink opened
after the chroot cannot reach the store, which is outside the new root, and that
is step 2 of `TOOL.md` section 6.5 made in a different place.

⚠ `--log-driver` is not implemented at all rather than accepted and ignored, and
`attach` is a `None` row in the parity table for the same reason.

⛔ Clause 4 of `experiments/230-lifecycle-loop.sh` drives it and has not run.

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
Status:      partial 2026-09-09

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

**Partial, 2026-09-09. The loop exists and it does not pass twenty times.**

⭐ **This is the entry working, not the entry failing.** One pass proves nothing
about a race, and a loop that only ever ran once would have shipped this. Three
runs: 10 of 20, 9 of 20 and 1 of 3, every one of them failing at `stop`.
[T-0602](supervise.md) carries the readings and the two candidate causes.

⚠ **The script never retries and never continues past a failure**, so the number
it reports is how many iterations happened BEFORE the first one, which is the
only number this entry is about.
