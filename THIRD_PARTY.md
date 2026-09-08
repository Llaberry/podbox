# Third-party material

podbox itself is [0BSD](LICENSE). ⚠ **0BSD is the licence on this project's own
work. It does not relicense anybody else's.** Everything below keeps its own
licence and its own notices.

`TODO/reference-map.md` carries the per-tree determination, the commit each was
read at, and what may be done with each. This file is the notice.

## Copied into this tree

Three things are copied rather than referenced, and all three are 0BSD, so
their terms and this project's are the same.

| what | from | licence | notice |
| --- | --- | --- | --- |
| `docs/` | [`Azathothas/TEMPLATE`](https://github.com/Azathothas/TEMPLATE) at `6206166`, by way of [`Azathothas/container-research`](https://github.com/Azathothas/container-research) at `0f155e3` | 0BSD | `references/Azathothas__TEMPLATE/tree/LICENSE` |
| `scripts/common/mine-repo.sh`, `check-one-home.sh` | `Azathothas/TEMPLATE` at `6206166` | 0BSD | same |
| `scripts/common/check-markers.sh` ⚠ **modified** | `Azathothas/TEMPLATE` at `6206166` | 0BSD | same |
| the seeded experiment scripts numbered 10 through 50, `Dockerfile.target`, `targetfs.sh`, `src/` | `Azathothas/container-research` at `0f155e3` | 0BSD | `references/Azathothas__container-research/tree/LICENSE` |

⚠ **One of those files is modified**, and it is marked as such above.
`scripts/common/check-markers.sh` excludes `references/` and `docs/`, both of
which are verbatim third-party text this project's prose conventions do not
reach. The reason and a reproduction that runs the pristine upstream copy out of
this repository's own corpus are in the file, at the patch note above
`TRACKED PLUS UNTRACKED-BUT-NOT-IGNORED`.

⚠ `experiments/results/` holds **this project's own** measurements only. The
research repository's captures were not copied into it: they stay at
`references/Azathothas__container-research/tree/experiments/results/` and
`.../verification/`, under their own conditions.

## Redistributed under `references/`

⛔ **The corpus is redistributed, not merely linked**, because
`docs/methodology/references.md` section 4 requires a project that ships code to
keep its corpus, and `scripts/check-todo.py` resolves every cited line into it.

Each tree is an unmodified copy at the commit its `PROVENANCE.md` records, with
its own licence file intact, except where that file is absent upstream. Nothing
under `references/` is compiled into podbox, and nothing there is on a code path.

Some trees were trimmed by **deleting** directories that carry no citation
(vendored dependency trees, test corpora, one binary tarball).
`TODO/reference-map.md` lists every deletion and why. ⚠ No licence file, notice
file or copyright header was removed by any trim, and each trimmed tree still
carries its top-level licence.

| licence | trees |
| --- | --- |
| MIT | `VHSgunzo__pathmap`, `fritzw__ld-preload-open`, `qaidvoid__onelf`, `io12__userland-execve-rust`, `VHSgunzo__ulexec`, `pkgforge-dev__cross-libc-dlopen`, `RuriOSS__ruri`, `RuriOSS__rurima`, `VHSgunzo__sharun`, `VHSgunzo__runimage`, `Azathothas__bit-cli` |
| Apache-2.0 | `indigo-dc__udocker`, `compforge__pathshim`, `multikernel__sandlock`, `containers__storage` (with `NOTICE`), `containers__podman` |
| 0BSD | `Azathothas__TEMPLATE`, `Azathothas__container-research` |
| BSD-3-Clause and others, per its own file | `apptainer__apptainer` (`tree/LICENSE.md` enumerates them) |
| split MIT and GPL-3.0 | `mhx__dwarfs` (`tree/LICENSE`: the read path is MIT, the write path is GPL-3.0) |
| LGPL-2.1 | `containers__bubblewrap`, `dex4er__fakechroot` |
| GPL-2.0 | `proot-me__proot` |
| GPL-3.0 | `89luca89__lilipod`, `garywill__treesandbox`, `salsa-debian__fakeroot` |
| ⚠ declared in `Cargo.toml` only, no licence file in the tree | `VHSgunzo__memfd-exec`, `novafacing__memfd-exec` |
| ⚠ none found | `ylang-ylang__dockless` |
| n/a, the repository does not exist | `VHSgunzo__userland-execve` (a `PROVENANCE.md` recording the 404, and nothing else) |

## What is not vendored, and why

⛔ **Nothing is vendored yet.** `Cargo.toml`'s `[workspace.dependencies]` is
empty, and every candidate is an entry in `TODO/deps.md` that closes with a
measured size delta rather than a `cargo add`.

Three trees are **blocked from vendoring on their licence** and
`TODO/reference-map.md` says what would clear each:

- `VHSgunzo__memfd-exec` and `novafacing__memfd-exec`: the only MIT statement in
  either tree is the `license` key in `Cargo.toml`, with no licence file in
  either and no classification by GitHub.
- `ylang-ylang__dockless`: no licence file, no manifest key, no statement in any
  document. Under default copyright there is no permission to copy. Its CLI
  posture is adopted as a design, in podbox's own words, and no line is copied.

⛔ **A copyleft tree is read and never copied.** `bubblewrap`, `fakechroot`,
`fakeroot`, `proot`, `lilipod` and `treesandbox` are in the corpus because their
mechanisms are worth describing. Every description of them in `TODO/` is written
from reading, cites a file and a line, and copies nothing.
