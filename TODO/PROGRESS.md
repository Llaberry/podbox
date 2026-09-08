# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M-1 complete. No podbox code exists. M0, the probe, is next.**
Session of 2026-09-08, on `main`.

[INDEX.md](INDEX.md) is the list. [RULES.md](RULES.md) is how this repository is
worked on. [reference-map.md](reference-map.md) is the corpus.
[`../docs/AGENTS.md`](../docs/AGENTS.md) is the router: a session with no memory
reads that first and it names everything else.

## The measured baseline

One machine, one day: kernel `6.18.44-fc-v24` (a Firecracker guest), rustc
1.94.1, cargo 1.94.1, GNU tar 1.35, docker 29.3.1, glibc 2.39, musl 1.2.4,
4 CPUs, 16 GB RAM.
⚠ `CONFIG_SECURITY_LANDLOCK` is unset on this kernel, so the M-mechanism rows of
the reconstruction cannot run here and report `SKIP`.

| what | value | taken by |
| --- | --- | --- |
| release binary, empty skeleton, `x86_64-unknown-linux-musl` | 389,656 bytes | `cargo build --release`, T-0910 |
| `PT_INTERP` in that binary | none. static-pie | `readelf -l`, T-1001 |
| interposer cdylib under `+crt-static` | refused by cargo | `experiments/60-interposer-libc.sh` |
| interposer cdylib, `-crt-static`, musl target | 14,064 bytes, and **not musl-linked** | `experiments/60-interposer-libc.sh` |
| interposer cdylib, `-crt-static`, gnu target | 267,680 bytes | `experiments/60-interposer-libc.sh` |
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
| the gate's checks, planted against | 14 of 15 have a case; 14 caught, 0 missed; 3 controls quiet | `scripts/plant.sh` |
| corpus | 30 trees, 152 MB in a fresh clone plus 27 MB of git objects | `scripts/common/mine-repo.sh` |
| `alpine:3.20` `etc/shadow` ownership | uid 0, gid 42 | `experiments/70-whiteout-contract.sh` |
| OCI layer member-name prefix | no `./` on any layer of either pinned image | `experiments/70-whiteout-contract.sh` |
| a slash-anchored whiteout glob against a layer-root whiteout | misses it | `experiments/70-whiteout-contract.sh` |

Acceptance, run on 2026-09-08:

```
$ ./scripts/check-todo.py
check-todo: 86 rows, 86 entries, 79 open, 0 partial, 2 blocked, 5 done
check-todo: ok
$ ./scripts/plant.sh
  plants   14 caught, 0 missed
  controls 3 quiet, 0 fired
$ cargo build --release --target x86_64-unknown-linux-musl
    Finished `release` profile [optimized] target(s)
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
```

## Counts

86 entries: 79 open, 0 partial, 2 blocked, 5 done.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

Six issues were filed against this repository. Every claim in them was checked
against source or a run before anything was adopted. Still no podbox
implementation code.

1. **The router.** [`../docs/AGENTS.md`](../docs/AGENTS.md) replaces the former
   `docs/README.md`, deleted in this session, which was a third router <!-- known-absent -->
   duplicating both the root `README.md` and the methodology it indexed.
   `docs/conventions/docs.md` names the roles a document set has, and a
   `README` under `docs/` is not one of them. The
   router carries what a session with no memory needs and links the rest,
   including the environment facts that previously lived only in scratch.
   `docs/conventions/docs.md`, `docs/conventions/git.md` and
   `docs/security/secrets.md` were copied in verbatim because the routing table
   names them.
2. **The gate reaches the whole tree.** [T-1201](gate.md). Checks 11 to 14 of
   `scripts/check-todo.py` resolve citations and links across every tracked file
   this project wrote, against `git ls-files` rather than the filesystem, and
   check 14 resolves a **bare** path with no line number, which is the shape the
   defect that opened issue 6 actually took. Before it, `tree_citations` was 1
   and bare path citations were an unchecked class. ⚠ The counts move with every
   commit that adds a sentence, so the current reading is whatever
   `./scripts/check-todo.py` prints and is not copied into an entry.
3. **The gate's checks are planted against.** [T-1202](gate.md).
   `scripts/plant.sh` breaks each check and asserts it goes red **with that
   check's own message**. Its first run found a case that landed its mutation
   and never reached its subject.
4. **Four measurements**, in `experiments/`, all committed with their results:
   `80-interposer-abi.sh`, `90-nsswitch-contract.sh`,
   `100-interpose-symbols.sh` and `125-across-distributions.sh`.
5. **Two entries added** and four amended in place: [T-0709](interpose.md) and
   [T-0410](complete.md) are new; [T-0701](interpose.md),
   [T-0702](interpose.md), [T-0703](interpose.md) and [T-0404](complete.md)
   took corrections underneath their premises.

### What the six issues claimed, and what the tree says

⭐ Nothing here was adopted on the filer's word. Each row names the run that
settled it.

| claim | verdict |
| --- | --- |
| The gate exits 0 on an empty `TODO/` | ❌ It already exited 1 on zero rows. ✅ The vacuity underneath was real: checks 7 to 9 could examine nothing and report success. Check 15 now refuses a run where any counter is zero |
| Citations should resolve against `git ls-files`, not the disk | ✅ Adopted. Nothing was untracked, and the check could not have told |
| Citations in source comments and the README go unchecked | ✅ True. The scan read `TODO/` alone. T-1201 |
| `git checkout --` restores a staged plant | ✅ Reproduced in a scratch repository: the plant survived |
| All eleven pinned digests resolve | ✅ 11 of 11 through `docker manifest inspect`, and 11 of 11 pulled and ran |
| `execvp`, `execl`, `execlp`, `posix_spawn`, `posix_spawnp` bypass an interposed `execve` | ✅ Measured, with the control arm: 1 of 7 caught, 7 of 7 with each defined |
| `libglib`, four `libpython3`, `libdbus-1`, `libarchive`, `libmagic`, `libsystemd` import them | ✅ All present on this host. ⚠ The counts differ (14, 12, 7 here) because the package set differs |
| `libpython3.*` import `stat64` and not `stat` | ✅ All four, only the `64` names at `GLIBC_2.33` |
| The recommended symbol set is missing from podbox's adopted mechanism | ❌ pathmap defines all of it. The names it omits take a descriptor or a shell string, so there is no path to rewrite |
| A shipped `libc.so.6` is stripped and needs `.dynsym` | ✅ `.symtab` 0, `.dynsym` 3136 |
| A newer-glibc object is refused by an older payload's loader | ✅ Predicted from ELF, then confirmed: the loader named `GLIBC_2.34` |
| 253 gconv modules record `DT_NEEDED libc.so.6` | ✅ 253 of 253 |
| A static glibc binary loads a host NSS module on 5 of 11 | ❌ Not reproduced. No container image dlopened anything, because they ship `passwd: files`. ⭐ The finding underneath is real and sharper: a supplied `/etc/passwd` is only read when `nsswitch` names `files`. T-0410 |
| The process dies on 2 of 11 | ⚠ Partly. 1 of 11: SIGFPE on `opensuse-leap-15.6`, from a probe built on glibc 2.39 |
| Issue 6's report that `TODO/` and `check-todo.sh` were absent | ✅ True at `48670bb`, and three commits stale when filed. `TODO/` landed in `ee16a99` and the script is `.py` |

### Two open questions this session closed

- ⭐ **The cross-libc load, both directions, both controls.** A musl-linked
  object does not load into a glibc payload and a glibc-linked one does not load
  into a musl payload. The mechanism is neither symbol versioning nor struct
  layout: it is the **SONAME**. musl's libc declares none, so an object linked
  against it records `libc.so`, and on a glibc host that path is a GNU ld
  script. The loader stops at the ELF header. That makes the discriminator one
  `readelf` away with nothing run, which [T-0709](interpose.md) turns into a
  selection rather than an attempt. [T-0702](interpose.md) is no longer blocked.
- ⭐ **A measurement taken on one host is a property of that host.**
  [T-1203](gate.md) is the runner, and its first eleven-row reading found six
  distinct `nsswitch` shapes and one distribution where a static glibc binary
  does not run at all.

### Six places where the corpus disagreed with the specification

⭐ Each is a disagreement between a claim and the code, checked at the captured
commit. Per `docs/methodology/references.md`, the disagreement is the finding.

| claim | what the tree says |
| --- | --- |
| `TOOL.md` section 3.5: `userland-execve` reaches this project as a fork to vendor | There is no such fork. `references/VHSgunzo__userland-execve/PROVENANCE.md` records a 404 beside a reachable control, and `references/VHSgunzo__ulexec/tree/Cargo.toml:29` depends on the original, `io12/userland-execve-rust`, whose MIT licence is unambiguous |
| `TOOL.md` section 7: pathmap has "129 interposed entry points" | 129 is the exported symbol count. `nm -D --defined-only` on a local build gives 113 libc entry points and 16 internals. T-0703 |
| `TOOL.md` section 0.5: `memfd-exec`'s `Cargo.toml` names a licence with no licence file | Correct, and its **upstream** `novafacing/memfd-exec` has no licence file either, so the gap is not the fork's. Both are unresolved and neither is vendored. T-0909 |
| `paper_final.md` section 6: podman's layer application has no path here | `references/containers__storage/tree/pkg/archive/archive.go:804-808` carries `ignoreChownErrors`, reachable from `storage.conf` for both drivers. Combined with F6's measured `vfs` initialization, a configuration nobody has run may clear it. T-0205, and it is stated as untested |
| `TOOL.md` section 6.7: the interposer is extracted to the store on first use | The store is outside the chroot, so an absolute `LD_PRELOAD` naming it does not resolve inside. The upstream maintainer's ruling in `references/fritzw__ld-preload-open`'s tracker is that the path must be absolute; podbox therefore places the object inside the rootfs. T-0702, T-1002 |
| `TOOL.md` section 6.7: no allocation and no locks on an interposed path | `references/pkgforge-dev__cross-libc-dlopen/tree/src/cross-libc-dlopen.c:585-598` holds one and writes down what makes it safe. The rule that survives is **no re-entry under the lock**. T-0701 |

### Three things measured that the specification does not carry

- **Struct layout is not the only cross-libc ceiling, and it is not the binding
  one.** `references/pkgforge-dev__cross-libc-dlopen/tree/docs/limits.md:20`
  records that `regoff_t` is 4 bytes on glibc and 8 on musl. That is true and it
  is reached later than the SONAME, which stops the load outright. Both push to
  the same conclusion: one interposer object per libc. T-0702, T-0709.
- **`supervise` on this runtime is unsafe as well as crippled.**
  `references/multikernel__sandlock` issue #27 establishes that the only known
  fix for seccomp-notify's TOCTOU race needs `PTRACE_SEIZE` on every thread of
  every process in the sandbox, and `ptrace` is filtered here. So the tier has
  no read channel **and** no way to be made race-safe if it had one. T-0606.
- **`podbox-complete` needs `/etc/nsswitch.conf` as much as `/etc/passwd`.**
  Supplying the second without the first is a shim that silently does nothing on
  a glibc rootfs naming another service, and the failure surfaces as a user that
  does not exist. T-0410.

## The work order

⭐ **This is the only work order.** Do not take one from the index or from a
kickoff prompt.

1. **M0, the probe.** [T-1101](milestones.md), and under it
   [T-0101](probe.md), [T-0102](probe.md), [T-0109](probe.md),
   [T-0105](probe.md), [T-0104](probe.md), [T-0103](probe.md),
   [T-0106](probe.md), then [T-0107](probe.md), [T-0108](probe.md),
   [T-0110](probe.md).
   Everything downstream branches on it, and it is the component the prior art
   most consistently gets wrong.
2. **T-0910**, the `cargo bloat` baseline wired into the gate. It is small, and
   every dependency decision after it is measured against it.
3. **M1, image acquisition.** [T-1102](milestones.md) and
   [image.md](image.md), with [T-0905](deps.md) and [T-0906](deps.md) measured
   before either lands.
4. **M2, extraction.** [T-1103](milestones.md) and [extract.md](extract.md).
   The highest-risk component, and three of its four acceptance criteria are
   already measured.
5. **M3, `run`.** [T-1104](milestones.md), [enter.md](enter.md),
   [cli.md](cli.md).
6. Then M4, M5, M6, M7 in order. [T-0709](interpose.md) and
   [T-0410](complete.md) are both P0 and both land inside M6 and M5
   respectively; neither needs a measurement that has not been taken.

⚠ [T-0205](image.md) and [T-1004](packaging.md) are P3 and are not in the order.
They are worth doing when something else touches the same ground.

## In progress

Nothing. This session opened no work it did not finish.

## Open questions for the operator

1. **`/dev/ptmx` on the target.** One `stat("/dev/ptmx")` settles whether `-t`
   can ever work, and nobody has run one. The corpus disagrees with itself: the
   mount table shows six device nodes and no `ptmx`, and an earlier account
   asserts the host `/dev/ptmx` works and published no capture. podbox probes
   and refuses by name until it is settled. [T-0503](enter.md).
2. **A musl cross toolchain carrying its own `libgcc_s`**, to build podbox's own
   Rust cdylib against musl. `musl-tools` is installed here and is not enough:
   rustc passes `-lgcc_s` on that target even under `panic = "abort"`, and
   `musl-tools` ships no musl-linked `libgcc_s.so.1`.
   `experiments/60-interposer-libc.sh` names the shortage and exits 2 on it.
   ⚠ This no longer blocks any design decision: `experiments/80-interposer-abi.sh`
   answered the question that mattered using the C reference interposer.
3. **A kernel with Landlock**, to run the three M-mechanism rows of
   `experiments/30-attribution-census.sh`. Any distro kernel has it. This host
   does not, so those rows report `SKIP` and the script exits 2.
4. **Whether `podbox` should refuse to install itself as `docker` where a
   working docker daemon exists.** [T-0803](cli.md) proposes refusing without an
   explicit flag. A machine with a working daemon is a machine where podbox is
   the wrong tool, and the alternative reading is that podbox should defer to it
   transparently. The entry carries the recommendation and not a ruling.
5. **Why a static glibc binary takes SIGFPE on `opensuse-leap-15.6`.**
   Measured by `experiments/125-across-distributions.sh` and recorded as a
   reading, not a diagnosis. It bears on what podbox may assume about payloads
   in an image, and the row is also the one distribution whose `nsswitch: compat`
   setting is therefore untested for the passwd question.
