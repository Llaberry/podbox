# SUMMARY

⭐ **The session record, saved beside [PROGRESS.md](PROGRESS.md).** PROGRESS.md
is the work order and carries no history; this is the one table a reader wants
when they ask what a session actually moved.

⛔ Every number here was re-derived after the last commit of the session it
describes. `docs/methodology/reviews.md` lens 3: a summary is a claim like any
other.

---

## Session of 2026-09-09, on `main`

**M3 landed, podbox stopped being one architecture, and two questions that had
been open since M0 were answered by building the answer.**

| what | before | after |
| --- | --- | --- |
| `origin/main` | M0's tip, carrying neither M1 nor M2 | everything, both branches consolidated |
| `podbox run` | did not exist | 7 clauses green, `chroot` rung inside the reconstruction |
| architectures the workspace compiles for | **1** | **6**, and one blocked and named |
| a `linux/arm64` image on this amd64 host | `Exec format error` | runs; `uname -m` says `aarch64` |
| records one store holds for one tag | 1 | one per platform |
| a registry with no certificate | unreachable | `--insecure-registry`, `--tls-verify=false`, every use disclosed |
| the `kcmp(2)` control | `ENOSYS`, could not answer | **`ESRCH`** in a QEMU guest, which is the target's answer |
| `landlock_create_ruleset` | `denied ENOSYS` | **`ok`, ABI 6**, in the same guest |
| entries | 96: 52 open, 5 partial, 2 blocked, 37 done | **104: 47 open, 3 partial, 2 blocked, 52 done** |
| tests | 175, red about 1 run in 3 | **212, 0 failed** |
| plant cases | 20 caught | **21 caught, 0 missed** |
| the shipping binary | 2,282,224 bytes | 2,360,176 bytes, `PT_INTERP` still 0 |
| experiments | 20 | **25** |

### The defects this session found in its own tree

| found by | what |
| --- | --- |
| building [T-0211](image.md)'s own fix | `O_CLOEXEC` is close-on-**exec**; there is no close-on-**fork**, so the entry's stated fix could not work |
| the door sweep | [T-0212](image.md) guarded `find_for` and left `find_one` open, so `extract` silently unpacked the wrong architecture |
| `scripts/dev.sh check`, first run | [T-0113](probe.md): two probes in one process took each other's scratch name and `/tmp` read as unwritable |
| running `podbox run` | `Store::find_for` short-circuited on a single hit and answered an `--platform arm64` request with the amd64 record |
| the guard-mutation lens | **the mutation harness itself** read `test result` with `head -1`, taking a suite that had run zero tests |
| reading `tls.rs` | it cited a rule in `remote-ops.md` that is not written there |
| `experiments/280-insecure-registry.sh` | a loopback registry was reached **through the proxy**, and `ureq` 2 ignores `NO_PROXY` |
| the cold pass | `dev.sh --help` created its state directory: a read that writes |

### What is not done, and is named

| | |
| --- | --- |
| [T-1104](milestones.md) is `partial` | [T-0505](enter.md) `exec`, [T-0801](cli.md) the parity table, [T-0803](cli.md) the `docker` name |
| [T-0506](enter.md) is `partial` | a probe inside a `qemu-user` container measures the emulator and is not yet labelled as such |
| `powerpc64le` | blocked on `syscalls` 0.8.1's nightly gate, which is stale; the experiment goes red when it clears |
| two `claude/*` branches | zero unique commits, and this session's proxy refuses every delete |
