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
Status:      open

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

---

### T-0502 Resolve the program inside the new root, in the process that changed it

Source:      `TOOL.md` section 6.5; `paper_final.md` section 5.5
Category:    enter
Priority:    P0
Effort:      S
Status:      open

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
Status:      open

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

---

### T-0505 `exec` is a fresh chroot, and `inspect` says so

Source:      `TOOL.md` section 4.2, section 6.8; `paper_final.md` section 10.6
Category:    enter
Priority:    P1
Effort:      S
Status:      open

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
Prove:       `podbox run -d --name execprobe alpine:latest sleep 30 && podbox exec execprobe sh -c 'echo marker' | grep -qx marker && podbox inspect --format '{{.Exec.Shares}}' execprobe | grep -qx filesystem && podbox rm -f execprobe`
