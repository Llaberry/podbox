# podbox

A container runtime for Linux environments that hand a process uid 0 and then
refuse almost every operation containers are built on.

The environment is common: AI-agent sandboxes, hardened CI executors, locked
down HPC nodes. A process there holds every capability bit and `Seccomp: 2`,
and cannot `unshare`, `mount`, `pivot_root`, `ptrace`, `mknod`, `setuid` to a
non-zero id, or `chown` to an unmapped one. `docker` on PATH is a podman alias
with no daemon.

podbox answers to `docker` and `podman` on PATH, takes the same verbs, flags
and exit codes, and where it cannot honour something it says so in one line.

## State

**`podbox probe` works. Nothing else does.** This tree is milestone M0 of
[`TOOL.md`](references/Azathothas__container-research/tree/TOOL.md) section 5: the
probe, and nothing else. Every other verb exits 125 and says so. The next
milestone is M1, image acquisition.

```sh
podbox probe            # the rung on stdout, the evidence on stderr, exit 0
podbox probe --json     # the same findings, for a harness
podbox probe --rows     # every probe, one row each
podbox probe --strict   # exit non-zero below the `namespace` rung
```

⛔ **It reports the mode it achieved and never lets a weaker one satisfy a
stronger request.** On a machine that hands a process uid 0 and refuses mounts
it selects `chroot`, prints what that does not provide, and does not pretend to
be a container.

`TODO/PROGRESS.md` carries the state line, the counts and the work order.

| path | what it is |
| --- | --- |
| [`TODO/`](TODO/) | the work. `INDEX.md` lists every entry, `PROGRESS.md` carries the order, `reference-map.md` carries the corpus and its licence determinations |
| [`crates/`](crates/) | the workspace of `TOOL.md` section 4.3. `podbox-probe` and the `probe` verb of `podbox-cli` are implemented; the rest are skeletons |
| [`references/`](references/) | the corpus: 30 trees at pinned commits, with their trackers. Tracked, in the tree |
| [`experiments/`](experiments/) | the reconstruction of the target runtime, seeded from `Azathothas/container-research`, plus this project's own measurements |
| [`scripts/`](scripts/) | the gate, the count scripts, the corpus fetcher, the environment bootstrap, and the `zig cc` wrappers |
| [`docs/`](docs/) | the methodology this repository is worked under, copied verbatim from [`Azathothas/TEMPLATE`](https://github.com/Azathothas/TEMPLATE). **Binding, not advisory** |

⭐ **Working on this repository: read [`docs/AGENTS.md`](docs/AGENTS.md) and
nothing else first.** It is the router. It says what to read for the task in
front of you, what this environment does to a session, and what a session owes
at its end. Everything binding is one link away from it.

## Building

A session in a fresh container starts here. It is idempotent, so running it on
a machine that already has everything costs a few seconds and changes nothing:

```sh
./scripts/common/bootstrap-env.sh
```

```sh
cargo build --release --target x86_64-unknown-linux-musl
readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep INTERP
```

The second command prints nothing: the artefact is a single static binary with
no `PT_INTERP`. The interposer is the one object that is not static, and it
is built separately:

```sh
./scripts/build-interpose.sh
```

## The gate

```sh
./scripts/check-todo.py
```

It re-derives every count in `TODO/INDEX.md` from the rows, asserts that no
status disagrees between the index and its entry, that every reference in
`TODO/reference-map.md` resolves, and that every cited path and line exists.
It runs in CI and it exits non-zero on a disagreement.

## Licence

[0BSD](LICENSE). No attribution clause, no notice retention, no share-alike.

⚠ Vendored code keeps its own licence and its own notices.
[`THIRD_PARTY.md`](THIRD_PARTY.md) carries them, and
[`TODO/reference-map.md`](TODO/reference-map.md) carries the determination that
had to be made before each tree was used.
