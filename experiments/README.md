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

⚠ **`--stage` clears its destination before copying, and that is not
tidiness.** `$STAGE` survives between runs, and `cp -a src/store dest/store`
NESTS when `dest/store` already exists, so a second run staging the same
directory reads the first run's copy. Measured on 2026-09-08 while staging a
podbox store twice for `TODO/probe.md` T-0111: the clause passed for a reason
that had nothing to do with what it was testing. Staging a FILE overwrites,
which is why only staging a directory revealed it.

## What else is in here, and why it is not numbered

⚠ Three things in this directory are not experiments and take no number,
because a number here is a citation somebody may write down:

| path | what it is |
|---|---|
| `Dockerfile.target` | the image `10-` builds, pinned by base digest |
| `targetfs.sh` | the image's ENTRYPOINT. It shapes the filesystem inside the container and then execs `/workspace/.harness/enter.sh`, which is written by `20-`. It is never run from a host |
| `src/` | the small programs the language comparison of `40-` builds and measures |

⭐ **Re-running an experiment dirties the tree by its date line.**
`110-`, `130-`, `140-`, `150-`, `160-` and `170-` end by saying whether every
*measured* value reproduced the committed one, so that diff is never guesswork:

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
./experiments/110-bloat-delta.sh image          # the release total, its breakdown, and the ceiling
./experiments/125-across-distributions.sh       # ~10 min, eleven pinned distributions
./experiments/130-probe-parity.sh               # M0's acceptance: the rung in both environments, and the rows
./experiments/140-space-precheck.sh             # blocks AND inodes, on two real tmpfs mounts
./experiments/150-image-acquisition.sh          # M1's acceptance: podbox's digest against docker's
./experiments/160-store-gc.sh                   # a GC under a holder, and the containment check
./experiments/170-probe-cache.sh                # the probe cache, and the key the specification got wrong
./experiments/210-store-concurrency.sh          # the store's contract, against 8 real concurrent processes
./experiments/220-extract-path-safety.sh        # M2: a hostile layer is refused and a distro rootfs is not
./experiments/260-multiarch.sh                  # six architectures check the workspace, and the one that does not
./experiments/270-multiarch-image.sh            # two platforms of one tag, and the ELF machine inside each tree
./experiments/280-insecure-registry.sh          # a registry with no certificate, and one nothing trusts
./experiments/290-microvm.sh                    # a stock kernel under QEMU, for the two questions this host cannot answer
./experiments/300-run.sh                        # M3's acceptance: a command inside an image, and the rung it got
./experiments/310-session-startup.sh            # what a cold session costs, and what dev.sh removes from it
./experiments/320-cli-contract.sh               # the parity table as data, and the docker and podman names
```

⚠ **The numbers jump from `170-` to `210-`, and again from `230-` to `260-`.**
`180-` to `200-` are reserved by entries M1 authored, and `230-` to `250-` by
M4, M5 and M6's acceptances; a number here is never reused even before its
script exists. [`../TODO/gate.md`](../TODO/gate.md) T-1205 records the four
`Prove` clauses that named a taken number, and check 18 of the gate now refuses
a fifth.

⛔ `80-`, `90-`, `100-`, `125-`, `130-`, `150-`, `170-`, `270-`, `280-` and
`300-` need a running docker daemon, and `80-` needs `musl-gcc` for its fourth
arm. `210-` needs outbound HTTPS and nothing else, because it pulls with eight
processes at once and the registry is not what it is measuring.
`110-` needs `cargo-bloat` for its breakdown and exits 2 without it. `140-`
needs to be able to `mount` a tmpfs and exits 2 where it cannot. `150-` needs
outbound HTTPS to a registry. `260-`'s aarch64 arm needs `binfmt_misc` and
`qemu-user`; `290-` needs `qemu-system-x86_64`, `busybox-static` and `cpio`.
⚠ `320-` is the odd one: a docker daemon is not a dependency there but a
CONDITION, and it decides which half of clause 5 can be measured, because the
operator's ruling of 2026-09-08 is about what podbox does when one answers.
Each says which of its checks could not run rather than reporting a pass it did
not earn.

⭐ **`130-`, `150-`, `220-` and `300-` are `TODO/milestones.md` T-1101's,
T-1102's, T-1103's and T-1104's acceptance as one command each**, and `320-` is
`TODO/cli.md` T-0801's and T-0803's, so all five are re-run rather than
recalled. None of them builds anything: point `PODBOX_BIN` at a binary, or build
the default first with
`cargo build --release --target x86_64-unknown-linux-musl`.

⚠ **`150-` and `160-` pull from a registry and `alpine:latest` is a moving
tag.** `PODBOX_TEST_IMAGE` overrides it, and a digest-pinned reference removes
the race that `150-` otherwise has to detect and report.

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
