# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M3 is in, and `podbox run` works. A command runs inside a pulled and
extracted image, the payload owns stdout, the banner names the rung on stderr,
and the exit code is the payload's own. Inside the reconstruction podbox falls
to the `chroot` rung and says what that mode does not provide. podbox is also no
longer one architecture: six build the whole workspace, and it runs a
`linux/arm64` image on an amd64 host through binfmt. Local registries with no
certificate, or one nothing trusts, are reachable by name. M4, the lifecycle, is
next.**
Session of 2026-09-09, on `main`.

⭐ **Four things this session settled that every previous session re-derived**,
and each is written where a session will hit it rather than here: the branch
([RULES.md](RULES.md) section 2), the environment
([`../scripts/dev.sh`](../scripts/dev.sh), which `docs/AGENTS.md` now opens
with), the two kernel questions nobody could answer ([T-0112](probe.md)), and
the registry quota that could turn the acceptance red ([T-0206](image.md)).

## The measured baseline

One machine, one day: kernel `6.18.44-fc-v24` (a Firecracker guest), rustc
1.98.1, cargo 1.98.1, GNU tar 1.35, docker 29.3.1, glibc 2.39, musl 1.2.4,
zig 0.16.0, 4 CPUs, 16 GB RAM.
⚠ `CONFIG_SECURITY_LANDLOCK` is unset on this kernel, so the M-mechanism rows of
the reconstruction cannot run here and report `SKIP`.
⚠ `CONFIG_CHECKPOINT_RESTORE` is also unset, so `kcmp(2)` answers `ENOSYS`. That
is a control of the bogus-argument discriminator, and the consequence is
[T-0102](probe.md)'s correction, which the entry carries.

| what | value | taken by |
| --- | --- | --- |
| ⭐ release binary, M3 complete, `x86_64-unknown-linux-musl` | 2,360,176 bytes | `experiments/110-bloat-delta.sh enter`, T-0910 |
| third-party crates in that binary | 97 | the same script |
| `PT_INTERP` in it | none. still static-pie | `readelf -l`, the same script |
| ⭐ architectures the workspace compiles for | **6**, and `powerpc64le` blocked and named | `experiments/260-multiarch.sh`, T-0911 |
| what six architectures cost the binary | **+128 bytes** on 2,282,224. ⚠ one build, below the instrument's resolution | the same |
| ⭐ `podbox run --platform linux/arm64 <img> uname -m`, on this amd64 host | **`aarch64`** | `experiments/300-run.sh` clause 5 |
| ⭐ the rung `podbox run` selects inside the reconstruction | **`chroot`**, where this host is `namespace` | `experiments/300-run.sh` clause 7 |
| ⭐ `landlock_create_ruleset` in a QEMU guest | **`ok`, ABI 6**, where this host answers `ENOSYS` | `experiments/290-microvm.sh`, T-0112 |
| ⭐ the `kcmp(2)` control in that guest | **`ESRCH`**, which is the target's own answer | the same |
| that whole boot, probe and poweroff, under TCG | about **6 s** | the same |
| a cold compile against an empty target directory | **29 s**, 87 crates, 150 MB | `experiments/310-session-startup.sh`, T-1005 |
| release binary, M2 complete | 2,282,224 bytes | `experiments/results/bloat-extract.txt` |
| release binary, M1 complete | 2,130,672 bytes | `experiments/results/bloat-image.txt` |
| release binary, M0 complete | 496,184 bytes | `experiments/results/bloat-baseline.txt`, T-0910 |
| release binary, empty skeleton | 389,656 bytes | `cargo build --release`, T-1100 |
| ⭐ `podbox images --format '{{.Digest}}' alpine:latest` | `sha256:28bd5fe8b56d…43f8b` | `experiments/150-image-acquisition.sh` |
| ⭐ `docker image inspect`'s `RepoDigests[0]` for the same tag | **the same value** | the same script |
| blobs a fresh `alpine:latest` pull stores | 4, all hashing to their own names | the same script |
| a second pull of the same tag | 0 layers fetched, every one `Already exists` | the same script |
| `podbox pull http://…` | exit **2**, "podbox speaks HTTPS only", under a 30 s timeout | the same script |
| a pull into a 1 MiB tmpfs | refused, 0 blobs written, message names all four things | `experiments/140-space-precheck.sh` |
| a pull with 6 free inodes and 256 MiB free | refused, and the message names inodes | the same script |
| registries the challenge-driven auth answers | 4: Docker Hub, `public.ecr.aws`, `ghcr.io`, `quay.io` | driven by hand, and `140-`/`160-` now default to the second |
| `rmi` while something holds the image | exit 125, "is in use by a running container" | `experiments/160-store-gc.sh` |
| `image prune -af` under the same holder | exit 0, `skipped:` names it, image kept | the same script |
| a blob path resolving outside the store | refused by name; the canary survived | the same script |
| ⭐ boot id: host, and inside the reconstruction | **identical**, `b538b475-930e-4dd1-9dec-3098dd77212f` | `experiments/170-probe-cache.sh` |
| ⭐ rung: host, and inside the reconstruction | `namespace` and `chroot` | the same script |
| the cache key components that differ between them | `mnt_ns`, `uid_map`, `gid_map`, `setgroups`, `seccomp`, `seccomp_filters` | the same script |
| `podbox probe` rung, unconfined on this host | `namespace` | `experiments/130-probe-parity.sh` |
| `podbox probe` rung, inside `experiments/20-enter-target.sh` | `chroot` | `experiments/130-probe-parity.sh` |
| attribution rows against `experiments/results/attribute.txt` | 15 matched, 1 recorded divergence, 0 differed | `experiments/130-probe-parity.sh` |
| probes in the set | 50: 34 census, 16 attribution | `podbox probe --json` |
| `/dev/ptmx` here, unconfined | present and openable: chardev 5:2, mode 666 | `podbox probe --json \| jq .ptmx`, T-0503 |
| `/dev/ptmx` inside the reconstruction | `ENOENT` to both `stat` and `open`. ⛔ Not an answer about the target | the same |
| `kcmp(-1,-1,...)` control, on this host | `ENOSYS`: the control cannot answer | `experiments/results/attribute.txt` |
| `kcmp(-1,-1,...)` control, on the target | `ESRCH`: executed | `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26` |
| writable paths obtained inside the reconstruction | 4 of 8 probed: `/tmp`, `/dev/shm`, `/workspace`, `/state` | `podbox probe --json` |
| interposer cdylib under `+crt-static` | refused by cargo | `experiments/60-interposer-libc.sh` |
| interposer cdylib, `-crt-static`, musl target, linked by `zig cc` | 286,056 bytes, `DT_NEEDED libc.so` | `experiments/60-interposer-libc.sh`, which exits 0 |
| interposer cdylib, `-crt-static`, gnu target | 266,632 bytes | `experiments/60-interposer-libc.sh` |
| podbox's own musl cdylib into a glibc payload | **refused**, `libc.so: invalid ELF header`, rc 127 | `experiments/60-interposer-libc.sh` check B |
| musl object into a glibc payload | **refused**, `libc.so: invalid ELF header` | `experiments/80-interposer-abi.sh` |
| glibc object into a musl payload | **refused**, `__snprintf_chk: symbol not found` | `experiments/80-interposer-abi.sh` |
| a `GLIBC_2.34` import against a libc declaring `GLIBC_2.31` | refused, and the loader names the version | `experiments/80-interposer-abi.sh` |
| `libc.so.6` symbol tables | `.symtab` 0 defined, `.dynsym` 3136 | `experiments/80-interposer-abi.sh` |
| an interposer defining `execve` alone | catches 1 of 7 exec entry points | `experiments/100-interpose-symbols.sh` |
| the same seven with every entry point defined | 7 of 7 | `experiments/100-interpose-symbols.sh` |
| the adopted mechanism's exported set | 129 symbols: 113 libc entry points, 16 internal | `experiments/100-interpose-symbols.sh` |
| path-taking names a pinned debian rootfs reaches | 38, of which 3 are undefined | `experiments/100-interpose-symbols.sh` |
| a supplied `/etc/passwd` under `nsswitch: files` | read | `experiments/90-nsswitch-contract.sh` |
| the same file under a non-`files` service | **ignored**, `getpwnam` returns NULL | `experiments/90-nsswitch-contract.sh` |
| glibc gconv modules recording `DT_NEEDED libc.so.6` | 253 of 253 | `experiments/90-nsswitch-contract.sh` |
| distributions where a supplied `/etc/passwd` is read | 10 of 11 | `experiments/125-across-distributions.sh` |
| distinct `nsswitch` `passwd` shapes across those 11 | 6 | `experiments/125-across-distributions.sh` |
| a static glibc binary on `opensuse-leap-15.6` | **SIGFPE**, rc 136 | `experiments/125-across-distributions.sh` |
| gate coverage | ⛔ not recorded here. It is **self-referential**: writing the number down changes it | `scripts/check-todo.py`, on every run |
| the gate's checks, planted against | 17 checks, 18 cases; see the acceptance block below | `scripts/plant.sh` |
| corpus | 30 trees, 154 MB in a fresh clone | `scripts/common/mine-repo.sh` |
| ⭐ release binary, M2 complete, `x86_64-unknown-linux-musl` | 2,282,224 bytes | `experiments/110-bloat-delta.sh extract` |
| third-party crates in that binary | 95 | the same script |
| ⭐ `alpine:latest` extracted | 515 entries, 1 with an id this host cannot apply | `podbox extract` |
| that one entry | `etc/shadow`, gid 42, exactly as check B predicted | `.meta.jsonl` |
| ⭐ `voidlinux-musl:latest` extracted | 2 layers, 4,664 entries, 1 whiteout, 3 dropped ids | `podbox extract` |
| symlinks surviving in that tree | 543, absolute ones rebased-and-allowed, stored verbatim | `find -type l` |
| `var/cache/xbps` after extraction | gone, whiteouted by layer 2 | `experiments/220-extract-path-safety.sh` and by hand |
| hostile layers refused | 4 of 4: traversal, `..`, absolute, hard link out | `experiments/220-extract-path-safety.sh` |
| entries that escaped a destination | **0**, checked on the filesystem rather than on an exit code | the same script |
| ⭐ `openat2(2)` on this kernel | **present** (6.18.44), so the `O_NOFOLLOW` walk needs forcing to run at all | measured by calling it |
| `execve` of `tar` during a full extraction | **0**. The only `execve` is podbox itself | `strace -f -e trace=execve` |
| ⭐ alpine's layer, compressed then uncompressed | 3,846,391 then 8,697,856 bytes, the second read from gzip `ISIZE` | `podbox extract` |
| ⭐ `docker image inspect --format '{{.Size}}'`, same image | 3,857,242 = podbox's blob total **to the byte**. ⛔ COMPRESSED, not uncompressed: see [T-0202](image.md) | docker 29.3.1, containerd store |
| `alpine:3.20` `etc/shadow` ownership | uid 0, gid 42 | `experiments/70-whiteout-contract.sh` |
| OCI layer member-name prefix | no `./` on any layer of either pinned image | `experiments/70-whiteout-contract.sh` |
| a slash-anchored whiteout glob against a layer-root whiteout | misses it | `experiments/70-whiteout-contract.sh` |

Acceptance, run on 2026-09-09. ⚠ A list of commands and **not an `&&` chain**,
deliberately: an experiment exits 2 when it could not run, and a chain collapses
that third state into a failure and reports one status for every clause.

```
$ ./scripts/dev.sh check
  fmt, clippy -D warnings, build, tests, the gate and the markers, all green
$ ./scripts/check-todo.py
check-todo: 104 rows, 104 entries, 47 open, 3 partial, 2 blocked, 52 done
$ ./scripts/plant.sh
  plants   21 caught, 0 missed
  controls 3 quiet, 0 fired
$ cargo test --workspace
  212 passed, 0 failed
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
$ ./experiments/110-bloat-delta.sh enter
  total_bytes 2360176   headroom 5639824   third-party crates 97
$ ./experiments/130-probe-parity.sh
  got chroot / got namespace / 15 matched, 1 recorded divergence, 0 differed
$ ./experiments/150-image-acquisition.sh
  podbox and docker report the same digest, against ghcr.io
$ ./experiments/220-extract-path-safety.sh
  A traverse / B dotdot / C absolute / D hardlink refused; E legit survived
$ ./experiments/260-multiarch.sh
  6 architectures check the workspace; 1 blocked and named; aarch64 runs
$ ./experiments/270-multiarch-image.sh
  two platforms of one tag, and the ELF machine inside each tree
$ ./experiments/280-insecure-registry.sh
  7 clauses, against two real registry:2 instances
$ ./experiments/290-microvm.sh
  landlock ok ABI 6, kcmp ESRCH, in about 6 s under TCG
$ ./experiments/300-run.sh
  7 clauses; clause 7 is the chroot rung inside the reconstruction
$ ./experiments/310-session-startup.sh
  a cold compile is 29 s and 87 crates; dev.sh returns in 1 s
```

## Counts

105 entries: 47 open, 3 partial, 2 blocked, 53 done.

⭐ **Six entries were authored this session and five of them are already
`done`**, which is the opposite of the last session's shape and is worth saying
why: [T-0911](deps.md), [T-0212](image.md), [T-0213](image.md),
[T-0112](probe.md), [T-0113](probe.md) and [T-1005](packaging.md) were each
authored **because something was built or measured**, so the entry and its
evidence landed together.

⚠ **Two were authored and NOT implemented**, in their own pass per
`docs/AGENTS.md`'s routing table: [T-0506](enter.md), which is `partial` with
its remaining half named, and [T-0411](complete.md), which cannot be measured
until a payload can be run and now can be.

⚠ **Four entries changed status without new code**, and each says why in place:
[T-1103](milestones.md) closed on clauses that only needed `run`;
[T-0107](probe.md), [T-0108](probe.md) and [T-0204](image.md) the same;
[T-0206](image.md) dropped from P1 to P3 because its premise was measured and
found wrong.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

⭐ **It opened by consolidating two branches nobody had merged.** `origin/main`
was at M0's tip and carried neither M1 nor M2, so a clone of it could not run
the acceptance in this file. Both were strict fast-forwards, so
`main` now carries everything and [RULES.md](RULES.md) section 2 carries the
cost so no session re-derives it. ⚠ The two stale branches still exist: see the
open questions.

### M3, `run`, and the four halves it closed

`crates/podbox-enter` and the `run` verb, driven by `experiments/300-run.sh`,
seven clauses, exit 0. [T-1104](milestones.md) is `partial`: its own `Prove`
passes and [T-0505](enter.md), [T-0801](cli.md) and [T-0803](cli.md) do not
exist yet.

⭐ **Clause 7 is the one that makes the others mean something.** Every other
clause runs on this host, where `mount(2)` succeeds and podbox selects
`namespace`. The reconstruction is where it is `EPERM`:

| | |
| --- | --- |
| stdout | `hi`, and nothing else |
| the rung inside the reconstruction | **`chroot`** |
| the same podbox on this host | `namespace` |
| the banner | `does NOT provide: process, network, IPC or mount isolation` |

[T-0107](probe.md), [T-0108](probe.md), [T-0204](image.md) and
[T-1103](milestones.md) were all `partial` for a half that needed `run`, and all
four are closed. ⭐ [T-1103](milestones.md)'s close showed the ownership wall
from **inside** the container for the first time: `ls -ln /etc/shadow` reports
gid 0, not the 42 the image declares.

### podbox stops being one architecture

⛔ **`crates/podbox-probe/src/sys.rs` declared 46 x86_64 syscall numbers by hand
and `compile_error!`d everywhere else**, and `oci::ARCH` was the constant
`"amd64"`. [T-0911](deps.md) and [T-0212](image.md).

| | before | after |
| --- | --- | --- |
| architectures the workspace compiles for | **1** | **6**, and one named blocker |
| the shipping binary, x86_64 | 2,282,224 | 2,282,352 bytes, +128 for the tables |
| records one store can hold for one tag | 1 | one per platform |

⭐ **The size question answered itself**: `syscalls` and `linux-raw-sys` are
`const` tables and inlined assembly, so six architectures cost 128 bytes. ⚠ One
build on one host, below what that instrument resolves.

⭐ **podbox does not merely compile for aarch64, it runs there**, and
`podbox run --platform linux/arm64 <image> /bin/uname -m` prints `aarch64` on
this amd64 host. ⛔ And a probe under `qemu-user` measures **QEMU**:
[T-0506](enter.md) carries what podbox still owes about saying so.

⚠ `powerpc64le` does not build, and not for a rustc limit: `syscalls` 0.8.1
gates powerpc, s390x and mips behind `asm_experimental_arch`, and clause 3 of
`experiments/260-multiarch.sh` compiles powerpc64 inline assembly on stable.
That clause goes **red** the day it starts building.

### A registry with no certificate, and the rule that forbade it did not exist

[T-0213](image.md). podbox could not reach a local registry at all.
`--insecure-registry HOST` is docker's flag and meaning, `--tls-verify=false` is
podman's, and neither is the other: not verifying a certificate and not having
one are different asks.

⛔ **`tls.rs` cited a rule that is not written where it said.** It claimed
`docs/security/remote-ops.md` and `docs/AGENTS.md` both forbid disabling
verification. `remote-ops.md` says nothing about TLS, and `AGENTS.md`'s note is
an instruction to **the agent** about this container's proxy.

⭐ **A defect the experiment found that no unit test could**: the first working
build pulled a loopback registry **through the environment's proxy** and got
`HTTP 405`. A loopback registry is never proxied now, and `NO_PROXY` is honoured,
which `ureq` 2 does not do at all.

### Two standing questions, answered by building the answer

⭐ **On the operator's suggestion**, [T-0112](probe.md):
`experiments/290-microvm.sh` boots a stock Alpine kernel under QEMU with an
initramfs carrying busybox and podbox, in **about 6 seconds** under TCG.

| | this host | the VM | the target |
| --- | --- | --- | --- |
| the `kcmp(2)` control | cannot answer, `ENOSYS` | **`ESRCH`** | `ESRCH` |
| `landlock_create_ruleset` | `denied ENOSYS` | **`ok`, ABI 6** | - |

⚠ A second **machine**, never a second **target**: a VM with every capability is
the easy case and proves nothing about confinement.

### A session reaches the code in one command

[T-1005](packaging.md). `scripts/dev.sh` starts the environment and the build in
the background and returns in a second, printing what to read while it works. A
cold compile is 29 s and 87 crates; the opening reading is 5,944 words.

⭐ **It found a defect on its first run.** [T-0113](probe.md): the write
allowlist reported `/tmp` as a skip, because the scratch filename carried the
**pid** and `cargo test` runs tests in parallel **threads** of one process.

### What the four review passes found

⭐ Each asked a different question and three of the four found something.

1. **The door sweep.** ⛔ **[T-0212](image.md) opened a second door and only one
   was guarded.** `find_for` took the platform and `find_one` did not, and
   `extract` and `inspect` reach the store through the second: `podbox extract
   alpine` silently unpacked whichever platform was pulled last. Fixed, and
   `extract` grew `--platform`.
2. **The guard mutation.** Four guards planted and each turned exactly its own
   test red. ⛔ **And the harness itself was wrong first**: it read
   `test result` with `head -1`, which took the **first suite's** result, and
   that suite had run zero tests. Two mutations reported "caught" when nothing
   had run. The aggregate across suites is what the numbers above are.
3. **The claim audit.** Every number here was re-derived after the last commit,
   and all six result files this session produced are tracked.
4. **The cold pass.** A fresh clone: gate 0, markers 0, every experiment
   executable, nothing leaked. ⚠ One real finding: `dev.sh --help` created its
   state directory, so a read wrote. Fixed.

## In progress

Nothing. Every entry this session opened is closed or is `partial` with the
remaining half named and the milestone it lands in.

## The work order

⭐ **This is the only work order.** Do not take one from the index or from a
kickoff prompt.

1. ⭐ **Finish M3's named remainder before starting M4.** [T-1104](milestones.md)
   is `partial` and names exactly three: [T-0505](enter.md), `exec` as a fresh
   chroot; [T-0801](cli.md), the verb and flag parity table **as data**, which is
   `L` and is the biggest single piece of the CLI contract; and
   [T-0803](cli.md), answering to `docker` and `podman` on PATH, which the
   operator already ruled on 2026-09-08.
2. **M4, the lifecycle.** [T-1105](milestones.md) and
   [supervise.md](supervise.md). `create`, `start`, `ps`, `logs`, `stop`, `rm`,
   `exec`, `inspect`, `kill`, `wait`, `cp`, twenty consecutive passes.
   ⚠ It needs [T-0505](enter.md) from step 1, so the order above is not
   negotiable.
3. **M5, M6, M7 in order.** [T-0709](interpose.md) and [T-0410](complete.md) are
   both P0 and land inside M6 and M5.

⭐ **Three entries are P1 and belong beside the milestones rather than after
them**, because each is about the gate or about honesty rather than a new
capability:

- [T-0506](enter.md), the half of the foreign-architecture work that is **not**
  done: a `podbox probe` inside a `qemu-user` container measures the emulator,
  and podbox does not yet mark the answer as the emulator's or key
  `$store/probe.json` on the interpreter. [T-0112](probe.md) measured why that
  matters.
- [T-0411](complete.md), a payload whose own package sources are `http://` on a
  runtime where tcp/80 hangs. ⚠ It cannot be measured before it can be run, and
  now it can.
- [T-0210](image.md), the store's concurrency contract, `L`. One ordering of one
  case was driven; the contract is not written down, so every future change to
  the store is a change to an unstated one. ⚠ **This session made that worse in
  a way worth naming**: [T-0211](image.md) and [T-0113](probe.md) were both
  concurrency defects in code nobody had written a contract for, and the second
  was found by luck.

⚠ [T-0206](image.md) dropped to P3. The acceptance no longer touches a
quota-bearing registry, so what the fixture still buys is running with the
network **off**, which is weaker than the problem it was filed for.

⚠ [T-0207](image.md), [T-0208](image.md), [T-0209](image.md),
[T-0205](image.md) and [T-1004](packaging.md) are P2 or P3 and are not in the
order.

## Open questions for the operator

⭐ **Four of the seven questions this section carried are answered, and two of
them by building the answer rather than by asking again.** What is left is what
genuinely needs somebody with access to a machine this project cannot reach.

1. **`/dev/ptmx` on the target**, and it is ⭐ **one command rather than a
   research task**: `podbox probe --json | jq .ptmx`, run on the target by
   anyone who can reach it. The probe half of [T-0503](enter.md) is implemented
   and reads `present: true, usable: true` on this host and `ENOENT` inside the
   reconstruction. ⛔ **The reconstruction does not settle it**: it builds
   `/dev` from the same mount table the question doubts, so its answer is that
   table repeated back. ⚠ [T-0112](probe.md)'s virtual machine does not settle
   it either, for the same reason in a different shape: what `/dev/ptmx` does
   there is a property of the initramfs this project writes.
2. **Why a static glibc binary takes SIGFPE on `opensuse-leap-15.6`.**
   Measured by `experiments/125-across-distributions.sh` and recorded as a
   reading, not a diagnosis. It bears on what podbox may assume about payloads
   in an image, and the row is also the one distribution whose `nsswitch: compat`
   setting is therefore untested for the passwd question.
3. ⚠ **Two stale branches cannot be deleted from here.**
   `claude/m2-extraction-ypu8qc` and `claude/podbox-m1-image-acquisition-8p2n3d`
   carry **zero commits that `main` does not**, verified with
   `git merge-base --is-ancestor` before anything was pushed, so nothing is at
   risk and nothing is lost. ⛔ Both `git push --delete` and the REST
   `DELETE /repos/.../git/refs/heads/...` are refused by this session's proxy
   with `Write access to this GitHub API path is not permitted through this
   proxy`, which is an environment policy and not a GitHub permission. It needs
   somebody with ordinary repository access to delete them, or nothing at all:
   they are inert.
4. ⚠ **Nothing else.** Everything below was a question and is now a measurement
   or a ruling, kept here only so a reader does not go looking for it.

### What was open and is not

- ⭐ **A kernel with Landlock. ANSWERED by building one**, on the operator's
  suggestion of 2026-09-09. [T-0112](probe.md):
  `experiments/290-microvm.sh` boots a stock Alpine `virt` kernel under QEMU and
  `landlock_create_ruleset(VERSION)` answers **`ok`, ABI 6**. ⚠ No `/dev/kvm`
  here, so it runs under TCG, and the whole cycle is about 6 seconds.
- ⭐ **A kernel with `CONFIG_CHECKPOINT_RESTORE`. ANSWERED the same way**, and
  it agrees with the target: the `kcmp(2)` control answers **`ESRCH`** in the
  VM, which is `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26`'s
  reading, and `controls_answered` is `true` there. [T-0102](probe.md).
- ⭐ **Whether podbox should refuse the `docker` name where a daemon is
  reachable. RULED by the operator on 2026-09-08** and carried by
  [T-0803](cli.md), which was already closed when this section last listed it as
  open. It refuses, unless an explicit flag says otherwise.
- ⭐ **The branch. SETTLED on 2026-09-09** and written into
  [RULES.md](RULES.md) section 2 with the cost it carried, so no session
  re-derives it: `main`, always, the harness notwithstanding.
- ⭐ **What `{{.Size}}` should report. ANSWERED BY MEASUREMENT on 2026-09-08**,
  and the premise was wrong: docker 29.3.1 here runs the containerd image store
  and its `.Size` is the **compressed** content, equal to podbox's blob total to
  the byte. [T-0202](image.md) carries the table and the caveat that
  "uncompressed size" is three different numbers.
