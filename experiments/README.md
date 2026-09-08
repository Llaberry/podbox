# experiments/

Scripts that reconstruct the runtime `paper_final.md` characterizes, and take
the measurements the paper cites, on a machine that is not the target.

Numbered in the order they run. A number is never reused: a citation of `30-`
has to keep meaning what it meant.

| script | the question it answers |
|---|---|
| `10-build-target-image.sh` | can a container image supply the target's *userspace* - the toolchain it has, the files it lacks? |
| `20-enter-target.sh` | can a container supply the target's *kernel-visible shape* - its mount topology, its ID map, its filter, its write policy - and which parts does this host refuse? |
| `30-attribution-census.sh` | which mechanism produces which denial, measured one mechanism at a time, against a written-down expectation. ⭐ `--capture experiments/results` is what writes `attribute.txt`, `census.txt` and `identity.txt`, and `130-` compares against the first |

## What else is in here, and why it is not numbered

⚠ Three things in this directory are not experiments and take no number,
because a number here is a citation somebody may write down:

| path | what it is |
|---|---|
| `Dockerfile.target` | the image `10-` builds, pinned by base digest |
| `targetfs.sh` | the image's ENTRYPOINT. It shapes the filesystem inside the container and then execs `/workspace/.harness/enter.sh`, which is written by `20-`. It is never run from a host |
| `src/` | the small programs the language comparison of `40-` builds and measures |

⭐ **Re-running an experiment dirties the tree by its date line.**
`110-` and `130-` end by saying whether every *measured* value reproduced the
committed one, so that diff is never guesswork:

```sh
./scripts/common/result-diff.sh experiments/results/bloat-baseline.txt
```

## Exit codes

Uniform across all three, per this repository's convention:

| code | meaning |
|---|---|
| 0 | the measurement ran and matched expectation |
| 1 | the measurement ran and something under test failed |
| 2 | the measurement could not run (missing dependency, kernel feature absent) |

## Quick start

```sh
./experiments/10-build-target-image.sh          # ~2 min, needs docker + network
./experiments/20-enter-target.sh                # interactive shell in the reconstruction
./experiments/20-enter-target.sh -- id          # or run one command
./experiments/30-attribution-census.sh          # the census, with assertions
./experiments/40-language-selection.sh          # the language comparison
./experiments/50-interpose-tier.sh              # pathmap against the four walls
```

⚠ The scripts above are seeded from `Azathothas/container-research` and measure
the target runtime. The ones below are **this project's own** and measure
decisions podbox has to make. Each writes its transcript to
`experiments/results/`.

```sh
./experiments/60-interposer-libc.sh             # podbox's cdylib per libc. Exits 2 here, and names why
./experiments/70-whiteout-contract.sh           # the OCI layer contract, against two pinned images
./experiments/80-interposer-abi.sh              # which interposer a payload may load, decided from ELF
./experiments/90-nsswitch-contract.sh           # whether a supplied /etc/passwd is read at all
./experiments/100-interpose-symbols.sh          # the exec family, and the completeness test
./experiments/110-bloat-delta.sh baseline       # the release total, its breakdown, and the ceiling
./experiments/125-across-distributions.sh       # ~10 min, eleven pinned distributions
./experiments/130-probe-parity.sh               # M0's acceptance: the rung in both environments, and the rows
```

⛔ `80-`, `90-`, `100-`, `125-` and `130-` need a running docker daemon, and
`80-` needs `musl-gcc` for its fourth arm. `110-` needs `cargo-bloat` for its
breakdown and exits 2 without it. Each says which of its checks could not run
rather than reporting a pass it did not earn.

⭐ **`130-` is `TODO/milestones.md` T-1101's acceptance as one command**, so it
is re-run rather than recalled. It builds nothing: point `PODBOX_BIN` at a
binary, or build the default first with
`cargo build --release --target x86_64-unknown-linux-musl`.

⚠ **`20-` builds its harness from the corpus, not from this tree.** It arrived
seeded from `Azathothas/container-research`, where the Go sources sit at
`verification/`, and podbox tracks that repository under `references/` instead.
`HARNESS_SRC` names the one place they are and the conditions block prints it.
Every invocation failed at `cd` until 2026-09-08 because of that.

## Running your own binary against it

This is the point of `20-` for anyone implementing against this runtime rather
than reading about it. `--stage` copies a file or directory into what becomes
`/workspace` - one of the four writable paths inside - and is repeatable:

```sh
./experiments/20-enter-target.sh --stage ./target/release/podbox \
    -- /workspace/podbox probe
```

Your binary then runs as uid 0 in a user namespace mapping `0 -> 1000`, under
the filter, in the target's mount topology, with no `/etc/passwd`, no `/run`,
no `/var`, a 64 MiB `/tmp` and six device nodes. `--raw` gives you the same
container *without* the confinement, which is where you set fixtures up.

`20-` needs a **privileged** container: reconstructing a mount topology
requires `mount(2)`, and reconstructing a partial ID map requires writing
`uid_map` with a host id the process does not own. Neither is available to the
reconstruction *inside* - that is the point of it.

## What this reconstructs, and what it cannot

The target is described in `paper_final.md` section 3.1, section 3.6, section 3.7 and measured in
`verification/real/`. Three mechanisms compose it:

| | mechanism | reconstructed here? |
|---|---|---|
| **N** | user namespace, map `0 -> 1000`, `setgroups` denied, one unmapped supplementary group, mount namespace owned by it | **yes**, fully |
| **F** | seccomp filter denying `unshare`, `setns`, `mount`, `umount2`, `pivot_root`, `ptrace`, `process_vm_readv`, `process_vm_writev` | **yes**, fully |
| **M** | Landlock-class write allowlist over `{/tmp, /dev/shm, /workspace, /state}` | **only where the host kernel has Landlock** |

⛔ **M needs `CONFIG_SECURITY_LANDLOCK` and `landlock` in the boot LSM list.**
Where the kernel lacks it, `20-` prints `M: unavailable` and continues with N+F;
`30-` then reports the M-dependent rows as `SKIP` and exits 2 rather than
passing a run that never tested them. This is a real gap, not a formality: on
the kernel this repository's own captures were taken on
(`6.18.44-fc-v24`, a Firecracker guest), `CONFIG_SECURITY_LANDLOCK is not set`,
so the M rows here are **[S]** from kernel source plus **[T]** from the target,
never **[V]** locally. Any host with a distro kernel - Debian, Fedora, Arch,
Ubuntu >= 20.04 - has Landlock and closes the gap.

Two further differences are inherent and are *not* bugs to chase:

- **The kernel is the host's.** Syscall availability, the Landlock ABI and the
  new mount API all follow the host kernel, not the target's 6.18.39. `30-`
  prints the running version with every result.
- **The reconstruction is not the security boundary.** Everything here is
  built by a privileged process that could tear it down. It reproduces what a
  confined program *observes*; it is not a sandbox and must not be used as one.

## Conditions

Every script prints, on the way out: host kernel, container runtime version,
image digest, the mechanisms actually applied, and the date. A number quoted
from these scripts without that block is a number without conditions.
