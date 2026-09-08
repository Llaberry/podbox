# Reference map

The corpus, and what may be done with each tree.

Per `docs/methodology/references.md` section 4 the shape chosen here is
**tracked, in the tree**: every reference lives under `references/<owner>__<repo>/`
on `main`, at a pinned commit, with its tracker beside it.

```sh
ls references/                                  # the corpus
cat references/VHSgunzo__pathmap/PROVENANCE.md  # commit, route, and gaps
```

That shape is chosen over the side branch for one reason:
`scripts/check-todo.py` asserts that **every cited path and line resolves**, and
a gate cannot resolve a citation into a branch it is not on. The cost is a
157 MB clone.

Each directory holds `PROVENANCE.md` (the commit, the route, and what could not
be fetched), `api/` (issues and pull requests in both states, comments, review
comments, releases, tags), and `tree/` (the source at that commit, with `.git`
already stripped and the commit captured first).

⛔ **Cite the commit beside every line reference.** The row below carries it.

## Licence, resolved before use

⛔ **The determination is made before the tree is used, not after.** podbox is
0BSD. A permissive tree may be vendored with its own notice retained; a copyleft
tree may be **read** and its mechanism described, and no line of it may be
copied into this repository.

| Tree | Commit | Licence | Where the determination came from | What may be done |
| --- | --- | --- | --- | --- |
| `references/VHSgunzo__pathmap` | `98b3d2a` | MIT | `tree/LICENSE`, and `api/repo.json` `.license.spdx_id` | vendor and patch |
| `references/fritzw__ld-preload-open` | `422b2bf` | MIT | `tree/LICENSE`, `api/repo.json` | vendor and patch |
| `references/qaidvoid__onelf` | `158b4af` | MIT | `tree/LICENSE`, `api/repo.json` | vendor and patch |
| `references/io12__userland-execve-rust` | `02ef0e0` | MIT | `tree/LICENSE`, `tree/Cargo.toml:8`, `api/repo.json` | vendor and patch |
| `references/VHSgunzo__ulexec` | `00934f8` | MIT | `tree/LICENSE`, `api/repo.json` | vendor and patch |
| `references/pkgforge-dev__cross-libc-dlopen` | `34482c7` | MIT | `tree/LICENSE`, `api/repo.json` | vendor and patch |
| `references/RuriOSS__ruri` | `711673a` | MIT | `tree/LICENSE`, `api/repo.json` | read; mechanism only |
| `references/RuriOSS__rurima` | `30a0637` | MIT | `tree/LICENSE`, `api/repo.json` | read; mechanism only |
| `references/VHSgunzo__sharun` | `b1ef744` | MIT | `tree/LICENSE`, `api/repo.json` | read; mechanism only |
| `references/VHSgunzo__runimage` | `f1512f2` | MIT | `tree/LICENSE`, `api/repo.json` | read; mechanism only |
| `references/Azathothas__bit-cli` | `cce8131` | MIT | `tree/LICENSE`, `api/repo.json` | read; the work model |
| `references/indigo-dc__udocker` | `638bc42` | Apache-2.0 | `tree/LICENSE`, `api/repo.json` | vendor with notice; mechanism adopted |
| `references/compforge__pathshim` | `8bcc34e` | Apache-2.0 | `tree/LICENSE`, `api/repo.json` | vendor with notice; protocol adopted |
| `references/multikernel__sandlock` | `841265d` | Apache-2.0 | `tree/LICENSE`, `api/repo.json` | read; kept as an exhibit |
| `references/containers__storage` | `83cf574` | Apache-2.0 | `tree/LICENSE`, `tree/NOTICE`, `api/repo.json` | read; mechanism only |
| `references/containers__podman` | `7d39ce8` | Apache-2.0 | `tree/LICENSE`, `api/repo.json` | read; mechanism only |
| `references/Azathothas__TEMPLATE` | `6206166` | 0BSD | `tree/LICENSE`, `api/repo.json` | copied verbatim into `docs/` and `scripts/common/` |
| `references/Azathothas__container-research` | `0f155e3` | 0BSD | `tree/LICENSE`, `api/repo.json` | copied: `experiments/` seeded from it |
| `references/apptainer__apptainer` | `6099bb1` | BSD-3-Clause plus others | `tree/LICENSE.md`, which says "Apptainer is subject to the Licenses detailed below" and enumerates several. `api/repo.json` reports `NOASSERTION` | read only, and per-file if that ever changes |
| `references/mhx__dwarfs` | `9062b57` | ⚠ **split: MIT and GPL-3.0** | `tree/LICENSE`: the code that **reads** a DwarFS image is MIT, the code that **writes** one is GPL-3.0 | read only. A copy would have to be justified file by file, and none is planned |
| `references/containers__bubblewrap` | `26bb788` | LGPL-2.1 | `tree/COPYING` | ⛔ read only |
| `references/dex4er__fakechroot` | `b42d1fb` | LGPL-2.1 or later | `tree/COPYING`: "fakechroot is distributed under the GNU Lesser General Public License (LGPL 2.1 or greater)" | ⛔ read only. The model is adopted; no line is copied |
| `references/salsa-debian__fakeroot` | `860de25` | GPL-3.0 | `tree/COPYING` | ⛔ read only. The model is adopted; no line is copied |
| `references/proot-me__proot` | `65f3f7d` | GPL-2.0 | `tree/COPYING` | ⛔ read only |
| `references/89luca89__lilipod` | `872755a` | GPL-3.0 | `tree/COPYING.md`, `api/repo.json` | ⛔ read only. Refused as a seed on language grounds before the licence was read; the licence settles it independently |
| `references/garywill__treesandbox` | `71acbee` | GPL-3.0 | `tree/LICENSE`, `api/repo.json` | ⛔ read only |
| `references/VHSgunzo__memfd-exec` | `9708cb7` | ⚠ **unresolved** | `tree/Cargo.toml:6` says `license = "MIT"`. **No licence file anywhere in the tree.** `api/repo.json` `.license.spdx_id` is null | ⛔ **do not vendor until resolved.** See below |
| `references/novafacing__memfd-exec` | `0a15efe` | ⚠ **unresolved** | `tree/Cargo.toml:5` says `license = "MIT"`. **No licence file either.** `api/repo.json` `.license.spdx_id` is null | ⛔ **do not vendor until resolved.** See below |
| `references/ylang-ylang__dockless` | `ed35b5d` | ⚠ **none found** | No licence file, no manifest key, and no statement in `tree/README.md` | ⛔ read only. The CLI posture is adopted as a design; no line is copied |
| `references/VHSgunzo__userland-execve` | none | n/a | **The repository does not exist.** `PROVENANCE.md` records the 404 beside a reachable control | nothing. See below |

### The three that are not resolved, and what would resolve them

⛔ **None of these is "probably fine", and none is vendored in this state.**

1. **`memfd-exec`, both the fork and its upstream.** The only MIT statement in
   either tree is the `license` key in `Cargo.toml`. There is no licence file
   and GitHub classifies neither. A manifest key is a declaration by the
   author and it is what crates.io publishes under, which is weaker evidence
   than a licence file and is not nothing. **What would resolve it:** a licence
   file appearing in either tree at a later commit. Until then `T-0909` carries
   the blocker and the memfd rung is written against `io12/userland-execve-rust`,
   whose MIT is unambiguous, plus podbox's own `memfd_create` and `fexecve`
   calls, which are three syscalls.

2. **`dockless` has no licence statement of any kind.** Under default copyright
   that means no permission to copy. Its value here is a **posture**, which
   `T-0801` records as a design decision in podbox's own words.

3. **`VHSgunzo/userland-execve` does not exist.** `TOOL.md` section 3.5 calls it "the
   second fork to vendor". `references/VHSgunzo__userland-execve/PROVENANCE.md`
   records the 404 with a reachable control beside it, and
   `references/VHSgunzo__ulexec/tree/Cargo.toml:29` shows that `ulexec` depends
   on the original crate, `userland-execve = "0.2.0"`, whose repository
   crates.io gives as `io12/userland-execve-rust`. That tree is in the corpus
   and its licence is clean.

## Verdicts

Per `docs/methodology/references.md`, exactly one per reference.

| Tree | Verdict | What transfers, and where it is written down |
| --- | --- | --- |
| `references/VHSgunzo__pathmap` | **adopt (vendor and patch)** | The path half of the interposer. [interpose.md](interpose.md) T-0703, T-0705, T-0707 |
| `references/fritzw__ld-preload-open` | **adopt (rulings)** | pathmap's upstream. Its tracker carries the absolute-`LD_PRELOAD` ruling and the musl build recipe. [interpose.md](interpose.md) T-0702 |
| `references/salsa-debian__fakeroot` | **adopt (model)** | The ownership half, and the errno test that is wrong here. [interpose.md](interpose.md) T-0704 |
| `references/dex4er__fakechroot` | **adopt (model)** | The path half as a per-entry-point library, and the exclude list. [interpose.md](interpose.md) T-0707 |
| `references/pkgforge-dev__cross-libc-dlopen` | **adopt (mechanism)** | Cross-libc loading, the struct-layout ceiling, the lock rule, the preload ordering requirement. [interpose.md](interpose.md) T-0701, T-0702 |
| `references/indigo-dc__udocker` | **adopt (mechanism), confirms (design)** | Ownership-neutral extraction, the inter-layer re-permission pass, the per-container mode selector. [extract.md](extract.md) T-0302, T-0303, T-0306; [probe.md](probe.md) T-0107 |
| `references/RuriOSS__ruri` | **adopt (discipline)** | Probe-then-refuse with a named floor, and one switch that turns every degradation into a refusal. [probe.md](probe.md) T-0107; [cli.md](cli.md) T-0804 |
| `references/compforge__pathshim` | **adopt (protocol)** | The probe's three-channel contract and the disposable-child verdict. [probe.md](probe.md) T-0101, T-0110 |
| `references/qaidvoid__onelf` | **adopt (packaging)** | The launch ladder, the separate request and result variables, the lock held through exec, the lexical symlink check. [packaging.md](packaging.md) T-1001, T-1002; [extract.md](extract.md) T-0304 |
| `references/io12__userland-execve-rust` | **adopt (vendor and patch)** | Userland exec, and the stack-size costing its tracker carries. [packaging.md](packaging.md) T-1003 |
| `references/VHSgunzo__ulexec` | **adopt (mechanism)** | The two rungs driven from one tool. [packaging.md](packaging.md) T-1003 |
| `references/VHSgunzo__memfd-exec` | **adopt (mechanism), blocked on licence** | The memfd rung, its probe-before-use, and a regression to avoid. [deps.md](deps.md) T-0909 |
| `references/novafacing__memfd-exec` | **confirms** | The upstream the fork's regression is measured against. [deps.md](deps.md) T-0909 |
| `references/multikernel__sandlock` | **anti-pattern exhibit** | The complete chain from a filtered read to a false report, and its own audit's blind spot. [supervise.md](supervise.md) T-0606 |
| `references/containers__storage` | **confirms** | Wall 1 at the exact line, and the configuration option that clears it. [extract.md](extract.md) T-0302; [image.md](image.md) T-0205 |
| `references/containers__podman` | **confirms** | Where the layer applier is reached from. [image.md](image.md) T-0205 |
| `references/containers__bubblewrap` | **confirms** | The witness that separates a refused clone from a refused mount. [probe.md](probe.md) T-0102 |
| `references/apptainer__apptainer` | **confirms** | A fourth tool at wall 1, in Go, so interposition cannot rescue it. [interpose.md](interpose.md) T-0706 |
| `references/89luca89__lilipod` | **refused as a seed; lessons adopted** | The dependency precheck, the `Credential` shape, and the fabricated-root `exec` bug. [enter.md](enter.md) T-0502; [cli.md](cli.md) T-0806 |
| `references/proot-me__proot` | **refused** | `ptrace` is filtered, and its own diagnostic misdirects. [probe.md](probe.md) T-0101; [interpose.md](interpose.md) T-0706 |
| `references/RuriOSS__rurima` | **refused (pull)** | `tar -xpf` as root, which is wall 1 again. [extract.md](extract.md) T-0301 |
| `references/ylang-ylang__dockless` | **adopt (CLI posture), refused (engine)** | Fail-fast guardrails that name the incident behind each. [cli.md](cli.md) T-0801, T-0804 |
| `references/garywill__treesandbox` | **refused** | It dies at `getpwuid(0)` before any namespace call. [complete.md](complete.md) T-0404 |
| `references/mhx__dwarfs` | **confirms** | `short write: -20` is a libarchive status and not an errno. [image.md](image.md) T-0203 |
| `references/VHSgunzo__sharun` | **filed elsewhere** | It solves relocation, not filesystem virtualization. Nothing in podbox owns it today; [packaging.md](packaging.md) T-1003 names where it would land |
| `references/VHSgunzo__runimage` | **filed elsewhere** | The launcher whose failure is bubblewrap's, already carried by that row |
| `references/Azathothas__container-research` | **adopt (the specification)** | `TOOL.md` and `paper_final.md` are the source of every entry here |
| `references/Azathothas__TEMPLATE` | **adopt (the methodology)** | `docs/` is copied from it verbatim and binds |
| `references/Azathothas__bit-cli` | **adopt (the work model)** | The shape of `INDEX.md`, `RULES.md`, `PROGRESS.md` and this file |
| `references/VHSgunzo__userland-execve` | **refused (does not exist)** | Recorded so no session re-derives the 404 |

## What was deleted, and why

⛔ **Trimmed by deleting, never by moving**, per
`docs/methodology/references.md` section 1: a trim that rewrites paths
invalidates every citation already written.

| Tree | Deleted | Why |
| --- | --- | --- |
| `references/containers__podman` | `tree/vendor`, `tree/test` | 104 MB of vendored Go modules and test data. podman at `7d39ce8` does not vendor `containers/storage` under `vendor/github.com/containers/storage`, so the wall-1 citation is taken against `references/containers__storage` at its own commit instead |
| `references/containers__storage` | `tree/tests`, `tree/vendor` | 24 MB. Nothing here cites either |
| `references/ylang-ylang__dockless` | `tree/vendor/udocker-englib-1.2.11.tar.gz` | a 45 MB binary tarball of udocker's fakechroot engine libraries. Nothing can be cited at a line inside a tarball. Re-fetchable from the upstream release the tree names |
| `references/89luca89__lilipod` | `tree/vendor` | 15 MB of vendored Go modules. The cited code is in `tree/pkg/` and `tree/cmd/` |
| `references/Azathothas__bit-cli` | `tree/vendor` | 11 MB. `tree/TODO/` is what this reference is here for |
| `references/mhx__dwarfs` | `tree/test` | 8.4 MB of test corpora |
| `references/apptainer__apptainer` | `tree/e2e` | 5.4 MB of end-to-end tests |

## Gaps

⛔ **A silently skipped source is the failure the procedure exists to prevent.**

- **Discussions were not fetched for any tree.** They are GraphQL only and
  `scripts/common/mine-repo.sh` reached GitHub through the credential-free REST
  proxy. Every `PROVENANCE.md` records this as its one gap. Where a project
  keeps its design argument in Discussions, this corpus does not have it.
- **`references/salsa-debian__fakeroot` has no `api/` at all.** fakeroot is not
  on GitHub; its source is a GitLab instance and its defect tracker is the
  Debian BTS, and `mine-repo.sh` speaks neither. The tracker pass of
  `docs/methodology/references.md` section 3 was **not taken** for that
  reference, and its verdict is code-only.
  `https://bugs.debian.org/cgi-bin/pkgreport.cgi?pkg=fakeroot` is where it would
  be taken.
- **`references/VHSgunzo__userland-execve` has no `tree/`.** The repository does
  not exist.
