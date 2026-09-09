# SUMMARY

⭐ **The session record, saved beside [PROGRESS.md](PROGRESS.md).** PROGRESS.md
is the work order and carries no history; this is the one table a reader wants
when they ask what a session actually moved.

⛔ Every number here was re-derived after the last commit of the session it
describes. `docs/methodology/reviews.md` lens 3: a summary is a claim like any
other.

---

## Session of 2026-09-09, on `main` (second)

**M3 and M4 both closed, CI went from nine red runs to green, and the milestone
that failed three times is the one worth reading about.**

| what | before | after |
| --- | --- | --- |
| CI on `main` | **9 consecutive red runs** | green |
| `podbox exec` | did not exist | a fresh chroot, and three channels say so |
| the verb and flag parity table | prose in a specification | **131 rows as data**, and the parsers ask it |
| `docker` and `podman` on PATH | not answered to | answered to, with the banner naming which |
| a probe under `qemu-user` | reported qemu's rung as the machine's | `emulated: true`, and the cache is keyed on the interpreter |
| the store's concurrency contract | unwritten | **7 invariants**, driven by 8 real processes |
| the container lifecycle | did not exist | **20 of 20 consecutive passes**, no sleep anywhere |
| entries | 104: 47 open, 3 partial, 2 blocked, 52 done | **106: 37 open, 1 partial, 2 blocked, 66 done** |
| tests | 212 | **251**, 0 failed |
| plant cases | 21 caught | **23 caught, 0 missed** |
| experiments | 25 | **27** |

### The defects this session found in its own tree

| found by | what |
| --- | --- |
| reading nine CI logs | `.cargo/config.toml` and the workflow both declared the toolchain, and the merge that brought `rustls` made the second one short by a word |
| ⭐ the door sweep, after three failing acceptance runs | `stop` connects to its launcher TWICE, and a container that stopped fast tore the socket down in between, so the fastest success read as "not running" |
| writing invariant I4 down | `rmi` and `prune` asked `in_use` OUTSIDE the index lock, so a `run` taking its hold in between kept its lock and lost its blobs |
| the OOM killer | `std::fs::read("/dev/urandom")` has no EOF; `podbox create` allocated 13 GB |
| a 300-second `run -d` | a readiness pipe without `O_CLOEXEC` is inherited through the payload's `execve` |
| a SIGTERMed payload reporting 128 | `si_status` is at byte 24 of a `siginfo_t`, not 20, which is `si_uid` |
| the test that asserts a refusal says why | eleven parity rows whose whole reason was "the lifecycle is M4" |
| `experiments/results/multiarch.txt` differing | clause 4's reading depended on unstated binfmt state and flipped between two rungs |
| a report full of docker's help | a backtick inside double quotes is command substitution, in two `say` lines |
| a clause reporting a correct refusal as a defect | `timeout X cmd &` makes `$!` the pid of `timeout` |
| `320-cli-contract.sh` going red the day M4 landed | two clauses named verbs and a flag BY HAND that the parity table already carries; they read them out of the table now |
| running the suite five times | T-0603's own test counted threads under a threaded harness, and then matched its own assertion line |

### What the four review passes found

1. ⭐ **The door sweep. It closed M4.** Enumerating every door to the control
   socket found the double connect in `stop`, and disproved both causes that had
   been written down from memory.
2. **The guard mutation.** `./scripts/plant.sh`: 23 caught, 0 missed, 3 controls
   quiet. Check 19's two new cases each went red with their own message, and
   writing them found that a literal build-tool name in the plant harness makes
   check 19 report the toolchain-free `todo` job as needing a toolchain. ⭐ The
   same lens caught T-0603's own test: it counted threads before and after and
   failed about one run in five, which is a test whose name claimed more than it
   checked, and its replacement then matched its own assertion line.
3. **The claim audit.** Every number in `PROGRESS.md` re-derived after the last
   commit, and two tracked readings were carrying per-run values (a binfmt
   registration's pid, a launcher's pid and a timestamp) that could never
   reproduce.
4. **What the driven pass showed that the suite could not.** The suite was green
   through every one of the three failing lifecycle runs, the 13 GB allocation
   and the 300-second `run -d`. None of them is reachable from a unit test.

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
