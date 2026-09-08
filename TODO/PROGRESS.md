# PROGRESS

⭐ **The record.** Where the project is, what the last session did, and the work
order. Rewritten every session. It carries no history: the history is the git
log and the entries.

**State: M0 complete and the dependency sweep is done. `podbox probe` exists,
is measured in both environments, and its acceptance runs as one command.
Every candidate in [deps.md](deps.md) is priced, ruled and recorded. M1, image
acquisition, is next and has nothing left to decide before it starts.**
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
| probes in the set | 50: 34 census, 16 attribution | `podbox probe --json` |
| `/dev/ptmx` here, unconfined | present and openable: chardev 5:2, mode 666 | `podbox probe --json \| jq .ptmx`, T-0503 |
| `/dev/ptmx` inside the reconstruction | `ENOENT` to both `stat` and `open`. ⛔ Not an answer about the target: see below | the same |
| `kcmp(-1,-1,...)` control, on this host | `ENOSYS`: the control cannot answer | `experiments/results/attribute.txt` |
| `kcmp(-1,-1,...)` control, on the target | `ESRCH`: executed | `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26` |
| writable paths obtained inside the reconstruction | 4 of 8 probed: `/tmp`, `/dev/shm`, `/workspace`, `/state` | `podbox probe --json` |
| interposer cdylib under `+crt-static` | refused by cargo | `experiments/60-interposer-libc.sh` |
| ⭐ interposer cdylib, `-crt-static`, musl target, linked by `zig cc` | 286,056 bytes, `DT_NEEDED libc.so`, genuinely musl-linked | `experiments/60-interposer-libc.sh`, which now exits **0** |
| the same before `zig cc`, linked by the host `cc` | 14,064 bytes and **not musl-linked**: `DT_NEEDED libc.so.6` | the reading this replaces, kept because it is why the arm exited 2 |
| interposer cdylib, `-crt-static`, gnu target | 266,632 bytes | `experiments/60-interposer-libc.sh` |
| ⭐ podbox's own musl cdylib into a glibc payload | **refused**, `libc.so: invalid ELF header`, rc 127 | `experiments/60-interposer-libc.sh` check B |
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
| ⚠ git objects, fresh clone | 51 MB `.git`, 49.48 MiB in one pack, 9,445 objects. See the debt below | `du -sh .git` and `git count-objects -vH` after `git clone` |
| `alpine:3.20` `etc/shadow` ownership | uid 0, gid 42 | `experiments/70-whiteout-contract.sh` |
| OCI layer member-name prefix | no `./` on any layer of either pinned image | `experiments/70-whiteout-contract.sh` |
| a slash-anchored whiteout glob against a layer-root whiteout | misses it | `experiments/70-whiteout-contract.sh` |

Acceptance, run on 2026-09-08:

```
$ ./scripts/check-todo.py
check-todo: 86 rows, 86 entries, 58 open, 3 partial, 2 blocked, 23 done
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

86 entries: 58 open, 3 partial, 2 blocked, 23 done.

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

### The dependency sweep, ruled against numbers

⭐ **[deps.md](deps.md) T-0901 to T-0908 are closed**, each with a measured
`cargo bloat` delta and its scaffold committed under `experiments/results/`.
The operator's ruling of 2026-09-08 is that a sweep may land what the
measurement and the entry's own recommendation agree on, and it is in
[RULES.md](RULES.md) section 11.

| area | crates | delta on 496,184 | ruling |
| --- | --- | --- | --- |
| `libc`, `rustix` ([T-0901](deps.md)) | 1, 3 | 0, below resolution | hand-declare |
| `seccompiler` ([T-0902](deps.md)) | 2 | +8,192 | hand-emit |
| `landlock` crate ([T-0903](deps.md)) | 13 | 0, below resolution | hand-declare |
| `oci-spec` ([T-0904](deps.md)) | 40 | +69,664 | write the four structs |
| ⭐ `rustls` + roots ([T-0905](deps.md)) | 15 | **+901,168** | **lands** |
| ⭐ `ureq` ([T-0906](deps.md)) | 69 | +1,024,152 (+123,000 over TLS) | **lands** |
| ⭐ `tar` + `flate2` + `ruzstd` ([T-0907](deps.md)) | 12 | **+69,632** | **lands** |
| ⭐ `sha2`, `serde_json` ([T-0908](deps.md)) | 9, 13 | +8,192, +32,768 | **land** |
| `clap` ([T-0908](deps.md)) | 4 | **+159,744** | no: the CLI is a table |
| `goblin` ([T-0908](deps.md)) | 11 | +32,768 | no: a header walk |

⚠ **Nothing landed is in the artefact yet.** The pins are in
`[workspace.dependencies]`; a member takes one when the milestone that needs it
lands, so `Cargo.lock` is unchanged and
`experiments/results/bloat-baseline.txt` is still the "before".

### Two findings the sweep produced that no entry predicted

1. ⛔ **`rustls` is pure Rust and its crypto provider is not.**
   [T-0905](deps.md)'s premise ruled OpenSSL out because it is C; `ring`
   compiles C and assembly too, and the first musl build failed on a missing
   cross-compiler. It does **not** break `crt-static`: the artefact still has
   no `PT_INTERP` and no `NEEDED`. What it costs is a C cross-compiler, and
   `scripts/zig-cc.sh` plus `.cargo/config.toml` is the answer.
2. ⛔ **A dependency nothing calls measures as zero.** `lto = true` deletes it,
   and a sweep that reports "this crate is free" has measured nothing at all.
   `experiments/110-bloat-delta.sh` now refuses that run, and it distinguishes
   it from a candidate genuinely below the instrument's resolution by
   **running the scaffold**: if the scaffold speaks, a zero delta is a reading;
   if it does not, the run failed to measure. Both halves were driven.

### `/dev/ptmx` is a command now, not a research task

[T-0503](enter.md) puts `stat("/dev/ptmx")` in T-0101's probe set, and that half
is implemented: two rows, `stat` and `open`, because existence is not function
and a node that stats and will not open is exactly the degraded `-t` the entry
refuses. `podbox probe --json | jq .ptmx` answers it on any machine.

⛔ **It does not answer it for the target yet, and the reconstruction cannot.**
The reconstruction builds `/dev` from the same mount table the entry says
cannot settle the question, so its `ENOENT` is that table repeated back rather
than an independent reading. What is still needed is one run on the target.

### New infrastructure this session

- ⭐ **`scripts/common/bootstrap-env.sh`.** Every session runs in a new
  container, so the tools the last one installed are gone. One idempotent,
  non-interactive script brings a barebones machine up to what the gate, the
  experiments and the builds need: the musl target, `cargo-bloat`, go, gcc,
  `zig`, the docker daemon **started**, and the small tools. `--check` reports
  without changing anything. It checks blocks **and** inodes first, times
  everything, and refuses a download whose checksum does not match. All three
  behaviours were driven: the checksum guard on a deliberate mismatch (nothing
  was unpacked), the real install, and a stopped docker daemon being restarted.
- ⭐ **`scripts/zig-cc.sh` and `scripts/zig-ar.sh`**, wired into
  `.cargo/config.toml`. `zig cc` carries its own `compiler-rt` and its own musl
  sources, so it is both a pinned input and the answer to the `-lgcc_s`
  shortage. ⚠ It rewrites `-target` rather than prepending one: `ring`'s build
  script passes the four-field **Rust** triple, which zig rejects as
  `UnknownOperatingSystem`, and a prepended flag loses to a later one.

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
is the operator's decision and not a session's. Nothing else is wrong: no
tracked path names any of it, and `check-todo.py` fails if one ever does again.

⚠ **The size of the debt is smaller than it was recorded as, and the earlier
figure is corrected here rather than defended.** The session that introduced it
recorded 88 MB of objects against 27 MB before. Measured on 2026-09-08 by
cloning the pushed `main` afresh: `du -sh .git` is **51 MB**, and
`git count-objects -vH` reports one pack of **49.48 MiB** over 9,445 objects.
Two readings of the same repository disagreeing is itself the finding; the
likeliest explanation is that 88 MB was read before the server repacked, and
`du` on a loose-object clone is not `du` on a packed one. ⛔ The **before**
figure of 27 MB is left as the earlier session's reading and was not re-measured:
it would need the history at `a92848d^`, and inventing a number for it is worse
than carrying one that says whose it is. The excess is therefore unquantified
here, not quantified wrongly.

## Open questions for the operator

1. **`/dev/ptmx` on the target**, and it is now ⭐ **one command rather than a
   research task**: `podbox probe --json | jq .ptmx`, run on the target by
   anyone who can reach it. The probe half of [T-0503](enter.md) is implemented
   and reads `present: true, usable: true` on this host and `ENOENT` inside the
   reconstruction. ⛔ **The reconstruction does not settle it**: it builds
   `/dev` from the same mount table the question doubts, so its answer is that
   table repeated back. What is still needed is a run on the target itself.
2. ⭐ **ANSWERED, and no longer a question for the operator.** It asked for a
   musl cross toolchain carrying its own `libgcc_s`, because `musl-tools`
   ships none and rustc passes `-lgcc_s` on that target even under
   `panic = "abort"`. `zig cc` carries its own `compiler-rt`, is installed
   pinned and checksummed by `scripts/common/bootstrap-env.sh`, and is wired
   into `.cargo/config.toml`, and `experiments/60-interposer-libc.sh` was
   re-run against it: it **exits 0**, having exited 2 since it was written, and
   its check B now reproduces the cross-libc refusal with podbox's own Rust
   cdylib rather than only with the C reference interposer.
   [T-0702](interpose.md) carries both readings.
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
