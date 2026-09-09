# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M3 and M4 are both CLOSED. `podbox run`, `exec`, the verb and flag
parity table as data, the `docker` and `podman` names on PATH, and the whole
lifecycle (`create`, `start`, `ps`, `logs`, `stop`, `kill`, `wait`, `rm`, `cp`,
`run -d`, `--name`) are in, and the lifecycle passes twenty consecutive times
with no sleep anywhere in it.** ⭐ CI is green again after nine red runs, the
store has a written concurrency contract, and a probe under an emulator now says
so instead of reporting qemu's answer as the machine's. M5, environment
completion, is next.
Session of 2026-09-09, on `main`.

⭐ **The one thing worth reading before anything else is how M4 closed.** Its
acceptance FAILED three times, at 10 of 20, 9 of 20 and 1 of 3, and the cause was
a real race in `stop` rather than in the loop: `stop` connects to the launcher
twice, and a container that stopped fast let the launcher tear its socket down in
between. [T-0602](supervise.md) carries it, along with the two candidate causes
that were written down first and were BOTH wrong.

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
| ⭐ **consecutive lifecycle passes** | **20 of 20**, after three runs that reached 10, 9 and 1 | `experiments/230-lifecycle-loop.sh` |
| a container whose LAUNCHER was `SIGKILL`ed | `dead`, exit code `-`, and `wait` exits 125 rather than printing one | the same, clause 2 |
| threads on the launcher's spawn path | **1** | the same, clause 3 |
| ⭐ rows in the verb and flag parity table | **131**, over 53 verbs, four statuses and no fifth | `podbox system info --format '{{json .Parity}}'` |
| ⭐ a probe under `qemu-aarch64-static`, asked what measured it | **`emulated: true`**, and the interpreter named in the cache key | `experiments/260-multiarch.sh` clause 6 |
| the same answer offered to a native podbox sharing the store | refused: `the instrument changed` | the same |
| 8 concurrent pulls of 3 references into one store | 8 exits of 0, 3 records, 12 blobs, 0 bad, 0 missing, 0 partials | `experiments/210-store-concurrency.sh` |
| a `SIGKILL` mid-pull, then any later command | 1 staging file left, **0** after the next command | the same |
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
check-todo: 106 rows, 106 entries, 37 open, 6 partial, 2 blocked, 61 done
$ ./scripts/plant.sh
  plants   23 caught, 0 missed
  controls 3 quiet, 0 fired
$ cargo test --workspace
  the aggregate ACROSS suites, never `| head -1`: 0 failed
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
$ ./experiments/210-store-concurrency.sh
  8 writers over 3 references; prune against a hold; a SIGKILL and the sweep
$ ./experiments/230-lifecycle-loop.sh 20
  20 of 20 consecutive passes, then the three clauses after it
$ ./experiments/300-run.sh
  8 clauses; clause 7 is the chroot rung inside the reconstruction, clause 8 exec
$ ./experiments/310-session-startup.sh
  a cold compile is 29 s and 87 crates; dev.sh returns in 1 s
$ ./experiments/320-cli-contract.sh
  131 parity rows, 53 verbs; docker and podman both run the payload and say so
```

## Counts

106 entries: 37 open, 1 partial, 2 blocked, 66 done.

⭐ **Fourteen entries closed and one is `partial`.** [T-0503](enter.md) is the
one, and it is waiting on a machine nobody here can reach rather than on work.

⚠ **Two entries were authored and not implemented**: [T-1206](gate.md) was
authored and implemented in the same change because it is a defect fix, and
[T-0912](deps.md) was authored from a measurement this tree already had.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

### CI had been red for nine commits, and the reason was a merge

⛔ **`.cargo/config.toml` points `CC_<target>` at `scripts/zig-cc.sh` and the
workflow carried a SECOND declaration of that requirement** as a
`bootstrap-env.sh` component list. Consolidating M1, M2 and M3 onto `main`
brought `rustls` and its `ring` with it and turned a list that had been complete
into one short by one word. Both red jobs died in the same place,
`zig-cc.sh: zig is not on PATH`, and a local `./scripts/dev.sh check` stayed
green throughout because this container has zig.

⭐ **[T-1206](gate.md) holds the invariant rather than the value.** Check 19
derives the required components in two hops, from `.cargo/config.toml` to the
wrapper it names to the component that wrapper says installs it, and hard-codes
neither. Cases 19a and 19b plant each arm.

### M3 closed: `exec`, the parity table as data, and the two names

⭐ **[T-0505](enter.md)**, `exec` as a fresh chroot. The degradation is stated in
three places that cannot disagree, because all three read one pair of constants.
⛔ Its own `Prove` could not have run in the order the work order puts it in: it
was written around `run -d`, `--name` and `rm -f`, which are M4's, and M4 is what
needs this entry. The rewrite is in the entry beside the original.

⭐ **[T-0801](cli.md), 131 rows over 53 verbs, and THE TABLE DECIDES.** Every
argument beginning with `-` goes through `parity::admit` before any match arm, so
a flag with no row cannot be quietly accepted and an arm for a flag with no row
is unreachable. `podbox run --name c1` used to be "unknown option"; where the
table says `None` it is now the row's own reason.

⭐ **[T-0803](cli.md)**, `docker` and `podman` through `argv[0]`, with the banner
naming which. The operator's ruling is implemented against a **reachable
daemon**: the socket is connected to and asked `/_ping` under a two-second
timeout, so a stale socket file reads as absent, which is the state the machines
podbox is for are in.

### The instrument that answered, and the store's contract

⭐ **[T-0506](enter.md) point 5.** What can be detected was MEASURED rather than
assumed: under `qemu-aarch64-static`, `/proc/cpuinfo` reports an ARMv8 processor
and `/proc/self/exe` names the guest binary, so both are the emulator's;
`/proc/sys/fs/binfmt_misc` is the host's, and that is what podbox reads. ⛔ The
claim is conditional and says so: the other state is `NoEvidence` and carries
what was checked.

⭐ **[T-0210](image.md)**, seven invariants in the store's own header. ⛔ Writing
I4 down found a defect nothing had run into: `rmi` and `prune` asked `in_use`
OUTSIDE the index lock, so a `run` taking its hold in between kept its image lock
and lost its blobs.

### M4, and the race its acceptance existed to find

⭐ **The loop failed three times before it passed**: 10 of 20, 9 of 20 and 1 of
3, always at `stop`. ⛔ The cause was a real race in `stop`, which connects to
the launcher twice, once to signal and once to wait: a container that stopped
fast let the launcher reap it, remove its control socket and exit in between, so
the second connect answered `ENOENT` and the fastest possible success read as
"the container is not running". A single pass would have shipped it and a retry
would have published it.

⚠ **The two causes written down before the door sweep were BOTH wrong**, and
that is worth keeping: `contain::within` re-appends the tail after resolving the
nearest existing ancestor, and `flock` is held on an open file description. What
found it was enumerating every door to the control socket.

⚠ **Three more defects the building of it found, each written where it belongs:**
`std::fs::read("/dev/urandom")` has no EOF and allocated 13 GB before the OOM
killer took it; a readiness pipe without `O_CLOEXEC` is inherited through the
payload's `execve`, so `run -d` blocked for exactly as long as the container ran;
and `si_status` is at byte 24 of a `siginfo_t` and not 20, which reported a
SIGTERMed payload as exit 128 instead of 143.

### Defects found in the harness rather than in podbox

- ⛔ `experiments/260-multiarch.sh` clause 4's reading depended on unstated
  binfmt state and flipped between `namespace` and `unsupported`. The
  registration is made once now, before clause 4, and its tracked line no longer
  carries a pid.
- ⛔ `timeout X podbox pull &` makes `$!` the pid of **`timeout`**, so killing it
  leaves podbox running; `experiments/210-store-concurrency.sh` read a correct
  refusal as a defect because of it.
- ⛔ A backtick inside double quotes is command substitution:
  `experiments/320-cli-contract.sh` ran `docker` and `podman` and printed
  docker's help into its own report.

## In progress

Nothing. Every entry this session opened is closed, and the one `partial`
([T-0503](enter.md)) names the machine it is waiting for.

## The work order

⭐ **This is the only work order.** Do not take one from the index or from a
kickoff prompt.

1. ⭐ **M5, environment completion.** [T-1106](milestones.md) and
   [complete.md](complete.md). [T-0410](complete.md) is P0 and is the one that
   decides whether any of the rest works: a supplied `/etc/passwd` under a
   non-`files` `nsswitch` is a no-op, measured across eleven distributions.
2. **[T-0411](complete.md)**, a payload whose package sources are `http://` on a
   runtime where tcp/80 hangs. ⚠ It could not be measured before a payload could
   be run, and now it can.
3. **[T-0912](deps.md)**, the powerpc gate. Measured stale on 2026-09-09: the
   inline assembly compiles on stable and the gate is the crate's, so podbox can
   clear it with `linux-raw-sys` and its own trap.
4. **M6, M7 in order.** [T-0709](interpose.md) is P0 and lands inside M6.

⚠ [T-0606](supervise.md) stays `blocked` and is not in the order: `supervise` has
no read channel on the target and no way to be made race-safe if it had one.

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
