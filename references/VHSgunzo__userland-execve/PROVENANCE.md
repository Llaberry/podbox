# VHSgunzo/userland-execve

Fetched 2026-09-08 by `scripts/common/mine-repo.sh`. **The repository does not
exist.**

| | |
| --- | --- |
| commit | `-` |
| route | proxy |
| control | reachable (pkgforge-dev/reverse-proxies answered 200) |

```
mine-repo: proxy returned 404 for repos/VHSgunzo/userland-execve
mine-repo: control says: reachable (pkgforge-dev/reverse-proxies answered 200)
```

⭐ **A 404 beside a reachable control is evidence, not a route failure.** This
directory is kept, holding that evidence, so the fetch is not re-attempted.

## What this corrects

`TOOL.md` §3.5 states that `memfd-exec` and `userland-execve` both reach this
project as forks, and calls `userland-execve` "the second fork to vendor".
The first half is correct and the second is not:

| | claim | what the fetch found |
| --- | --- | --- |
| `memfd-exec` | a fork | ⭐ **correct.** `VHSgunzo/memfd-exec` is a fork of `novafacing/memfd-exec` (`api/repo.json`, `.fork` is true, `.parent.full_name`) |
| `userland-execve` | a fork | ⛔ **no such repository.** `VHSgunzo` publishes no fork of it |

The crate `ulexec` depends on is the original:
`references/VHSgunzo__ulexec/tree/Cargo.toml:29` names `userland-execve = "0.2.0"`,
and crates.io gives that crate's repository as `io12/userland-execve-rust`.
That tree is in the corpus at `references/io12__userland-execve-rust/`.

The vendoring decision therefore rests on the original's own state, not on a
fork's. `TODO/reference-map.md` carries the determination.
