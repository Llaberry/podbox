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
Status:      done 2026-09-08

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

**Done 2026-09-08.** The `Prove` command was run and exits 0: **48 probes**, and
no probe reports a verdict other than `ok` without an errno beside it.
`crates/podbox-probe/src/probes.rs` is the set and
`crates/podbox-probe/src/child.rs` is the child.

⭐ **The fork is `clone(2)`, not `std::process::Command`**, because the minimum
set needs `clone(CLONE_NEWNS)` and `clone(CLONE_NEWUSER)` attempted *with those
flags* and the standard library cannot set them. One spawn path serves both
kinds of probe: a `Kind::Clone` row's verdict is the clone's own errno, and a
`Kind::Child` row re-executes `/proc/self/exe --probe-child <name>` with the
namespace flags that row needs.

⭐ **The multithreaded rule is checked, not assumed.**
`crates/podbox-probe/src/probes.rs`'s `unshare(CLONE_NEWUSER)` body reads
`/proc/self/status`'s `Threads:` line first and reports a `skip` where it is
above one, because the kernel refuses that call to any multithreaded caller
with `EINVAL` whatever the policy. That is what makes the rule survive the day
somebody adds a thread pool rather than being a comment nobody re-reads.

⭐ **The declared syscall numbers were checked against the kernel's own
headers**, because a wrong one would report a verdict for a syscall nobody
named and would look exactly like a measurement. The command is one `gcc` away
and is reproducible:

```sh
printf '#include <sys/syscall.h>\n#include <stdio.h>\nint main(void){printf("%%d\\n",SYS_move_mount);}\n' \
  | gcc -x c - -o /tmp/nr && /tmp/nr   # against `crates/podbox-probe/src/sys.rs`
```

All 46 declared numbers and all 19 flag constants matched on 2026-09-08. Two
tests in `crates/podbox-probe/src/sys.rs` additionally cross-check the trap, the
ABI and the negative-return window against facts the process already knows by
another route, and the sixteen attribution numbers are exercised on every run of
`experiments/130-probe-parity.sh`.

⚠ **The verdict channel survives a caller with fd 1 closed.** `pipe2(2)` hands
out the lowest free descriptors, so with stdout closed one end of the pipe lands
on fd 1, where `dup2(1, 1)` is a no-op that returns without closing and an
unconditional `close` after it would shut the channel every child answers
through. Driven on 2026-09-08 with `podbox probe 1>&- 2>err`: the evidence block
is identical to an ordinary run, two skips either way.

⚠ **The errno comes from the raw syscall, not from a libc wrapper.**
`crates/podbox-probe/src/sys.rs` issues `syscall` directly, so the value in
`-4095..=-1` *is* `-errno` with no thread-local in between. glibc's `setuid(3)`
runs a multi-threaded id-change dance and musl's runs another; either answer
would be the library's rather than the kernel's.

⚠ **Four probes of the Go instrument were changed on purpose**, and the module
header of `crates/podbox-probe/src/probes.rs` carries all four with the reason.
The one worth naming here: the bare `mount(MS_SLAVE,/)` row is not ported,
because in the caller's mount namespace it changes the propagation of `/` for
the machine, permanently. The same operation is measured inside
`clone(CLONE_NEWNS)`, which is also where bubblewrap performs it.

---

### T-0102 Separate a filtered syscall from an executed one with a bogus argument

Source:      `TOOL.md` section 2.1 and section 6.1 rule 7, `paper_final.md` section 10.2 rule 7
Category:    probe
Priority:    P0
Effort:      S
Status:      done 2026-09-08

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
Prove:       `./experiments/30-attribution-census.sh` exits 0 or 2, never 1, and `podbox probe --json | jq -e '.controls.pidfd_getfd == "EBADF" and (.controls.kcmp == "ESRCH" or (.controls.kcmp == null and .controls_answered == false))'`

**Done 2026-09-08.** `./experiments/30-attribution-census.sh` exits **2** (34 ok,
0 failed, 3 skipped: the Landlock rows this kernel cannot run), and the `jq`
above exits 0. `crates/podbox-probe/src/select.rs`'s `Selection::controls`
evaluates the controls in the same run as the rows they qualify.

⛔ **The `Prove` line above was amended, and the amendment is the finding.** It
read `.controls.kcmp == "ESRCH"`, which asserts one host's answer as a property
of every host. `kcmp(2)` exists only where the kernel was built with
`CONFIG_CHECKPOINT_RESTORE`. Two readings settle it and they disagree with each
other, which is the point:

| where | `kcmp(-1,-1,...)` | source |
| --- | --- | --- |
| the target | `errno=3 ESRCH`, executed | `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26` |
| the host this repository is worked on | `errno=38 ENOSYS` | `experiments/results/attribute.txt`, and the corpus' own capture agrees |

So the entry's own premise holds on the target and the `Prove` did not. ⭐ The
rule underneath it is unchanged and is what podbox implements: **a control that
cannot answer must say so rather than letting the rows beside it read as
attributions.** `ENOSYS` is the kernel saying the control is absent, which is
not the control answering, so podbox records it as `skip` and sets
`controls_answered` false. `podbox probe`'s evidence block then says, in as many
words, that the mechanism attributions beneath it are unconfirmed.

⚠ **`controls_answered` does not move the exit code**, and that is deliberate.
Folding it into `--strict` would fail every kernel without
`CONFIG_CHECKPOINT_RESTORE` where podbox is otherwise fully capable, and one
exit code cannot then carry two different statements.
`crates/podbox-probe/src/select.rs`'s `Selection::exit_code` says so at the site.

⚠ A control that answers the **wrong** errno is caught as well as an absent one:
that is the subtler failure, and it is the one where the rows around it silently
stop being attributions.

---

### T-0103 Probe creation and attachment separately

Source:      `TOOL.md` section 6.1 rule 8, `paper_final.md` section 9.4 and F13
Category:    probe
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** The `Prove` command was run and exits 0.
`crates/podbox-probe/src/mounts.rs` derives the four from the attribution rows
that already ran, rather than issuing a second set of calls that would answer
about a different moment.

⭐ **The `may_mount()` sentence is in the output**, not only in this entry:
where `fsmount` succeeded and the attach did not, the evidence block prints that
no capability will clear the denial. That is the line that stops the next reader
hunting for one.

⚠ On the reconstruction of 2026-09-08 all four read `ok`, because this host has
no Landlock and `move_mount` therefore attaches. On the target the attach half
is `EPERM` from the LSM. The reading is the machine's, and the four-way shape is
what makes the difference legible instead of collapsing to `mounts: no`.

---

### T-0104 Probe the write allowlist by writing, for blocks and inodes

Source:      `TOOL.md` section 6.1 rule 10 and section 2.0
Category:    probe
Priority:    P0
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** The `Prove` command was run and exits 0.
`crates/podbox-probe/src/writable.rs` creates a real file `O_EXCL`, writes bytes
to it, removes it, then `statfs(2)`s the directory for blocks **and** inodes.

⭐ **The set reported is the set obtained.** Every candidate carries its own
verdict and errno, and `writable: true` marks the ones that answered. Six of
eight on this host unconfined; four of eight inside the reconstruction, and
those four are `/tmp`, `/dev/shm`, `/workspace` and `/state`, obtained rather
than assumed. `/var/tmp`, `/run` and `$HOME` are `skip (ENOENT)` there, which is the
absence of a directory and not a denial.

⚠ The candidates are the fixed list plus `$PODBOX_STORE`, `$XDG_RUNTIME_DIR`,
`$XDG_DATA_HOME`, `$TMPDIR`, `$HOME` and the working directory, and each row
says which named it. A path is probed because something would use it, not
because a specification listed it.

---

### T-0105 Read the ID maps directly rather than inferring them

Source:      `TOOL.md` section 6.1 rule 6, `paper_final.md` section 3.1
Category:    probe
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** The `Prove` command was run and exits 0.
`crates/podbox-probe/src/identity.rs` reads the three files and selects the
`/proc/self/status` lines **by name**, never by position.

⭐ **Two sentences in the diagnostic are the whole reason for the entry**, and
both fired inside the reconstruction: that a single-range uid map is what
explains every `EINVAL` from `chown(2)` and `setuid(2)`, and that gid 65534 is
`overflowgid` rather than a supplementary group. Reading
`/proc/self/uid_map: 0 1000 1` beside `groups=[0, 65534]` is what turns two
confusing errnos into one explanation.

⚠ A file that could not be read is `null` with the reason in
`.identity.unreadable`, never an empty string that would read like a value.

---

### T-0106 The `mknod` pair, because one of them tests nothing

Source:      `TOOL.md` section 6.1 rule 4, `paper_final.md` section 3.5 and F11
Category:    probe
Priority:    P2
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** The `Prove` command was run and exits 0. Both rows are in
the reported output and neither is collapsed.

⭐ **The pair did its job inside the reconstruction on the first run**:
`mknod(chr 1:3 /tmp/nodprobe)` answers `EPERM` and `mknod(chr 0:0 = whiteout)`
answers `OK`, at the same path in the same directory. No path-scoped policy can
tell two device numbers apart at one path, so the denial is capability-based.
That is one syscall's worth of extra effort for a mechanism attribution.

⚠ Neither row deletes a file that is already at its path. An existing
`/tmp/nodprobe` is a `skip` naming the file, because an errno about a node this
probe created says nothing about the one that was there.

---

### T-0107 Mode selection, and one switch that turns every degradation into a refusal

Source:      `TOOL.md` section 4.1, section 6.1; `paper_final.md` section 10.1
Category:    probe
Priority:    P0
Effort:      M
Status:      partial 2026-09-08

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

**Partial, 2026-09-08.** The selection and the switch are implemented and
measured; the first half of the `Prove` above cannot be earned at this
milestone and is not claimed.

**What holds now.** `crates/podbox-probe/src/select.rs` derives the rung from
the probe rows and nothing else. Measured on 2026-09-08: `namespace` unconfined
and `chroot` inside `./experiments/20-enter-target.sh`, which is
[T-1101](milestones.md)'s acceptance. `podbox probe --strict` exits **1** inside
the reconstruction and **0** unconfined, and `Rung`'s ordering is what enforces
that a weaker mode never satisfies a stronger request.

⭐ **`supervise` is refused on this runtime for a reason `TOOL.md` section 4.1
does not carry**, and the refusal is at the tier rather than per call:
[T-0606](supervise.md) establishes from `references/multikernel__sandlock` issue
#27 that a race-safe `Continue` needs `PTRACE_SEIZE` on every thread of every
process in the sandbox, and `ptrace` is filtered here. So the third leg is
`ptrace(PTRACE_TRACEME)`, and the reconstruction is rejected on it with the
errno printed.

⛔ **What is NOT done, and it is not out of scope.** The `podbox run` half needs
`run`, which is [T-1104](milestones.md), M3. ⚠ Running it today exits 125
because every verb but `probe` is unimplemented, so the command would *pass
vacuously*; that is not evidence and is not recorded as any. This entry closes
when M3 lands and the first half is run against a real `run`.

---

### T-0108 The mode banner

Source:      `TOOL.md` section 6.1 and section 6.8, `paper_final.md` section 10.1 and section 10.8
Category:    probe
Priority:    P0
Effort:      S
Status:      partial 2026-09-08

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

**Partial, 2026-09-08.** The banner exists, is on stderr, and is derived; the
`Prove` above needs `run` and is not claimed.

**What holds now.** `crates/podbox-probe/src/report.rs` emits it, and
`podbox probe` prints it on stderr while stdout carries the rung alone. Inside
the reconstruction it reads:

```
podbox 0.1.0: mode=chroot (namespaces: uts-only; mounts: none; devices: shimmed;
ownership: virtualized+sidecar; network: host-shared; pids: host-shared)
probes: clone(NEWNS)=ok mount=EPERM fsmount=ok move_mount=ok
        clone(NEWUTS)=ok sethostname=ok chroot=ok ptrace=EPERM
this mode does NOT provide: process, network, IPC or mount isolation
```

⭐ **Every field is derived from a row that ran**, which is why
`clone(CLONE_NEWUTS)` and `sethostname(in NEWUTS)` were added to the probe set:
the specification's banner names `sethostname=ok` and the Go instrument has no
such row, so `namespaces: uts-only` would otherwise have been an assertion.
`move_mount=ok` where the specification's example shows `EPERM` is the measured
difference: this reconstruction has no Landlock.

⚠ **`mounts:` and `network:` state what the RUNG provides, not what the machine
permits.** On 2026-09-08 the reconstruction attaches mounts and the machine's
own four verdicts all read `ok`, and the banner still says `mounts: none`,
because the `chroot` rung mounts nothing. The machine's reading is in the
evidence block underneath, where it is a measurement rather than a claim about
the mode.

⛔ **What is NOT done, and it is not out of scope.** "Once per `run` and `exec`"
needs `run` and `exec`, which are [T-1104](milestones.md), M3. The config switch
that suppresses it needs the configuration surface of [cli.md](cli.md).

---

### T-0109 A verdict is the operation's, and "could not run" never reads as "denied"

Source:      `TOOL.md` section 6.1 rule 9, `paper_final.md` section 3.7 and F17
Category:    probe
Priority:    P0
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** Both were run: the census exits **2**, never 1, and the
`jq` exits 0.

⭐ **Rule 1 is enforced by the parent, not by review.** A probe child writes its
verdict on stdout *and* sets its exit status from the operation, and
`crates/podbox-probe/src/child.rs`'s `interpret` refuses the pair when they
disagree. A child that contradicts itself has established nothing, so that is a
`skip`. A child that says nothing, or dies on a signal, is the same. There is no
path by which an exit code alone becomes a verdict.

⭐ **Rule 2 fired four times on the first real runs**, which is what a third
state is for: the missing squash fixture, `kcmp(2)` absent from this kernel,
`/mnt` absent (before the mount probe was corrected to read its own errno), and
`$HOME` absent inside the reconstruction. None of them printed as a denial and
none of them was counted as one when the rung was chosen; `podbox probe` lists
them under a heading that says so.

⭐ **Rule 3 is the type, not a convention.** `Verdict::exit_code` and
`Verdict::from_exit_code` are one another's inverse, so 0, 1 and 2 mean the same
thing in the child, in the parent and in every script in `experiments/`.

⚠ The corrected mount probe is worth naming: it used to `stat` `/mnt` first and
report a `skip` when it was absent. That is a precondition check in the wrong
place: a seccomp filter answers `EPERM` before the syscall body, so on the
reconstruction the policy would have gone unmeasured. It now runs the call and
separates the precondition from the **errno**: only `ENOENT` is a statement
about the target.

---

### T-0111 Cache the probe result, and key it on what actually decides it

Source:      `TOOL.md` section 6.1, last paragraph
Category:    probe
Priority:    P2
Effort:      S
Status:      open

Problem:     The probe forks 50 children, and everything downstream branches on
             its answer. Re-running the whole set for every `run` and every
             `exec` is the cost `TOOL.md` section 6.1 asks to avoid by caching.
             ⛔ **A cache keyed wrongly is worse than no cache**, and this is the
             one component where a stale answer is the exact failure the project
             exists to prevent: a `namespace` verdict served to a process that
             has a `chroot` is podbox telling the lie it was built to refuse.
Premise:     ⭐ **Measured, and it disagrees with the specification's key.**
             `TOOL.md` section 6.1 says to key on the boot id and re-probe when
             it changes. Read on 2026-09-08, one command in three places:

             ```
             host                     01703f0e-380a-462e-ab15-36460e5a8be7
             the reconstruction       01703f0e-380a-462e-ab15-36460e5a8be7
             a plain docker container 01703f0e-380a-462e-ab15-36460e5a8be7
             ```

             `/proc/sys/kernel/random/boot_id` is the **kernel's**, and every
             namespace on that kernel reads the same value. The three above
             produce three different probe answers: `namespace` on the host and
             `chroot` inside the reconstruction, measured by
             `experiments/130-probe-parity.sh`. So the boot id cannot tell them
             apart, and a cache keyed on it alone would serve the host's answer
             to a confined process on the first `run` after a probe.
Approach:    Key on the boot id **and** on what the probe measured about the
             confinement. ⚠ Three of the four below are already read by
             `crates/podbox-probe/src/identity.rs`; the fourth is not and is
             new work this entry carries:
             1. the boot id, which catches a reboot;
             2. `/proc/self/uid_map`, `/proc/self/gid_map` and
                `/proc/self/setgroups`, which `crates/podbox-probe/src/identity.rs`
                already reads and which change with the user namespace;
             3. `Seccomp` and `Seccomp_filters` from `/proc/self/status`, which
                change when a filter is installed;
             4. ⭐ **the mount namespace's inode**, `readlink("/proc/self/ns/mnt")`,
                which is what actually differs between the three readings above
                and is the one component **nothing in the tree reads today**.
                It is the whole reason this entry is not a two-line change.
             Write `$store/probe.json` as the existing `--json` document plus
             that key, and re-probe when any component differs.
             ⚠ The document is already versioned by `"podbox"`, and
             `crates/podbox-probe/src/report.rs` is the one writer. Adding a
             second serializer for the cache is the copy-paste this project's
             `docs/conventions/code.md` forbids: the cache stores what `--json`
             prints.
Decision:    Refuse to use a cache whose key does not match, rather than
             refreshing part of it. A partial refresh means two halves of one
             answer measured under two confinements, which is the class of
             defect [T-0109](#t-0109-a-verdict-is-the-operations-and-could-not-run-never-reads-as-denied)
             is about. ⚠ The alternative considered and rejected is no cache at
             all: 50 forks is small, but `run` and `exec` are the hot path and
             `TOOL.md` section 6.1 asks for it by name.
             ⛔ **Not blocked, and not startable either.** There is no `$store`
             until [T-1102](milestones.md), M1, so this lands beside the store
             and not before it.
Prove:       `podbox probe --json > /tmp/a.json && podbox probe --json > /tmp/b.json && jq -e --slurpfile a /tmp/a.json '.rung == $a[0].rung' /tmp/b.json` and, inside `./experiments/20-enter-target.sh`, a probe run after a host run selects `chroot` rather than reading the host's cached `namespace`

---

### T-0110 `podbox probe` exit-code and channel contract

Source:      `TOOL.md` section 5 M0; `references/compforge__pathshim/tree/README.md:65`
Category:    probe
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** Run inside `./experiments/20-enter-target.sh`, where the
selected rung is `chroot`: stdout is the single word `chroot` and the exit code
is 0. Unconfined the same command prints `namespace` and exits 0, which is the
other half of [T-1101](milestones.md)'s acceptance.

⭐ **Three stdout formats, one per flag, and the evidence never shares the
channel.** Bare prints the rung; `--json` prints one document; `--rows` prints
every probe row in the format `verification/probe` emits, which is what makes
the milestone's row-for-row comparison a mechanical diff rather than a reading.
`--json` and `--rows` together are refused with exit 2 rather than interleaved.

⚠ Invalid input exits **2**, adopting `pathshim`'s contract at
`references/compforge__pathshim/tree/README.md:65`. The divergence this entry
names is unchanged and is not yet reachable: podbox refuses rather than
degrading into a passthrough, and that decision lands with `run` at M3.
