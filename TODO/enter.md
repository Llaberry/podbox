# enter

`crates/podbox-enter`. `TOOL.md` section 6.5, milestone M3.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

Five steps, and step 2 is the one that gets skipped.

```
1. resolve the rootfs path; refuse it if it is a symlink
2. open every fd the child needs WHILE THE OUTER ROOT IS STILL CURRENT
3. chroot(rootfs); chdir("/")
4. resolve the program path, now, inside the new root, in this process
5. exec
```

---

### T-0501 Open every descriptor before the root changes

Source:      `TOOL.md` section 6.5, `paper_final.md` section 10.5
Category:    enter
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     A chroot cuts off every path outside the new root. A child with no
             explicit stdio opens `/dev/null`, which an extracted rootfs does
             not have, and the failure names a missing file rather than a
             missing step.
Premise:     Read, and the same trap is measured from the other side by T-0401:
             the shell that redirects into a non-existent `/dev/null` creates a
             growing file.
Approach:    Open stdio, the log sinks, any host device the config exposes via
             `--device`, and a PTY pair if `-t` was asked for and T-0503 says it
             is available, **before** `chroot`. A descriptor opened before the
             change keeps working after it.
             ⭐ This is the only route by which a PTY can reach a chrooted
             payload at all: allocate the pair outside, pass the descriptors in,
             `TIOCSCTTY` in the child.
Decision:    Pass descriptors rather than bind-mounting anything. There is no
             attach path on this runtime, so a descriptor is the only thing that
             crosses the boundary.
Prove:       `podbox run --rm --device /dev/urandom alpine:latest sh -c 'head -c4 /dev/urandom | wc -c' | grep -qx 4`

**Done 2026-09-09.** `crates/podbox-enter/src/lib.rs`. Every buffer the child
touches after the `chroot`, the argv, the environment, the working directory and
the candidate program paths, is built **before** the fork, because between
`clone` and `execve` only async-signal-safe work is permitted and nothing there
may allocate.

⚠ **stdio is inherited rather than re-opened, deliberately**, and that is what
makes `podbox run <image> cmd | consumer` give the consumer the payload's bytes.
The `/dev/null` trap this entry names is real and is why podbox never *closes*
stdio: a child with none opens `/dev/null`, which an extracted rootfs does not
have. ⛔ `--device` and the PTY pair are **not implemented**; they are
[T-0503](#t-0503-probe-devptmx-and-refuse--t-by-name-where-it-is-absent)'s and
M4's, and `-t` is refused by name rather than degraded.

---

### T-0502 Resolve the program inside the new root, in the process that changed it

Source:      `TOOL.md` section 6.5; `paper_final.md` section 5.5
Category:    enter
Priority:    P0
Effort:      S
Status:      done 2026-09-09

Problem:     A prior implementation resolved the program in a child whose
             environment had been replaced. The child recomputed its store path
             from an empty environment, got a **relative** path, created a fresh
             empty rootfs under its working directory, chrooted into **that**,
             and reported `stat /bin/sh: no such file or directory` from a
             rootfs where that path exists.
Premise:     Read, in the account of a shipped bug. The general shape is in the
             corpus too: `references/89luca89__lilipod` is the Go implementation
             the account is about, and it is refused as a seed on language
             grounds and on licence grounds ([reference-map.md](reference-map.md)).
Approach:    Resolve the program path **after** `chroot` and `chdir("/")`, in
             the process that did them, never in the parent and never at command
             construction time. Do not strip the environment the child needs to
             find the store.
             ⚠ A relative store path is the specific hazard. Compute every store
             path from an absolute root captured before any environment change,
             and assert it is absolute.
Decision:    Resolve in the child rather than pass a pre-resolved absolute path
             from the parent. A parent-resolved path names the outer tree, and
             the outer tree is gone.
Prove:       `env -i "$(command -v podbox)" run --rm alpine:latest /bin/sh -c 'echo ok' | grep -qx ok`

**Done 2026-09-09.** `podbox_enter::run` resolves the program **after**
`fchdir`, `chroot(".")` and `chdir("/")`, in the process that did them.
`Plan::program_candidates` builds the list before the fork and the child tries
each with `execve`; nothing is resolved in the parent.

⭐ Driven: `podbox run <image> sh -c 'echo onpath'` finds `sh` along the image's
own `PATH` **inside the new root**, and `experiments/300-run.sh` clause 4 asserts
it. A parent-resolved path would have named the outer tree, which is gone.

⚠ The relative-store hazard this entry names is closed from the other side too:
`Store::default_root` is computed before any environment change, and clause 7 of
`300-run.sh` runs with `PODBOX_STORE` set to a path inside `/workspace` and
enters a rootfs under it.

---

### T-0503 Probe `/dev/ptmx`, and refuse `-t` by name where it is absent

Source:      `TOOL.md` section 0, section 6.5, section 6.8; `paper_final.md` section 10.5
Category:    enter
Priority:    P1
Effort:      S
Status:      partial 2026-09-08

Problem:     `-t` either works or it does not, and the corpus disagrees with
             itself about which. A degraded PTY that cannot open a terminal is
             worse than a refusal, because the payload waits.
Premise:     ⚠ **Unresolved, and stated as unresolved.** The target's mount
             table shows exactly six device nodes bind-mounted into `/dev`
             (`full`, `null`, `random`, `tty`, `urandom`, `zero`), no `ptmx` and
             no `devpts`, and `mknod` cannot create one. An earlier account of
             the same runtime asserts the host `/dev/ptmx` works and published no
             capture. A mount table does not list plain files, so neither source
             settles it. One `stat("/dev/ptmx")` on the target would, and nobody
             has run one.
Approach:    `stat("/dev/ptmx")` in the **outer** environment, before the chroot,
             as part of T-0101's probe set. If present, allocate the pair
             outside and pass the descriptors in per T-0501. If absent, `-t` is a
             **named-reason refusal**: `-t requires /dev/ptmx in the outer
             environment; it is absent`.
             ⚠ A payload that reopens `/dev/pts/N` by name is unsupported either
             way, and the banner says so rather than letting it fail deeper.
Decision:    Refuse rather than degrade. `-t` is a request for a terminal, and a
             terminal that is not there cannot be approximated by a pipe without
             changing what every interactive program does.
Prove:       `podbox probe --json | jq -e 'has("ptmx")' && { podbox run --rm -t alpine:latest true || podbox run --rm -t alpine:latest true 2>&1 | grep -q 'ptmx'; }`

**Partial, 2026-09-08.** The probe half is implemented and measured; the refusal
half needs `run`, which is M3.

**What holds now.** Two rows in T-0101's set, in the outer environment, before
any chroot: `stat(/dev/ptmx)` and `open(/dev/ptmx, O_RDWR)`. The first half of
the `Prove` runs and exits 0. Readings taken on 2026-09-08:

| where | `.ptmx` |
| --- | --- |
| this host, unconfined | `present: true, usable: true`, chardev 5:2 mode 666 |
| inside `experiments/20-enter-target.sh` | `present: false, usable: false`, `ENOENT` both |

⭐ **Existence is not function, so there are two rows and not one.** A device
node that stats and will not open is exactly the degraded `-t` this entry
refuses, and `.ptmx.usable` is the OPEN. ⚠ The open passes `O_NOCTTY`: without
it, opening a terminal can make it the prober's controlling terminal, which is
a mutation of the process doing the measuring.

⛔ **THE RECONSTRUCTION DOES NOT SETTLE THE PREMISE, AND MUST NOT BE READ AS
HAVING DONE SO.** It reproduces the target's `/dev` from the same mount table
this entry says cannot answer the question, so its `ENOENT` is that mount table
repeated back, not an independent measurement. ⭐ What HAS changed is that the
question is now one command on the machine that matters: `podbox probe --json |
jq .ptmx` on the target, by anyone who can reach it. Until somebody runs it
there, the premise stays unresolved and is stated as unresolved.

---

### T-0504 Refuse a rootfs path that is a symlink

Source:      `TOOL.md` section 6.5 step 1
Category:    enter
Priority:    P1
Effort:      S
Status:      done 2026-09-09

Problem:     `chroot` follows a symlink. A rootfs path that is one puts the
             payload somewhere the runtime did not choose, and every containment
             claim after that is about a different directory.
Premise:     Read.
Approach:    `lstat` the resolved rootfs path and refuse if it is a symlink,
             naming the path and its target. Then hold a directory fd on it and
             use `fchdir` plus `chroot(".")`, so nothing between the check and
             the change can swap the path.
Decision:    Refuse rather than resolve. Resolving accepts a rootfs the store
             does not own, and the store is where the ownership sidecar and the
             lock (T-0204) live.
Prove:       `ln -sfn "$(podbox inspect --format '{{.RootfsPath}}' alpine:latest)" /tmp/rootlink && ! podbox run --rm --rootfs /tmp/rootlink alpine:latest true`

**Done 2026-09-09.** `podbox_enter::RootDir::open` `lstat`s with
`AT_SYMLINK_NOFOLLOW`, refuses a symlink **naming its target**, refuses anything
that is not a directory, and then holds the directory open with
`O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC`. The entry uses `fchdir` on that descriptor
and `chroot(".")`, so nothing between the check and the change can swap the path.

⚠ Two tests, and the second is what makes the first mean something: a symlinked
rootfs is refused, **and the directory it points at is accepted**, so the check
is about the symlink rather than about the path being rejected wholesale.

---

### T-0505 `exec` is a fresh chroot, and `inspect` says so

Source:      `TOOL.md` section 4.2, section 6.8; `paper_final.md` section 10.6
Category:    enter
Priority:    P1
Effort:      S
Status:      done 2026-09-09

Problem:     `docker exec` enters the container's namespaces. podbox has none to
             enter, so its `exec` re-runs the section 6.5 sequence against the same
             rootfs. It shares the filesystem tree with the original process and
             nothing else, and a caller that assumes otherwise gets a process
             that cannot see the original's `/proc`, its signals or its
             environment.
Premise:     Read.
Approach:    Implement `exec` as a fresh entry, and mark it **Degraded** in the
             parity table with the difference stated: "a fresh chroot re-entry;
             shares only the filesystem". `inspect` carries the true mode per
             container so a caller can check rather than assume.
             ⛔ Do not imply containment. A pidfd addresses one process; it does
             not contain descendants and it is not a PID namespace, and
             grandchildren that reparent are outside podbox's reach.
Decision:    A fresh chroot rather than refusing `exec`. `exec` is load-bearing
             for M4's lifecycle test and for every agent workflow, and the
             filesystem-only sharing is enough for the overwhelming majority of
             uses provided it is stated.
Prove:       `./experiments/320-cli-contract.sh` clause 4 and `./experiments/300-run.sh` clause 8: exec's stdout is the payload's, the banner says it is a fresh chroot, and `inspect --format '{{.Exec.Shares}}'` reads `filesystem`

⛔ **The `Prove` above was rewritten, and the original is here because the
rewrite is the finding.** It read

```
podbox run -d --name execprobe alpine:latest sleep 30 && podbox exec execprobe ... && podbox rm -f execprobe
```

and every verb in it except `exec` is M4's: `run -d`, `--name` and `rm` need
container state, which [T-1105](milestones.md) creates. ⛔ The work order puts
this entry BEFORE M4 because M4 needs it, so a `Prove` that needs M4 could never
have run in the order it was written for. The mechanism does not depend on that
half: `exec` re-enters a rootfs, and today a rootfs is named by an image
reference and at M4 by a container name that resolves to the same directory.

**Done, 2026-09-09.** `crates/podbox-cli/src/exec.rs`, `Exec.Mode` and
`Exec.Shares` on `inspect`, and clause 8 of `experiments/300-run.sh`.

⭐ **The degradation is stated in three places and they cannot disagree**,
because all three read one pair of constants in `crates/podbox-cli/src/images.rs`:
the banner on every `exec`, `podbox inspect --format '{{.Exec.Mode}}'` for a
program, and the `Degraded` row for the verb in [T-0801](cli.md)'s parity table.
A unit test asserts the banner contains what `inspect` reports, and clause 8
asserts it on the shipped binary.

⛔ **`exec` never pulls and never extracts.** A verb that creates the thing it
claims to attach to is exactly the lie `TOOL.md` section 4.1 forbids, so an
image the store does not hold, or holds and has never extracted, is a refusal
naming `podbox run`. ⚠ It also has no default command: an image's `Cmd` is what
`run` starts, and re-running it from `exec` is a process nobody asked for.

⛔ **`--format` had to learn a dotted name for this.** `{{.Exec.Shares}}` was
refused as an unsupported traversal, because the walker rejected any `.` inside
a name. A dotted name is now ONE registered name, so an unregistered one is
still refused rather than resolving half of itself and rendering a blank.

---

### T-0506 A foreign-architecture container, and never a rung measured by the emulator

Source:      Found on 2026-09-09 by running `podbox probe` under `qemu-aarch64`, while closing [T-0911](deps.md)
Category:    enter
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     ⛔ **A probe run under `qemu-user` measures the emulator, and
             printing its rung as the machine's is the exact lie podbox exists
             to refuse.** Measured on 2026-09-09,
             `experiments/results/multiarch.txt` clause 4: `qemu-aarch64`
             answers `EINVAL` to `clone(CLONE_NEWNS)`, `ENOSYS` to `fsmount` and
             `move_mount`, and reports `Seccomp: 0` and `NoNewPrivs: 0` however
             the host is confined. Every one of those is a statement about QEMU.
             ⚠ It was harmless while podbox ran only where it was built. The
             multiarch work makes it reachable by an ordinary user: `podbox run`
             on a `linux/arm64` image on an amd64 host executes its payload,
             and any `podbox probe` inside it, under precisely this emulator.
Premise:     ⭐ **Measured, in this tree.** The aarch64 binary reports rung
             `namespace` under qemu on a host that is also `namespace`, so the
             two agree today and the agreement is a coincidence of this host
             rather than evidence. The rows that disagree are printed in the
             same result file, and they are the ones a caller would act on.
             ⚠ It is the same shape as `experiments/20-enter-target.sh`'s `/dev`
             answering out of the mount table the question doubts
             ([T-0503](#t-0503-probe-devptmx-and-refuse--t-by-name-where-it-is-absent)),
             and it has the same remedy: say which instrument answered.
Approach:    Read the image's platform and the host's, and where they differ:
             1. **Say so, once, on stderr**, naming both. ⛔ Never silently.
             2. Read `/proc/sys/fs/binfmt_misc` for an interpreter registered
                for the image's architecture, and whether its flags carry `F`.
                ⭐ Measured 2026-09-09, clause 5: an `F` registration holds the
                interpreter open, so a **bare chroot with no qemu inside it**
                runs the foreign binary. Without `F` the interpreter must be
                reachable by path inside the rootfs, which is the copy-in below.
             3. Where an interpreter is registered without `F`, copy the static
                interpreter into the rootfs, the operator's ruling of
                2026-09-09, and ⛔ **disclose the write**: podbox put a file in
                the payload's tree, which is a thing the payload can see, and
                the honesty rules do not have an exception for convenience.
             4. Where no interpreter is registered at all, **refuse** with exit
                125, naming the host platform, the image platform, whether
                `binfmt_misc` is mounted, and what would fix it. ⛔ Never exec
                into a bare `Exec format error`.
             5. ⛔ **Mark the probe answer as the emulator's** wherever podbox
                runs under one, in the evidence block and in `--json`, and
                refuse to write it into `$store/probe.json` under a key that
                does not name the emulator. [T-0111](probe.md)'s cache key is
                about confinement; this adds the interpreter.
Decision:    Report and refuse before falling back. A runtime that guesses which
             architecture a payload wanted has the same problem as one that
             guesses a rung. ⚠ The copy-in is deliberate and disclosed rather
             than refused, because refusing it would make podbox useless on
             every machine whose binfmt registration predates `F`, which is most
             of them.
Prove:       `podbox run --rm --platform linux/arm64 <image> /bin/true` succeeds where an interpreter is registered and exits 125 naming both platforms where none is, and `podbox probe --json` run inside it carries the interpreter in `measured_by`

**Done 2026-09-09.** `crates/podbox-enter/src/binfmt.rs` and the `run` verb,
driven by `experiments/300-run.sh` clause 5 and
`experiments/260-multiarch.sh` clause 5.

| | |
| --- | --- |
| `podbox run --platform linux/arm64 <image> /bin/uname -m`, on an amd64 host | **`aarch64`**, exit 0 |
| what podbox said on stderr | it named the image's platform, the host's, and the interpreter, and that the binfmt `F` flag makes it reachable inside the chroot |
| `--platform linux/riscv64`, nothing registered | exit **125**, naming the ELF machine it looked for, how many registrations are enabled, and that installing `qemu-user-static` is what docker and podman rely on too |

⛔ **podbox reads the registrations and never writes one.** Registering an
interpreter is a machine-wide change and podbox is not the machine's owner.
What it does write, on the operator's ruling of 2026-09-09, is the interpreter
**into the rootfs** where the registration lacks `F`, and that write is
disclosed in the banner naming the exact path: the payload can see that file,
and the honesty rules have no exception for a helpful edit.

⚠ **The `F` flag is read out of the registration rather than assumed.** Without
it the kernel opens the interpreter by path **at exec time**, and that path is
inside the new root; with it the interpreter is opened when the registration is
made and held. Measured, `experiments/results/multiarch.txt` clause 5: an `F`
registration ran an aarch64 binary in a bare chroot containing nothing but that
binary.

⚠ **The `e_machine` is parsed out of the registration's own magic**, at byte 18
of the ELF header, so podbox answers "is there an interpreter for THIS image"
rather than "is there any interpreter at all". A magic too short to reach byte
18 does not select on the architecture, and podbox treats that as no match
rather than as a match.

**Point 5 done, 2026-09-09.** `crates/podbox-probe/src/interp.rs`,
`measured_by` in `report::document`, a line in the banner, and `interpreter` as
the eighth component of the cache key. Driven by
`experiments/260-multiarch.sh` clause 6.

| | |
| --- | --- |
| `measured_by.emulated`, aarch64 podbox under qemu on this amd64 host | **`true`** |
| `measured_by.interpreter` and `cache_key.interpreter` | both `/usr/bin/qemu-aarch64-static` |
| what the banner says, beside the rung | `⛔ MEASURED BY /usr/bin/qemu-aarch64-static, NOT BY THIS MACHINE` |
| that answer offered to a native podbox sharing the store | refused: `the instrument changed` |

⭐ **What can be detected was measured rather than assumed**, by running the
arm64 rootfs's own tools under `qemu-aarch64-static`:

| asked | under the emulator | usable |
| --- | --- | --- |
| `/proc/cpuinfo` | `model name: ARMv8 Processor rev 0 (v8l)` | ⛔ no, emulated |
| `readlink /proc/self/exe` | the guest binary's own path | ⛔ no, emulated |
| `/proc/self/maps` | guest mappings only; qemu's own are filtered out | ⛔ no |
| `ls /proc/sys/fs/binfmt_misc` | the **host's** registrations | ⭐ yes |

⛔ **So the claim is conditional and says so.** podbox cannot prove it is
running natively; what it establishes is that this machine routes binaries of
podbox's **own** ELF machine through an interpreter. The other state is
`NoEvidence`, it carries the list of what was checked, and it is never printed
as "measured natively": `TODO/probe.md` T-0109 rule 1 forbids collapsing "no
evidence" into an answer.

⚠ **Two routes in, and only one leaves a `binfmt_misc` trace.** An explicit
`qemu-aarch64-static ./podbox` involves no registration at all, so the
environment is checked for the variables qemu-user reads. That is weaker
evidence and is reported as its own sentence rather than merged with the first.

⛔ **The reader moved to `podbox-probe` rather than being copied.**
`podbox-enter::binfmt` asks "can this machine execute a FOREIGN image"; this
asks the mirror question about podbox's OWN architecture. One parser answers
both, and `podbox-enter` re-exports it.

⚠ **A defect this found in `experiments/260-multiarch.sh`**: clause 4's reading
depended on whatever binfmt state the machine happened to be in, because
`podbox probe` re-execs itself once per probe and an explicitly-invoked aarch64
podbox cannot start its own children without a registration. The rung read
`namespace` on a machine that had one and `unsupported` on a machine that did
not, and the committed reading recorded only the first. The registration is now
made once, before clause 4, and the trap removes it.

