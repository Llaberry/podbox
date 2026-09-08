# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M0 complete. `podbox probe` exists, is measured in both environments,
and its acceptance runs as one command. M1, image acquisition, is next.**
Session of 2026-09-08, on `main`.

[INDEX.md](INDEX.md) is the list. [RULES.md](RULES.md) is how this repository is
worked on. [reference-map.md](reference-map.md) is the corpus.
[`../docs/AGENTS.md`](../docs/AGENTS.md) is the router: a session with no memory
reads that first and it names everything else.

## The measured baseline

One machine, one day: kernel `6.18.44-fc-v24` (a Firecracker guest), rustc
1.98.1, cargo 1.98.1, GNU tar 1.35, docker 29.3.1, glibc 2.39, musl 1.2.4,
4 CPUs, 16 GB RAM.
⚠ `CONFIG_SECURITY_LANDLOCK` is unset on this kernel, so the M-mechanism rows of
the reconstruction cannot run here and report `SKIP`.
⚠ `CONFIG_CHECKPOINT_RESTORE` is also unset, so `kcmp(2)` answers `ENOSYS`. That
is a control of the bogus-argument discriminator, and the consequence is
[T-0102](probe.md)'s correction below.

| what | value | taken by |
| --- | --- | --- |
| release binary, M0 complete, `x86_64-unknown-linux-musl` | 496,184 bytes | `experiments/110-bloat-delta.sh`, T-0910 |
| third-party crates in that binary | **0** | `cargo tree`, same script |
| release binary, empty skeleton, same target | 389,656 bytes | `cargo build --release`, T-1100 |
| `PT_INTERP` in the M0 binary | none. static-pie | `readelf -l`, T-1001 |
| `podbox probe` rung, unconfined on this host | `namespace` | `experiments/130-probe-parity.sh` |
| `podbox probe` rung, inside `experiments/20-enter-target.sh` | `chroot` | `experiments/130-probe-parity.sh` |
| attribution rows against `experiments/results/attribute.txt` | 15 matched, 1 recorded divergence, 0 differed | `experiments/130-probe-parity.sh` |
| probes in the set | 48: 32 census, 16 attribution | `podbox probe --json` |
| `kcmp(-1,-1,...)` control, on this host | `ENOSYS`: the control cannot answer | `experiments/results/attribute.txt` |
| `kcmp(-1,-1,...)` control, on the target | `ESRCH`: executed | `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26` |
| writable paths obtained inside the reconstruction | 4 of 8 probed: `/tmp`, `/dev/shm`, `/workspace`, `/state` | `podbox probe --json` |
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
| the gate's checks, planted against | 17 checks, 18 cases; see the acceptance block below | `scripts/plant.sh` |
| corpus | 30 trees, 154 MB in a fresh clone | `scripts/common/mine-repo.sh` |
| ⚠ git objects | 88 MB, up from 27 MB. See the debt below | `du -sh .git` on a fresh clone |
| `alpine:3.20` `etc/shadow` ownership | uid 0, gid 42 | `experiments/70-whiteout-contract.sh` |
| OCI layer member-name prefix | no `./` on any layer of either pinned image | `experiments/70-whiteout-contract.sh` |
| a slash-anchored whiteout glob against a layer-root whiteout | misses it | `experiments/70-whiteout-contract.sh` |

Acceptance, run on 2026-09-08:

```
$ ./scripts/check-todo.py
check-todo: 86 rows, 86 entries, 67 open, 2 partial, 2 blocked, 15 done
check-todo: ok
$ ./scripts/plant.sh
  plants   18 caught, 0 missed
  controls 3 quiet, 0 fired
$ cargo test --workspace
test result: ok. 37 passed; 0 failed
$ cargo build --release --target x86_64-unknown-linux-musl
    Finished `release` profile [optimized] target(s)
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
$ ./experiments/110-bloat-delta.sh baseline
  total_bytes 496184   headroom 7503816   third-party crates 0
$ ./experiments/130-probe-parity.sh
  got chroot / got namespace / 15 matched, 1 recorded divergence, 0 differed, 0 missing
```

## Counts

86 entries: 67 open, 2 partial, 2 blocked, 15 done.

Derived by `scripts/todo-count.py` and asserted by `scripts/check-todo.py`.
[INDEX.md](INDEX.md)'s Counts block carries the per-priority breakdown, and the
gate refuses a commit where the two disagree.

## What this session did

⭐ **M0, and it is the first podbox implementation code in the tree.**
`crates/podbox-probe` and the `probe` verb of `crates/podbox-cli`, with **zero
dependencies**: `[workspace.dependencies]` is still empty, which is the
measurement [T-0901](deps.md) asked for before a syscall crate is considered.

1. **The probe set.** [T-0101](probe.md), [T-0109](probe.md),
   [T-0106](probe.md). 48 probes, each in a freshly forked child, each
   recording the kernel's errno rather than a boolean, each verdict one of
   three states. `crates/podbox-probe/src/sys.rs` issues the syscalls directly
   so the errno is the kernel's and not a libc wrapper's.
2. **The discriminator and its controls.** [T-0102](probe.md). The four
   bogus-argument rows and both controls travel in the same run, and a control
   that cannot answer sets `controls_answered` false and says so in the
   evidence block.
3. **Creation, configuration, attachment and use, as four verdicts.**
   [T-0103](probe.md), with the `may_mount()` sentence in the output.
4. **The write allowlist by writing**, for blocks and inodes both, reporting
   the set obtained. [T-0104](probe.md).
5. **The ID maps read directly.** [T-0105](probe.md), and the two sentences
   that turn `EINVAL` and `groups=0,65534` into one explanation.
6. **Rung selection and the strictness switch.** [T-0107](probe.md), `partial`.
   **The banner.** [T-0108](probe.md), `partial`. Both are implemented and
   measured; both have a half that needs `run`, which is M3.
7. **The channel contract.** [T-0110](probe.md): the rung on stdout, the
   evidence on stderr, `--json` and `--rows` beside it, 2 for invalid input.
8. **[T-1101](milestones.md), the milestone, closed against a script**:
   `experiments/130-probe-parity.sh`.
9. **[T-0910](deps.md)**, the `cargo bloat` baseline, committed, with the
   ceiling given one home and check 17 of the gate holding it there. Three new
   plant cases.

### Three defects found in this tree, each blocking the acceptance

⭐ Verify, do not accept. Each was found by trying to run the thing rather than
by reading it.

| what was claimed | what the tree did |
| --- | --- |
| `experiments/20-enter-target.sh` runs the reconstruction | It named `$REPO/verification/{confine,probe,cprobe}`, and this tree has no `verification/`: the corpus carries it under `references/`. Every invocation died at `cd`. It now resolves `HARNESS_SRC` and prints it in the conditions block |
| `experiments/results/attribute.txt` is what the milestone compares against | It did not exist. `30-attribution-census.sh --capture experiments/results` has now produced it, with `census.txt` and `identity.txt` |
| [T-0102](probe.md)'s `Prove` asserts `.controls.kcmp == "ESRCH"` | That is one host's answer stated as every host's. `kcmp(2)` needs `CONFIG_CHECKPOINT_RESTORE`; the target answers `ESRCH` and this host answers `ENOSYS`. The rule underneath survives and is what podbox implements |

### Four things the probe reports that the Go instrument gets wrong

⭐ Each is a defect in the reference, checked against a run, and the module
header of `crates/podbox-probe/src/probes.rs` carries all four.

1. ⛔ **The bare `mount(MS_SLAVE,/)` row is not ported.** In the caller's mount
   namespace it changes the propagation of `/` for the machine, permanently.
   The same operation is measured inside `clone(CLONE_NEWNS)`, which is where
   bubblewrap performs it.
2. **A probe that mounts, unmounts.** `mount(tmpfs,/mnt)` and
   `move_mount(-> /tmp/mm-probe)` succeed unconfined and the Go instrument
   leaves both attached. A failure to remove one is reported in the row.
3. **Nothing is overwritten.** Every scratch file is `O_EXCL`; a file already
   at that path is a `skip` naming it, not a `remove` and a retry.
4. ⭐ **A failed precondition is a `skip`, never the row's denial.** Where
   `fsmount` fails, the Go instrument reports its errno as `move_mount`'s
   verdict, and `kcmp`'s `ENOSYS` as a denial. Neither operation was measured.

### What the three review passes found

⭐ Each pass asked a different question, and each found something. A pass
reporting nothing was too shallow.

1. **Is it true?** All 46 declared syscall numbers and all 19 flag constants
   were checked against `<sys/syscall.h>` and the kernel headers, and every
   reference citation in the new code was opened at its line. All matched. Two
   ABI cross-checks were added as tests, and four critical guards were
   mutation-proved: the errno window, the child verdict-versus-status check,
   the rung ordering under `--strict`, and the banner reporting the rung rather
   than the machine. Each went red, and each named its own test.
2. **Is it consistent?** Four numbers had moved and their sentences had not:
   `docs/AGENTS.md` and `scripts/plant.sh` still said fifteen and sixteen
   checks, [T-1202](gate.md)'s closing note still said fifteen plants, the root
   `README.md` still said nothing was implemented, and `experiments/README.md`
   listed neither of the two new scripts. All five are corrected.
3. **Is it usable cold?** The whole CLI surface was driven as a first-time user
   would: no arguments, `--help`, an unknown verb, `probe --help`, an unknown
   flag, `--json --rows` together, and the internal child flag with and without
   a name. Every one answers on the right channel with the right code. ⚠ One
   real defect came out of this pass rather than out of a test: with fd 1
   closed at process start, `pipe2(2)` hands a pipe end to fd 1 and the child's
   unconditional `close` after a no-op `dup2(1, 1)` shut the channel every
   probe answers through. Every verdict would have become a skip, honestly and
   uselessly. Fixed, and driven with `podbox probe 1>&-` to confirm.

### One correction to the probe itself, made after the first confined run

⚠ `mount(tmpfs,/mnt)` originally checked that `/mnt` existed and skipped when it
did not. `/mnt` does not exist inside the reconstruction, so the row skipped and
this machine's mount policy went unmeasured, while a seccomp filter would have
answered `EPERM` before the syscall body ever resolved the path. The check moved
from **before the call** to **after the errno**: only `ENOENT` is a statement
about the target. The row now reads `EPERM` inside, which is the reference's
reading and the right one.

## The work order

⭐ **This is the only work order.** Do not take one from the index or from a
kickoff prompt.

1. **M1, image acquisition.** [T-1102](milestones.md) and
   [image.md](image.md), with [T-0905](deps.md) and [T-0906](deps.md) measured
   before either lands. `experiments/110-bloat-delta.sh <area>` is now the
   instrument for both.
2. **M2, extraction.** [T-1103](milestones.md) and [extract.md](extract.md).
   The highest-risk component, and three of its four acceptance criteria are
   already measured.
3. **M3, `run`.** [T-1104](milestones.md), [enter.md](enter.md),
   [cli.md](cli.md). ⭐ It also closes the halves [T-0107](probe.md) and
   [T-0108](probe.md) are `partial` for, and both name exactly what is left.
4. Then M4, M5, M6, M7 in order. [T-0709](interpose.md) and
   [T-0410](complete.md) are both P0 and both land inside M6 and M5
   respectively; neither needs a measurement that has not been taken.

⚠ [T-0205](image.md) and [T-1004](packaging.md) are P3 and are not in the order.
They are worth doing when something else touches the same ground.

⛔ **One piece of M0's specification has no entry and was not written.**
`TOOL.md` section 6.1's last paragraph asks for the probe result to be cached in
`$store/probe.json`, keyed by boot id, and re-probed when the key changes. There
is no store until M1, so it cannot be built yet; there is also no entry for it,
and authoring one is a separate pass from implementing (`docs/AGENTS.md`'s
routing table). ⭐ The next session should author it into [image.md](image.md)
or [probe.md](probe.md) before M1's store lands, so the cache is designed with
the store rather than bolted to it.

## In progress

Nothing. Every entry this session opened is closed or is `partial` with the
remaining half named and the milestone it lands in.

## One debt, introduced earlier and not cleared

⛔ **About 1400 files of an exported debian rootfs and 23 cargo artefacts were
committed in `954030b` and untracked again three commits later.** They are gone
from the working tree and from `HEAD`, and the gate refuses the class
(check 15, planted against by `scripts/plant.sh` case 15). They remain in git
history: a fresh clone carries 88 MB of objects where it carried 27 MB.

Clearing it needs a history rewrite and a force push over a pushed branch, which
is the operator's decision and not a session's. ⚠ Until then the clone is 61 MB
larger than it needs to be and nothing else is wrong: no tracked path names any
of it, and `check-todo.py` fails if one ever does again.

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
   does not, so those rows report `SKIP` and the script exits 2. ⭐ It would also
   close the one row of M0's own acceptance that this host cannot exercise:
   `move_mount(-> /tmp/mm-probe)` attaches here and is `EPERM` on the target,
   so podbox's `chroot` rung was selected without the LSM ever being present.
4. **A kernel with `CONFIG_CHECKPOINT_RESTORE`**, so the `kcmp(2)` control of
   the bogus-argument discriminator can answer. Without it `podbox probe`
   reports `controls_answered: false` on every run of this host, which is
   correct and is also a permanent notice nobody can clear here. The target has
   it. [T-0102](probe.md).
5. **Whether `podbox` should refuse to install itself as `docker` where a
   working docker daemon exists.** [T-0803](cli.md) proposes refusing without an
   explicit flag. A machine with a working daemon is a machine where podbox is
   the wrong tool, and the alternative reading is that podbox should defer to it
   transparently. The entry carries the recommendation and not a ruling.
6. **Why a static glibc binary takes SIGFPE on `opensuse-leap-15.6`.**
   Measured by `experiments/125-across-distributions.sh` and recorded as a
   reading, not a diagnosis. It bears on what podbox may assume about payloads
   in an image, and the row is also the one distribution whose `nsswitch: compat`
   setting is therefore untested for the passwd question.
7. ⚠ **The branch.** `docs/AGENTS.md`'s first absolute and
   [RULES.md](RULES.md) section 2 both say `main`, and the operator said `main`
   in this session; the harness this session ran under asked for a
   `claude/*` branch. The three were reconciled the way `docs/AGENTS.md`'s
   closing section says to: the operator's word first, then the linked rule.
   Work is on `main`.
