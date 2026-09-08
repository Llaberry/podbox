# probe

`crates/podbox-probe`. `TOOL.md` section 6.1, milestone M0.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

Probing is where every prior account went wrong, and the corpus contains both
halves of the lesson: `pathshim` refuses a tier it cannot install and says why,
`sandlock` continues per call and reports success for mediation it never
performed. The rules below are each paid for by one of them.

⭐ **Two entries carry the whole discipline.** [T-0102](#t-0102-separate-a-filtered-syscall-from-an-executed-one-with-a-bogus-argument)
is the discriminator, and [T-0109](#t-0109-a-verdict-is-the-operations-and-could-not-run-never-reads-as-denied)
is the rule that a probe reports the verdict of the operation it names.

---

### T-0101 The probe set, one disposable child per probe, errno not boolean

Source:      `TOOL.md` section 6.1, `paper_final.md` section 10.2
Category:    probe
Priority:    P0
Effort:      M
Status:      open

Problem:     Without this nothing downstream can branch. Every mode the ladder
             offers has prerequisites that differ per machine, and a runtime
             that assumes any of them has hard-coded one machine.
Premise:     Read, not measured here. `TOOL.md` section 6.1 lists the minimum set;
             `paper_final.md` section 10.2 is the same list with its reasoning.
             `references/Azathothas__container-research/tree/verification/probe/`
             implements it in Go and is the reference, not a sketch.
Approach:    Port the set, do not redesign it. Each probe runs in a freshly
             forked child, because a successful `unshare`, `chroot` or `setuid`
             mutates the prober. Record the **errno**, never a boolean: `EINVAL`
             from `setuid` or `chown` means an unmapped id and points at a
             mapping; `EPERM` points at a policy, and they are different
             remedies.
             The disposable-child shape to copy is
             `references/compforge__pathshim/tree/src/main.rs:103-118`, which
             runs its probe as `Command::new("/proc/self/exe")` with a private
             argument and takes the verdict from the child's exit status.
             ⛔ Never probe `unshare(CLONE_NEWUSER)` from a multithreaded
             process: the kernel refuses it to any such caller with `EINVAL`
             regardless of policy. In Rust this is free, and the rule survives
             the day somebody adds a thread pool.
Decision:    Fork per probe rather than a thread per probe. A thread cannot be
             disposed of after `chroot` and `syscall(SYS_unshare)` mutates only
             the calling thread, which leaves the process half-transitioned.
             The alternative, running probes in-process and undoing them, loses
             because several of them have no undo.
Prove:       `podbox probe --json | jq -e '.probes | length >= 24 and (map(select(.errno == null and .verdict != "ok")) | length == 0)'`

---

### T-0102 Separate a filtered syscall from an executed one with a bogus argument

Source:      `TOOL.md` section 2.1 and section 6.1 rule 7, `paper_final.md` section 10.2 rule 7
Category:    probe
Priority:    P0
Effort:      S
Status:      open

Problem:     Two mechanisms return the same errno for the same call. A seccomp
             filter and a path-scoped LSM both answer `EPERM`, and a runtime
             that cannot tell them apart reports the wrong remedy.
Premise:     Measured, in the reconstruction this repository seeds
             `experiments/` from. A filter sees the syscall number and six
             argument registers, cannot dereference a pointer, and runs before
             the syscall body. So an argument the kernel rejects *inside* the
             body separates them:

             | call | answer | meaning |
             | --- | --- | --- |
             | `mount(2)`, nonexistent target | `ENOENT` | the syscall executed |
             | `mount(2)`, nonexistent target | `EPERM` | a filter refused it |
             | `move_mount` to a nonexistent dest | `ENOENT` | executed; an `EPERM` elsewhere is an LSM |
             | `process_vm_readv` on pid 999999 | `ESRCH` | executed |
             | `process_vm_readv` on pid 999999 | `EPERM` | filtered |

Approach:    Implement the four rows above plus their controls. The controls
             are not optional: `pidfd_getfd(-1, -1)` must answer `EBADF` and
             `kcmp(-1, ...)` must answer `ESRCH`. A probe whose control has
             stopped answering has stopped discriminating and must say so
             rather than reporting a mechanism.
             The independent witness for the clone-versus-mount half is
             bubblewrap: `references/containers__bubblewrap/tree/bubblewrap.c:3034`
             sets `clone_flags = SIGCHLD | CLONE_NEWNS` unconditionally, and
             `references/containers__bubblewrap/tree/bubblewrap.c:3257-3258`
             is the `MS_SLAVE` remount that dies with `Failed to make / slave`.
             Reaching that message proves the clone succeeded and the mount did
             not.
Decision:    Carry the controls in the same probe run rather than in a separate
             self-test. A control that runs at a different time answers about a
             different machine state.
Prove:       `./experiments/30-attribution-census.sh` exits 0 or 2, never 1, and `podbox probe --json | jq -e '.controls.pidfd_getfd == "EBADF" and .controls.kcmp == "ESRCH"'`

---

### T-0103 Probe creation and attachment separately

Source:      `TOOL.md` section 6.1 rule 8, `paper_final.md` section 9.4 and F13
Category:    probe
Priority:    P1
Effort:      S
Status:      open

Problem:     A runtime that asks only "does `mount(2)` work" learns less than
             one syscall's worth more effort would have told it, and then
             reports "no mounts" where the truth is "mounts you can create and
             never attach".
Premise:     Measured on the target, one session, and half of it reproduced in
             the reconstruction. `fsopen`, `fsconfig(CMD_CREATE)`, `fsmount`,
             `open_tree(OPEN_TREE_CLONE)` and `mount_setattr` succeed;
             `move_mount` is denied inside the kernel; `openat` on a detached
             mount fd answers `EACCES`.
Approach:    Probe the create half, the configure half and the attach half as
             three separate verdicts, and probe **use** as a fourth: a detached
             mount that cannot be opened through is not a usable private
             filesystem, so a runtime that stops at "creatable" reports a
             capability it does not have.
             ⭐ `fsmount` succeeding proves `may_mount()` passes, and therefore
             that no mount denial on that machine is a capability problem. Put
             that in the diagnostic: it stops the next reader looking for a
             capability to acquire.
Decision:    Report four verdicts rather than one `mounts: no`. The four-way
             answer is what makes the diagnostic actionable and it costs three
             extra syscalls.
Prove:       `podbox probe --json | jq -e '.mounts | has("create") and has("configure") and has("attach") and has("use")'`

---

### T-0104 Probe the write allowlist by writing, for blocks and inodes

Source:      `TOOL.md` section 6.1 rule 10 and section 2.0
Category:    probe
Priority:    P0
Effort:      S
Status:      open

Problem:     `{/tmp, /dev/shm, /workspace, /state}` is one machine's allowlist.
             A runtime that hard-codes it has hard-coded that machine, and on
             the machine it was written for `/tmp` is 64 MiB.
Premise:     Read, and partly measured. `TOOL.md` section 0 says the allowlist has
             never been verified anywhere a reader can re-run, and instructs
             that it be probed rather than assumed. The capacity half is
             separate and belongs to T-0203.
Approach:    For each directory the workload needs, attempt a real write and
             record the errno, then `statvfs` it for **blocks and inodes both**.
             Report the set obtained, not the set expected. An `EACCES` where
             the directory's owner is mapped is a path-scoped policy; an
             `EACCES` where it is unmapped is the user namespace, and
             `/proc/self/uid_map` (T-0105) tells them apart in one read.
Decision:    Write a real file and remove it, rather than `access(W_OK)`.
             `access` consults the capability set, which reports every bit here
             and answers the wrong question.
Prove:       `podbox probe --json | jq -e '.writable | type == "array" and length > 0 and all(has("path") and has("blocks_free") and has("inodes_free"))'`

---

### T-0105 Read the ID maps directly rather than inferring them

Source:      `TOOL.md` section 6.1 rule 6, `paper_final.md` section 3.1
Category:    probe
Priority:    P1
Effort:      S
Status:      open

Problem:     A dozen probes infer what one read answers, and the inference is
             wrong in the case that matters: `groups=0,65534` looks like a
             supplementary group and is `overflowgid`, which is what
             `getgroups(2)` returns for a group with no mapping.
Premise:     Measured on the target. `/proc/self/uid_map` reads `0 1000 1`,
             `/proc/self/gid_map` the same, `/proc/self/setgroups` reads `deny`.
Approach:    Read all three, plus `/proc/self/status` for `CapEff` and
             `Seccomp`, and put the contents in the diagnostic. A single-entry
             map is the fact that explains every `EINVAL` from `chown` and
             `setuid`, and quoting it turns a confusing errno into an
             explanation.
             ⚠ A full `CapEff` proves nothing on its own: a process that is
             root in a new user namespace receives the complete set by
             construction, whatever its parent held.
Decision:    Read the maps even where every probe already ran. They cost one
             `open` each and they are what the section 6.9 diagnostic quotes.
Prove:       `podbox probe --json | jq -e '.identity.uid_map != null and .identity.setgroups != null and .identity.cap_eff != null'`

---

### T-0106 The `mknod` pair, because one of them tests nothing

Source:      `TOOL.md` section 6.1 rule 4, `paper_final.md` section 3.5 and F11
Category:    probe
Priority:    P2
Effort:      S
Status:      open

Problem:     `mknod(path, S_IFCHR|0600, 0)` is in common use as a `CAP_MKNOD`
             probe and it tests nothing: `makedev(0,0)` is `WHITEOUT_DEV` and
             `vfs_mknod` exempts whiteouts from the capability check.
Premise:     Measured. Under a user namespace with a partial map the whiteout
             succeeds while `mknod(S_IFCHR, makedev(1,3))` returns `EPERM`.
Approach:    Run **both**, in the same directory, and report the pair. A
             whiteout that succeeds where a real device number fails proves the
             denial is capability-based and not path-based, because no
             path-scoped policy can tell two device numbers apart at one path.
             That is a free mechanism attribution and it costs one syscall.
Decision:    Keep both rows in the reported output rather than collapsing them
             to `mknod: denied`. The pair is the evidence; the collapse throws
             away the attribution that makes it useful.
Prove:       `podbox probe --json | jq -e '.probes | map(select(.name | startswith("mknod"))) | length == 2'`

---

### T-0107 Mode selection, and one switch that turns every degradation into a refusal

Source:      `TOOL.md` section 4.1, section 6.1; `paper_final.md` section 10.1
Category:    probe
Priority:    P0
Effort:      M
Status:      open

Problem:     A weaker mode must never satisfy a stronger request. A user who
             believes they have namespaces when they have a `chroot` is worse
             off than one who is told the truth, and an automated user cannot
             notice the difference the way a human skimming a log might.
Premise:     Read in the specification, and both halves are measured in the
             corpus. `ruri` degrades per operation and refuses on one named
             floor; `udocker` selects a mode per container and persists it.
Approach:    Derive the rung from the probe results, never from a privilege
             that usually implies it. Two mechanisms transfer:

             1. ⭐ `references/RuriOSS__ruri/tree/src/include/ruri.h:237-248`.
                `ruri_warn_on_error(ret, expect, show, fmt, ...)` is non-fatal
                by default and, under `ruri_flag(force_panic)`, warns and then
                exits at the same site. **One switch converts every degradation
                into a refusal.** podbox needs that switch because its audience
                is automated and has to be able to demand strictness.
                The floor beside it is
                `references/RuriOSS__ruri/tree/src/unshare.c:53-54`, the single
                namespace `ruri` refuses to degrade on, against
                `references/RuriOSS__ruri/tree/src/unshare.c:55-77` where five
                others warn and continue.
             2. `references/indigo-dc__udocker/tree/udocker/engine/execmode.py:43-56`.
                `get_mode()`'s precedence is force, then the per-container file,
                then a config override, then a per-architecture default, then
                `DEFAULT`. Persisting the selection per container is what
                `TOOL.md` section 4.1 asks for at container granularity.

             ⚠ `ruri`'s `show` argument is `!ruri_flag(disable_warnings)`, so a
             flag can silence its degradation notices. podbox suppresses the
             banner by config and never by default, because a silenced
             degradation is `sandlock`'s failure mode with a flag in front of
             it.
Decision:    Take `ruri`'s inversion and `udocker`'s precedence chain, and take
             neither project's architecture. The alternative considered and
             rejected is a single global strictness level: it cannot express
             "this container asked for isolation and that one did not", which
             is the distinction the ladder exists for.
Prove:       `podbox run --network=none --rm alpine true; test $? -ne 0` and `podbox probe --strict` exits non-zero on any degraded rung

---

### T-0108 The mode banner

Source:      `TOOL.md` section 6.1 and section 6.8, `paper_final.md` section 10.1 and section 10.8
Category:    probe
Priority:    P0
Effort:      S
Status:      open

Problem:     An agent cannot notice that its "container" was a `chroot`. The
             banner is the product, not a disclaimer.
Premise:     Read. `TOOL.md` section 6.1 gives the exact shape, on stderr, once per
             `run` and `exec`, suppressible by config and never by default.
Approach:    Print the achieved mode, what it does not provide, and the probe
             verdicts that decided it:

             ```
             podbox 0.1: mode=chroot (namespaces: uts-only; mounts: none; devices: shimmed;
             ownership: virtualized+sidecar; network: host-shared; pids: host-shared)
             probes: clone(NEWNS)=ok mount=EPERM fsmount=ok move_mount=EPERM
                     clone(NEWUTS)=ok sethostname=ok chroot=ok ptrace=EPERM
             ```

             ⛔ No output may imply namespaces, cgroups or devices exist when
             they do not.
Decision:    stderr, not stdout. A payload's stdout is data to whatever
             consumes it, and a banner on it corrupts every pipeline. The same
             split `pathshim` uses at
             `references/compforge__pathshim/tree/src/main.rs:134-138`.
Prove:       `podbox run --rm alpine:latest /bin/echo hi 2>banner.txt >out.txt && grep -q '^hi$' out.txt && grep -q 'mode=' banner.txt`

---

### T-0109 A verdict is the operation's, and "could not run" never reads as "denied"

Source:      `TOOL.md` section 6.1 rule 9, `paper_final.md` section 3.7 and F17
Category:    probe
Priority:    P0
Effort:      S
Status:      open

Problem:     Both mistakes were live in the research harness this project
             inherits its `experiments/` from, and both produced a published
             row that was wrong: a mount row reported the child's exit code,
             and the child exited 0 whether the mount failed or not; and a probe
             whose fixture was missing reported the resulting `ENOENT` as a
             denial.
Premise:     Measured, and recorded in the reference itself.
             `references/Azathothas__container-research/tree/verification/README.md:91-107`
             carries all three verdict bugs and their fixes.
Approach:    Three rules in the probe type, not in review:

             1. A child sets its exit status **from the operation** or the
                parent does not read a verdict from it.
             2. A missing precondition is a third state, `skip`, with the reason,
                and it is never rendered as a denial.
             3. The process-level exit code distinguishes them: **0** ran and
                matched, **1** ran and something failed, **2** could not run.
                That is the contract every script in `experiments/` already
                holds to.
Decision:    Model the verdict as a three-variant enum rather than a boolean
             plus a comment. A boolean cannot carry "not measured", and every
             instance of this defect in the corpus is a boolean that had to.
Prove:       `./experiments/30-attribution-census.sh; test $? -ne 1` and `podbox probe --json | jq -e '[.probes[].verdict] | all(. == "ok" or . == "denied" or . == "skip")'`

---

### T-0110 `podbox probe` exit-code and channel contract

Source:      `TOOL.md` section 5 M0; `references/compforge__pathshim/tree/README.md:65`
Category:    probe
Priority:    P1
Effort:      S
Status:      open

Problem:     A probe whose result has to be parsed out of prose is not a gate.
             The audience is automated and needs one line on stdout and an exit
             code it can branch on.
Premise:     Measured in the corpus, at file and line. `pathshim` prints the
             machine-readable verdict on **stdout**, the reason on **stderr**,
             and exits 1 when a normal invocation would degrade:
             `references/compforge__pathshim/tree/src/main.rs:134-138`. Its
             README states the contract at
             `references/compforge__pathshim/tree/README.md:65`: `bind-view`
             and 0, `passthrough` and 1, invalid input and 2.
Approach:    `podbox probe` prints the selected rung on stdout, the evidence on
             stderr, and exits 0. `podbox probe --strict` exits non-zero when
             the selected rung is below `namespace`, so a caller can gate on it
             without parsing.
             ⚠ podbox diverges from `pathshim` in one place and it is
             deliberate: `pathshim`'s `run` degrades to an unmapped passthrough
             and executes the command anyway
             (`references/compforge__pathshim/tree/src/main.rs:160-172`). That
             is honest and it still lets a weaker mode satisfy the request.
             podbox refuses instead, because a `-v` that silently became a
             passthrough shows the payload the host tree.
Decision:    Add `--json` beside the one-word stdout line rather than replacing
             it. The word is what a shell branches on; the JSON is what a
             harness diffs against `experiments/results/`.
Prove:       `test "$(podbox probe)" = chroot && podbox probe >/dev/null; test $? -eq 0`
