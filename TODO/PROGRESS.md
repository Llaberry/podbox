# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M-1 complete. No podbox code exists. M0, the probe, is next.**
Session of 2026-09-08, on `main`, commit range starting at the M-1 skeleton.

[INDEX.md](INDEX.md) is the list. [RULES.md](RULES.md) is how this repository is
worked on. [reference-map.md](reference-map.md) is the corpus.

## The measured baseline

One machine, one day: kernel `6.18.44-fc-v24` (a Firecracker guest), rustc
1.94.1, cargo 1.94.1, GNU tar 1.35, docker 29.3.1, 4 CPUs, 16 GB RAM.
⚠ `CONFIG_SECURITY_LANDLOCK` is unset on this kernel, so the M-mechanism rows of
the reconstruction cannot run here and report `SKIP`.

| what | value | taken by |
| --- | --- | --- |
| release binary, empty skeleton, `x86_64-unknown-linux-musl` | 389,656 bytes | `cargo build --release`, T-0910 |
| `PT_INTERP` in that binary | none. static-pie | `readelf -l`, T-1001 |
| interposer cdylib under `+crt-static` | refused by cargo | `experiments/60-interposer-libc.sh` |
| interposer cdylib, `-crt-static`, musl target | 14,064 bytes | `experiments/60-interposer-libc.sh` |
| interposer cdylib, `-crt-static`, gnu target | 267,680 bytes | `experiments/60-interposer-libc.sh` |
| cross-libc preload reach | **not measured.** exit 2, no musl libc on this host | `experiments/60-interposer-libc.sh` |
| corpus | 30 trees, 157 MB, tracked in the tree | `scripts/common/mine-repo.sh` |
| `alpine:3.20` `etc/shadow` ownership | uid 0, gid 42 | `experiments/70-whiteout-contract.sh` |
| OCI layer member-name prefix | no `./` on any layer of either pinned image | `experiments/70-whiteout-contract.sh` |
| a slash-anchored whiteout glob against a layer-root whiteout | misses it | `experiments/70-whiteout-contract.sh` |

Acceptance for M-1, run on 2026-09-08:

```
$ ./scripts/check-todo.py
check-todo: 81 rows, 81 entries, 76 open, 0 partial, 3 blocked, 2 done
check-todo: ok
$ cargo build --release --target x86_64-unknown-linux-musl
    Finished `release` profile [optimized] target(s)
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
```

## Counts

81 entries: 76 open, 0 partial, 3 blocked, 2 done.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

Milestone M-1 of `TOOL.md` section 5, in full. No podbox implementation code.

1. **The skeleton.** The workspace of section 4.3 with eight crates, all empty.
   `rust-toolchain.toml`, `.cargo/config.toml`, `LICENSE` (0BSD), `README.md`,
   `THIRD_PARTY.md`, and CI that runs the gate. `docs/` copied verbatim from
   `Azathothas/TEMPLATE` by way of `Azathothas/container-research`.
2. **The corpus.** 30 trees at pinned commits under `references/`, fetched with
   `scripts/common/mine-repo.sh`, each with its tracker in `api/` and its
   `PROVENANCE.md`. Every one read, its tracker read, and given exactly one
   verdict in [reference-map.md](reference-map.md), with a licence
   determination made before use.
3. **`TODO/`.** 81 entries across eleven categories, one per item of `TOOL.md`
   section 5 and section 6, each carrying Source, Category, Priority, Effort, Status, Problem,
   Premise, Approach, Decision and Prove, and each citing the reference to read
   at the line to read it at.
4. **The two count scripts.** `scripts/todo-count.py` writes,
   `scripts/check-todo.py` reads and is the gate.
5. **Three measurements of this project's own**, in `experiments/`, beside the
   five seeded from the research repository.

### Six places where the corpus disagreed with the specification

⭐ Each of these is a disagreement between a claim and the code, checked at the
captured commit. Per `docs/methodology/references.md`, the disagreement is the
finding.

| claim | what the tree says |
| --- | --- |
| `TOOL.md` section 3.5: `userland-execve` reaches this project as a fork to vendor | There is no such fork. `references/VHSgunzo__userland-execve/PROVENANCE.md` records a 404 beside a reachable control, and `references/VHSgunzo__ulexec/tree/Cargo.toml:29` depends on the original, `io12/userland-execve-rust`, whose MIT licence is unambiguous |
| `TOOL.md` section 7: pathmap has "129 interposed entry points" | 129 is the exported symbol count. `nm -D --defined-only` on a local build gives 113 libc entry points and 16 internals. T-0703 |
| `TOOL.md` section 0.5: `memfd-exec`'s `Cargo.toml` names a licence with no licence file | Correct, and its **upstream** `novafacing/memfd-exec` has no licence file either, so the gap is not the fork's. Both are unresolved and neither is vendored. T-0909 |
| `paper_final.md` section 6: podman's layer application has no path here | `references/containers__storage/tree/pkg/archive/archive.go:804-808` carries `ignoreChownErrors`, reachable from `storage.conf` for both drivers. Combined with F6's measured `vfs` initialization, a configuration nobody has run may clear it. T-0205, and it is stated as untested |
| `TOOL.md` section 6.7: the interposer is extracted to the store on first use | The store is outside the chroot, so an absolute `LD_PRELOAD` naming it does not resolve inside. The upstream maintainer's ruling in `references/fritzw__ld-preload-open`'s tracker is that the path must be absolute; podbox therefore places the object inside the rootfs. T-0702, T-1002 |
| `TOOL.md` section 6.7: no allocation and no locks on an interposed path | `references/pkgforge-dev__cross-libc-dlopen/tree/src/cross-libc-dlopen.c:585-598` holds one and writes down what makes it safe. The rule that survives is **no re-entry under the lock**. T-0701 |

### Two things this session measured that the specification does not carry

- **Struct layout is the cross-libc ceiling, not symbol versioning.**
  `references/pkgforge-dev__cross-libc-dlopen/tree/docs/limits.md:20`: `regoff_t`
  is 4 bytes on glibc and 8 on musl and the `FTW_*` constants are off by one,
  and "the offset is compiled into the object, so no preload can reach it". So
  podbox ships one interposer object per libc. T-0702.
- **`supervise` on this runtime is unsafe as well as crippled.**
  `references/multikernel__sandlock` issue #27 establishes that the only known
  fix for seccomp-notify's TOCTOU race needs `PTRACE_SEIZE` on every thread of
  every process in the sandbox, and `ptrace` is filtered here. So the tier has
  no read channel **and** no way to be made race-safe if it had one. T-0606.

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
6. Then M4, M5, M6, M7 in order.

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
2. **A musl toolchain, or an alpine container, to finish
   `experiments/60-interposer-libc.sh`.** It exits 2 here and names what it
   needs. The design does not depend on the answer; the measurement is what
   would let [T-0702](interpose.md) close.
3. **A kernel with Landlock**, to run the three M-mechanism rows of
   `experiments/30-attribution-census.sh`. Any distro kernel has it. This host
   does not, so those rows report `SKIP` and the script exits 2.
4. **Whether `podbox` should refuse to install itself as `docker` where a
   working docker daemon exists.** [T-0803](cli.md) proposes refusing without an
   explicit flag. A machine with a working daemon is a machine where podbox is
   the wrong tool, and the alternative reading is that podbox should defer to it
   transparently. The entry carries the recommendation and not a ruling.
