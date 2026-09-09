# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M5 is BUILT and its acceptance is 8 of 10. `crates/podbox-complete`
exists, `podbox run` and `podbox exec` both take it, and eight of the ten M5
distributions install a C toolchain through their own package manager and build
and run a two-file program with it.** ⭐ The three P0 CLI entries M3 left open
are closed and docker's exit codes were MEASURED rather than read: four of eight
cases were wrong. ⭐ The powerpc gate is cleared, so eight architectures check
the workspace where six did, and the powerpc64le binary runs under an emulator.
Session of 2026-09-09, on `main`.

⭐ **The one thing worth reading before anything else is that the banner was
lying.** It printed `mode=` the rung the PROBE selects, and `podbox_enter`
performs a plain `chroot` on every machine. On this host, which probes as
`namespace`, every `podbox run` said `mode=namespace (namespaces: as configured;
mounts: full)` for a payload that had neither. [T-0804](cli.md) is where that was
found, and it is exactly the rule that entry exists to enforce: no output may
imply namespaces, cgroups or devices exist when they do not.

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
| ⭐ release binary, M5 complete, `x86_64-unknown-linux-musl` | 2,601,840 bytes | `experiments/110-bloat-delta.sh enter`, T-0910 |
| third-party crates in that binary | 98 | the same script |
| what M5's completion layer cost | **+241,664 bytes** on M3's 2,360,176 | the same script |
| ⭐ **M5 rows that install a toolchain and build a program** | **8 of 10**; 1 the machine's, 1 podbox's and diagnosed | `experiments/240-distro-sweep.sh`, T-1106 |
| the row docker also fails on, so it is the machine's | `voidlinux-musl`, `SSL_connect returned 1` from xbps | the same, its docker control |
| the row docker succeeds on, so it is podbox's | `opensuse-leap`; libzypp reads a hash-indexed CApath | the same, and [T-0412](complete.md) |
| ⭐ `/dev/null` inside a container on THIS host | a real character device 1:3 | the same. ⚠ `mknod` is TRIED before a shim is written |
| ⭐ architectures that check the workspace | **8**, and **0** blocked | `experiments/260-multiarch.sh`, T-0912 |
| ⭐ `podbox probe --json` under `qemu-ppc64le-static` | parses, and reads `ELF machine 0x15` out of its own header | the same, clause 7 |
| ⭐ exit-code cases where podbox and docker agree | **15 of 15**, after four were wrong | `experiments/330-exit-codes.sh`, T-0802 |
| docker's code for a flag its PARSER refuses | **125**; for anything a verb refuses after that, **1** | the same |
| a command found and not invocable | **126**, where podbox said 127 | the same |
| refusals driven from outside the binary | 10, each asserting the code AND the reason | `experiments/250-negative-tests.sh`, T-1109 |
| doors a symlink could make the completion layer write through | 6 planted, **0** escapes | `experiments/85-completion-symlink-escape.sh`, T-0405 |
| ⭐ tcp/80 egress from this host | **works**, 200 in 0.104 s. ⛔ So T-0411's premise does not hold here | `curl`, recorded in [T-0411](complete.md) |
| `PT_INTERP` in it | none. still static-pie | `readelf -l`, the same script |
| ⭐ architectures the workspace compiles for | **6**, and `powerpc64le` blocked and named | `experiments/260-multiarch.sh`, T-0911 |
| what six architectures cost the binary | **+128 bytes** on 2,282,224. ⚠ one build, below the instrument's resolution | the same |
| ⭐ `podbox run --platform linux/arm64 <img> uname -m`, on this amd64 host | **`aarch64`** | `experiments/300-run.sh` clause 5 |
| ⭐ the rung `podbox run` ENTERS with, on any machine | **`chroot`**, and the banner says so and names what the machine would permit | `podbox_enter::ENTERED_RUNG`, T-0804 |
| ⭐ **consecutive lifecycle passes** | **20 of 20**, after three runs that reached 10, 9 and 1 | `experiments/230-lifecycle-loop.sh` |
| a container whose LAUNCHER was `SIGKILL`ed | `dead`, exit code `-`, and `wait` exits 125 rather than printing one | the same, clause 2 |
| threads on the launcher's spawn path | **1** | the same, clause 3 |
| ⭐ rows in the verb and flag parity table | **139**, over 53 verbs, four statuses and no fifth | `podbox system info --format '{{json .Parity}}'` |
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
| `podbox pull http://…` | exit **1**, "podbox speaks HTTPS only", under a 30 s timeout. ⚠ 1 and not 2 since T-0802 measured docker | the same script |
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
check-todo: 108 rows, 108 entries, 22 open, 3 partial, 2 blocked, 81 done
$ ./scripts/plant.sh
  plants   23 caught, 0 missed
  controls 3 quiet, 0 fired
$ cargo test --workspace
  the aggregate ACROSS suites, never `| head -1`: 0 failed
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
$ ./experiments/110-bloat-delta.sh enter
  total_bytes 2601840   headroom 5398160   third-party crates 98
$ ./experiments/85-completion-symlink-escape.sh
  6 doors, 0 escapes; five confined and the one that stays inside followed
$ ./experiments/240-distro-sweep.sh
  rows 10, ran 10, built_and_ran 8, host_not_runtime 1. ⛔ exits 1: opensuse
$ ./experiments/250-negative-tests.sh
  10 refusals driven from outside; exits 2, three could not be measured here
$ ./experiments/260-multiarch.sh
  8 architectures check the workspace, 0 blocked; ppc64le RUNS under qemu
$ ./experiments/330-exit-codes.sh
  15 cases, podbox and docker agree on every one
$ ./experiments/130-probe-parity.sh
  got chroot / got namespace / 15 matched, 1 recorded divergence, 0 differed
$ ./experiments/150-image-acquisition.sh
  podbox and docker report the same digest, against ghcr.io
$ ./experiments/220-extract-path-safety.sh
  A traverse / B dotdot / C absolute / D hardlink refused; E legit survived
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
  139 parity rows, 53 verbs; docker and podman both run the payload and say so
```

## Counts

109 entries: 19 open, 2 partial, 2 blocked, 86 done.

⭐ **Fifteen entries closed and three are `partial`.** [T-0503](enter.md) waits
on a machine nobody here can reach; [T-1106](milestones.md) is 8 of 10 rows and
names the one that is podbox's; [T-1109](milestones.md) drove ten refusals and
says which three could not be driven here.

⚠ **Three entries were authored and not implemented**, which is the rule:
[T-0412](complete.md), [T-0608](supervise.md) and nothing else.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

### The banner was lying, and T-0804 is where that was found

⛔ **`mode=` printed the rung the PROBE selects, and `podbox_enter` performs a
plain `chroot` on every machine.** On this host, which probes `namespace`, every
run announced `namespaces: as configured; mounts: full` for a payload with
neither. The banner is built from `podbox_enter::ENTERED_RUNG` now -- one
constant, so the banner, the container record and `--strict` cannot disagree --
and it prints the machine's own answer beside it rather than dropping it.

### docker's exit codes, measured, and four of eight were wrong

⭐ **The discriminator is not obvious and it is the finding: docker exits 125
for anything its FLAG PARSER refuses and 1 for anything a verb refuses
afterwards.** `run --badflag`, `--pull=bogus` and `--memory=notasize` are 125;
`images --format '{{.Nope}}'`, `rmi no-such-image` and `run` with no image are
1. The costly one was 126: every `execve` failure was folded into "not found",
so a non-executable command said 127 where docker says 126, and a caller
branching on 127 retries with another path.

⛔ **The codes had been written out in THREE files and two copies had already
diverged.** `podbox-cli::main` carried the old usage code while
`podbox-image::error` carried the corrected one, so two verbs of one binary
disagreed about what a flag error is. They live in `podbox_probe::exit`, are
served as data by `podbox system info --format '{{json .ExitCodes}}'`, and six
clauses across four experiments read them from there instead of carrying a `2`.

### M5, and what a ten-row matrix found that one image would not

Each of these passed on some rows and failed on others:

- ⛔ `quay.io/rockylinux/rockylinux:9` declares no `PATH`, docker supplies one
  and podbox did not. The payload's shell then set a `PATH` **without exporting
  it**, so gcc ran with nothing to search for its own installation directory,
  emitted relative search paths and could not execute `cc1` -- while `cc1` sat
  in `/usr/libexec` with its execute bit set. `almalinux`, the same gcc, worked.
- ⛔ Debian's base images carry `./` as a tar member and podbox refused the
  whole layer of every one of them. The archive's own root is not an entry.
- ⛔ The completion layer resolved directory components with symlinks REFUSED,
  which is right for extraction and wrong afterwards. `/etc/ssl/certs ->
  /var/lib/ca-certificates/pem` is openSUSE's own link and `/lib -> usr/lib` is
  void's; the first made the CA fixup fail and the second made the libc probe
  read `unknown`. `RESOLVE_IN_ROOT` for the directories, `O_NOFOLLOW` and
  unlink-then-create for the last component.
- ⛔ `RESOLVE_IN_ROOT` treats the descriptor it is given as `/`, so stepping one
  component at a time moves the root with every step and an absolute link
  answers `ENOENT`. Its walk opens from the rootfs descriptor every time.
- ⚠ A CAfile at a path the image's TLS stack does not read is a bundle nothing
  reads: void's OpenSSL wants `$OPENSSLDIR/cert.pem`, which the image ships no
  copy of.

### `mknod` is tried before a shim is written

⭐ T-0401 assumed the wall; the code asks the kernel. On this host `mknodat`
SUCCEEDS, so `/dev/null` is a real character device `1:3` and the row is not a
degradation at all. ⛔ A shim reported on a machine that did not need one is a
degradation podbox invented, which is the same class of wrong as hiding one.

### This machine intercepts TLS, and that is why M5 could not be measured at all

⚠ `apk add gcc` failed with `certificate verify failed` **under docker as well
as under podbox**, so the cause is the machine. It announces its own root in
`$SSL_CERT_FILE`, `$CURL_CA_BUNDLE` and `$REQUESTS_CA_BUNDLE`, which `curl`,
`python`, `node` and `cargo` all read. T-0407 appends that announced bundle to
the image's own trust store, keeping the image's roots, under a marker so the
append is idempotent and auditable. ⛔ A bundle found only at a default path is
**no announcement** and is never propagated; `--no-host-cas` refuses the
announced case too; and the append is marked degraded, so `--strict` refuses a
run that needed it.

### T-0912: the gate was a crate-level attribute

⭐ `syscalls` 0.8.1 carries `#![feature(asm_experimental_arch)]` for five
architectures, so no `#[cfg]` inside podbox could route around it. The
dependency is target-gated away on those five and `sys.rs` takes their numbers
from `linux-raw-sys`. podbox's own trap for powerpc64 handles that
architecture's error convention, which is not x86_64's -- a positive errno in
`r3` with `CR0.SO` set -- and `260-multiarch.sh` clause 7 EXECUTES it under
`qemu-ppc64le-static`, asserting on a value the binary can only produce by
reading its own ELF header through it. ⛔ `mips` is refused at compile time
rather than given a convention nobody has run.

### Defects found in the harness rather than in podbox

- ⛔ `330-exit-codes.sh` ran `docker ""`, an EMPTY VERB, where it meant `docker`
  with no arguments, and reported "podbox and docker disagree" where they agree.
- ⛔ `240-distro-sweep.sh` counted passing rows by grepping the transcripts, and
  a failing row's transcript also carries its docker control's success. Counted
  in the loop now, which is safe because that loop is not piped.
- ⛔ Two `podbox-image` tests raced: a bare `fork` in one copies the lock fd
  another is holding in a second thread, so the second read its own lock as
  still held. They serialise on one mutex, with the reason written down.
- ⛔ `85-completion-symlink-escape.sh` carried a per-run `mktemp` path into its
  tracked reading, so it differed from itself on every run.

## In progress

Nothing is half-written. Three entries are `partial` and each names what it is
waiting for in its own file: [T-0503](enter.md) a machine, [T-1106](milestones.md)
one distribution row, [T-1109](milestones.md) three clauses that need M6 or a
machine without a pty.

## The work order

⭐ **This is the only work order.** Do not take one from the index or from a
kickoff prompt.

1. ⭐ **[T-0412](complete.md), the fixup that has to run INSIDE the rootfs.** It
   is the one thing between M5 and 10 of 10, the diagnosis is complete, and the
   same mechanism closes [T-0406](complete.md)'s keyring half. Then re-run
   `experiments/240-distro-sweep.sh` and close [T-1106](milestones.md).
2. **M6, the interposer.** [T-1107](milestones.md) and
   [interpose.md](interpose.md). [T-0709](interpose.md) is P0 and is a READER
   rather than an object: it selects by `DT_NEEDED` and refuses on the version
   predicate, and `experiments/80-interposer-abi.sh` has already measured every
   fact it needs. Then [T-0701](interpose.md) to [T-0704](interpose.md).
3. **[T-0608](supervise.md)**, the detached container seen twice and not
   reproduced. ⚠ Reproduce before changing anything: M4's own race was found by
   enumerating the doors after two written-down causes were both wrong.
4. **[T-0805](cli.md)**, the four-part diagnostic, and the rest of
   [cli.md](cli.md).
5. **M7 packaging.**

⚠ [T-0606](supervise.md) stays `blocked` and is not in the order: `supervise`
has no read channel on the target and no way to be made race-safe if it had one.

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
4. ⚠ **This machine intercepts TLS, and podbox now propagates its announced CA
   bundle into every container.** The rule is written down and narrow -- only
   where `$SSL_CERT_FILE`, `$CURL_CA_BUNDLE` or `$REQUESTS_CA_BUNDLE` names one,
   appended rather than replacing, marked degraded, and off with
   `--no-host-cas`. ⛔ It is still a trust change made inside somebody else's
   image, and it is the one decision this session made that an operator might
   want to rule on rather than inherit. [T-0407](complete.md) carries it.
5. ⚠ **`podbox run -d` was seen twice reading `exited` one second after
   starting a `sleep 20`, and could not be reproduced in eight later attempts.**
   [T-0608](supervise.md) is the entry and it names two candidates it refuses to
   act on before a reproduction. ⚠ Both sightings were on a machine with all
   four CPUs busy, which is the one condition the passing runs did not share.
6. ⚠ **Nothing else.** Everything below was a question and is now a measurement
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
- ⭐ **Whether tcp/80 hangs on this host. MEASURED, and it does NOT**: 200 in
  0.104 s. [T-0411](complete.md)'s premise is about the runtimes podbox targets
  and not about this machine, so the hang that fixup prevents cannot be
  reproduced here and the entry says so rather than claiming its `Prove`.
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
