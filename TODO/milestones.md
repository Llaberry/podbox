# milestones

`TOOL.md` section 5. One entry per milestone, carrying its acceptance test and nothing
else: the work is in the component files, and the milestone is the gate that
says the work is done.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

⛔ **Do not proceed on a milestone whose test has never run green.** A test that
has never run green does not count, and neither does a milestone built on one.

The order is a dependency order, not a preference. [PROGRESS.md](PROGRESS.md)
carries the work order.

---

### T-1100 M-1 the corpus, the work index and the skeleton

Source:      `TOOL.md` section 5 M-1
Category:    milestones
Priority:    P0
Effort:      L
Status:      done 2026-09-08

Problem:     Everything after this depends on `TODO/` existing and being right,
             and on the references having been read rather than cited.
Premise:     ⭐ **Measured, and the acceptance is four commands.** `TOOL.md` section 5
             M-1: the reader script exits 0; every entry cites a path and line
             that resolves; `cargo build --release --target
             x86_64-unknown-linux-musl` succeeds on the empty skeleton;
             `readelf -l` on the result shows no `PT_INTERP`.
Approach:    Produced: `docs/` copied verbatim; `LICENSE` (0BSD), `README.md`,
             `THIRD_PARTY.md`; the workspace of section 4.3 with every crate a
             skeleton; `rust-toolchain.toml` and `.cargo/config.toml`;
             `experiments/` seeded from
             `references/Azathothas__container-research` plus three measurements
             of this project's own; the corpus of thirty trees under
             `references/`; `TODO/` with its index, rules, record,
             reference map and per-category entries; the two count scripts, the
             reader wired as a gate; and CI that runs it.
Decision:    The corpus is **tracked in the tree** rather than on a side branch,
             because `scripts/check-todo.py` resolves every cited path and line
             and cannot do so into a branch it is not on.
             [reference-map.md](reference-map.md) records the choice and what a
             clone pays for it.
Prove:       `./scripts/check-todo.py && cargo build --release --target x86_64-unknown-linux-musl && readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP | grep -qx 0`

**Done. The `Prove` command was run on 2026-09-08 and exits 0.** The output is
in [PROGRESS.md](PROGRESS.md)'s baseline block.

⭐ **After this, no session needs to read the references again.** Each entry
carries what to do and which reference to open at which line. That is the whole
point of paying for it once.

---

### T-1101 M0 the probe, and nothing else

Source:      `TOOL.md` section 5 M0
Category:    milestones
Priority:    P0
Effort:      M
Status:      done 2026-09-08

Problem:     Everything downstream branches on the probe, and it is the
             component the prior art most consistently gets wrong.
Premise:     Read. The work is [probe.md](probe.md) T-0101 through T-0110.
Approach:    `podbox probe` prints the mode it would select, the evidence, and
             exits 0. Nothing else is implemented at this milestone.
Decision:    The probe before the store, before extraction, before `run`.
             Reversing it means every later component carries an assumption
             about the machine that the probe would have replaced with a
             measurement.
Prove:       `./experiments/130-probe-parity.sh` exits 0. It runs the three clauses of the acceptance in one command: `chroot` inside `./experiments/20-enter-target.sh --stage ./target/x86_64-unknown-linux-musl/release/podbox`, `namespace` unconfined, and every attribution row against `experiments/results/attribute.txt`

**Done 2026-09-08.** `./experiments/130-probe-parity.sh` exits 0. Its transcript
is `experiments/results/probe-parity.txt`:

```
== 1. inside the reconstruction, the rung must be chroot
  got chroot
== 2. unconfined, the rung must be namespace
  got namespace
== 3. the attribution rows ...
  15 matched, 1 recorded divergence, 0 differed, 0 missing
```

⭐ **The acceptance is a script rather than three commands somebody re-types.**
The wording above was amended to name it: the three clauses are unchanged, and
`docs/AGENTS.md`'s fourth absolute requires the measurement to ship with the
script that took it. `experiments/130-probe-parity.sh` is that script.

⛔ **Two blockers were in the way and both were defects in this tree, not in the
probe.**

1. `experiments/20-enter-target.sh` named `$REPO/verification/{confine,probe,cprobe}`
   for its harness sources, and this tree has no `verification/`: podbox tracks
   that repository under `references/`. Every invocation had been failing at
   `cd`, so the reconstruction had never run here. It now resolves
   `HARNESS_SRC` to
   `references/Azathothas__container-research/tree/verification` and prints it
   in the conditions block. **A citation that does not resolve, wearing a shell
   error message.**
2. `experiments/results/attribute.txt` did not exist. The acceptance names it,
   and nothing had produced it. `./experiments/30-attribution-census.sh --capture
   experiments/results` now has, alongside `census.txt` and `identity.txt`. It
   exits 2 on this host: the three Landlock rows cannot run here and are
   reported as skipped rather than passed.

⭐ **The one recorded divergence is the reference being wrong, not podbox.** The
Go instrument records `kcmp(-1,-1,...) [control] FAIL errno=38 ENOSYS`. `ENOSYS`
is this kernel saying `kcmp(2)` was not built in, which is the control being
absent rather than the control answering; podbox reports it as `skip`.
[T-0109](probe.md) is exactly that rule and [T-0102](probe.md) carries the
reading from both hosts. The parity script allows that one substitution, only
for that row, only when the errno numbers agree, and prints it in full on every
run; anything else is a mismatch.

⚠ **What M0 does NOT include, stated so the next milestone is not surprised.**
`run`, `exec` and the store are M3 and M1. [T-0107](probe.md) and
[T-0108](probe.md) are `partial` for that reason and each names the half that is
left. `TOOL.md` section 6.1's `$store/probe.json` cache is [T-0111](probe.md),
authored in this session and deliberately not implemented: it belongs beside
the store. ⛔ Authoring it found that the specification's cache key does not
work, and the entry carries the reading that settles it.

---

### T-1102 M1 image acquisition

Source:      `TOOL.md` section 5 M1
Category:    milestones
Priority:    P1
Effort:      M
Status:      done 2026-09-08

Problem:     No extraction is possible without a store, and no store is
             trustworthy without digest parity.
Premise:     Read. The work is [image.md](image.md) T-0201 through T-0204.
Approach:    `podbox pull`, `images`, `rmi`, `tag`, and a content-addressed
             store. No extraction yet.
Decision:    Do not gold-plate it. The registry plane is ordinary HTTPS and file
             I/O and it works here, and it is the least interesting part of the
             problem.
Prove:       `podbox pull alpine:latest && podbox images --format '{{.Digest}}' alpine:latest | grep -qx "$(docker image inspect alpine:latest --format '{{index .RepoDigests 0}}' | cut -d@ -f2)"`

**Done 2026-09-08**, against a script rather than a recollection:
`experiments/150-image-acquisition.sh` exits 0 and carries the `Prove` above as
its clause 1.

```
podbox  sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
docker  sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
```

`podbox pull`, `images`, `image ls`, `rmi`, `image rm`, `tag`, `image prune` and
`inspect` are implemented. [image.md](image.md) T-0201 and T-0202 are `done`;
T-0203 and T-0204 are `partial`, each with exactly one half left and each naming
the milestone it lands in: T-0203's second call site is before an extraction
that does not exist until M2, and T-0204's `Prove` holds its lock with
`podbox run -d`, which is M3.

⚠ **A moving tag is a race, and the script handles it rather than ignoring it.**
`alpine:latest` can be republished between podbox's pull and docker's, and the
two digests would then differ for a reason that is not podbox's. Clause 1
re-pulls both once on a mismatch and reports which of the two it was; it did not
have to on this run.

⭐ **The sweep stopped being inert.** `crates/podbox-image` is the first member
to take a `[workspace.dependencies]` pin, and the artefact moved from 496,184 to
**2,130,672 bytes**, a delta of **+1,634,488** against
`experiments/results/bloat-baseline.txt`, with 5,869,328 bytes of headroom under
the ceiling and still no `PT_INTERP`. `experiments/results/bloat-image.txt` is
the reading.

---

### T-1103 M2 extraction that survives the ownership wall

Source:      `TOOL.md` section 5 M2
Category:    milestones
Priority:    P0
Effort:      L
Status:      open

Problem:     The single highest-risk component. Four separate tools in the
             corpus stop here.
Premise:     ⭐ Measured, for three of the four acceptance criteria.
             `experiments/results/whiteout-contract.txt` establishes that
             `alpine`'s `etc/shadow` is gid 42, that `voidlinux-musl` ships
             `var/cache/xbps` as an absolute self-referential symlink in layer 1
             and whiteouts it in layer 2, and that a slash-anchored whiteout
             selector misses a layer-root whiteout. The work is
             [extract.md](extract.md) T-0301 through T-0307.
Approach:    The four acceptance criteria, in order, and each is a separate
             failure mode rather than a variation of one.
Decision:    In-process extraction at entry level. Shelling out to `tar` is what
             makes this wall reach five tools instead of one.
Prove:       `./experiments/70-whiteout-contract.sh && ./experiments/80-extract-path-safety.sh && podbox pull alpine:latest && podbox run --rm alpine:latest test -f /etc/shadow && podbox pull voidlinux/voidlinux-musl:latest && podbox run --rm voidlinux/voidlinux-musl:latest sh -c '! test -L /var/cache/xbps'`

---

### T-1104 M3 `run` on the chroot rung

Source:      `TOOL.md` section 5 M3
Category:    milestones
Priority:    P0
Effort:      M
Status:      open

Problem:     ⭐ This is the product requirement the whole specification exists
             for: an agent that knows docker must need zero new knowledge.
Premise:     Read. The work is [enter.md](enter.md) T-0501 through T-0505 and
             [cli.md](cli.md) T-0801 through T-0806.
Approach:    That exact command, inside the reconstruction, with the mode banner
             on stderr and nothing but the payload's output on stdout.
Decision:    The banner goes to stderr. A banner on stdout corrupts every
             pipeline the payload is in, and the payload's stdout is data.
Prove:       `./experiments/20-enter-target.sh --stage ./target/x86_64-unknown-linux-musl/release/podbox -- /workspace/podbox run --rm alpine:latest /bin/echo hi` prints `hi` on stdout, a `mode=` line on stderr, and exits 0

---

### T-1105 M4 the lifecycle, twenty times

Source:      `TOOL.md` section 5 M4
Category:    milestones
Priority:    P0
Effort:      L
Status:      open

Problem:     The prior art's own capture of this lifecycle contains a failed run
             because it decided "running" by sleeping and looking.
Premise:     Read. The work is [supervise.md](supervise.md) T-0601 through
             T-0607.
Approach:    `create`, `start`, `ps`, `logs`, `stop`, `rm`, `exec`, `inspect`,
             `kill`, `wait`, `cp`. The loop is create, detached start, `ps` shows
             it running, `exec` prints a marker, `stop`, `rm`.
Decision:    Twenty consecutive passes, and a single failure fails the milestone.
             Retrying a failure is how a race becomes a published pass.
Prove:       `./experiments/100-lifecycle-loop.sh 20` exits 0

---

### T-1106 M5 environment completion, ten distributions

Source:      `TOOL.md` section 5 M5, section 9
Category:    milestones
Priority:    P1
Effort:      L
Status:      open

Problem:     The layer that makes package managers work. Without it the runtime
             runs `echo` and nothing a user wants.
Premise:     Read. The set is not arbitrary: alpine, debian, ubuntu, archlinux,
             almalinux, rocky, rocky-minimal, fedora, opensuse-leap and
             voidlinux-musl, each chosen because it broke something. The work is
             [complete.md](complete.md) T-0401 through T-0409.
Approach:    Each member installs a C toolchain through its native package
             manager and builds and runs a two-file project, with no
             user-supplied fixups.
             ⚠ **Drive it through T-1203's runner rather than writing a second
             one.** `experiments/125-across-distributions.sh` owns the pinning,
             the `no-pull` row, the exit-2-when-nothing-ran rule and the
             reference qualification; this milestone supplies the subject
             script and the row list. Two runners drift, and the one that
             drifts is the one nobody is looking at.
Decision:    A C toolchain rather than a trivial package. It exercises the
             ownership, the resolver, the keyring and the sandbox user at once,
             and a two-file project catches a toolchain that installed and
             cannot link.
Prove:       `./experiments/130-distro-sweep.sh` exits 0

---

### T-1107 M6 the interposer

Source:      `TOOL.md` section 5 M6
Category:    milestones
Priority:    P1
Effort:      L
Status:      open

Problem:     The measured gap. A stock path interposer delivers a bind view with
             `mount(2)` denied and leaves `chown 0:42` at `EINVAL`, identically
             to the bare call.
Premise:     Measured, and the reason is at file and line:
             `references/VHSgunzo__pathmap/tree/path-mapping.c:1240-1241` routes
             `chown` through the path-rewriting macro and passes the ids
             through untouched. The work is [interpose.md](interpose.md) T-0701
             through T-0708.
Approach:    Both halves in one object: path virtualization, and ownership
             virtualization that clears the wall stopping the most tools.
Decision:    Both, or the milestone is not done. ⭐ No project in the corpus does
             both halves in one interposer while also speaking docker's CLI.
             That gap is what podbox is.
Prove:       `podbox run --rm alpine:latest sh -c 'chown 0:42 /tmp/f && stat -c %u:%g /tmp/f' | grep -qx '0:42'` and `podbox run --rm -v "$PWD:/mapped" alpine:latest sh -c 'cd /mapped && test "$(pwd)" = /mapped'`

---

### T-1108 M7 packaging

Source:      `TOOL.md` section 5 M7
Category:    milestones
Priority:    P2
Effort:      M
Status:      open

Problem:     The binary has to run on the target with no libraries present.
Premise:     ⭐ Measured on the skeleton already: T-1001 is `done` and its
             `Prove` exits 0. What remains is holding it once dependencies land,
             which is what T-0910's ceiling is for.
Approach:    A single static binary, optionally one file with an embedded
             rootfs. The launch ladder is T-1003.
Decision:    Keep T-1001 and this entry separate. T-1001 is the property of the
             skeleton and is measurable now; this is the property of the shipped
             artefact and is not.
Prove:       `readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP | grep -qx 0 && ./experiments/20-enter-target.sh --stage ./target/x86_64-unknown-linux-musl/release/podbox -- /workspace/podbox version`

---

### T-1109 The negative tests, which are tests

Source:      `TOOL.md` section 9
Category:    milestones
Priority:    P1
Effort:      M
Status:      open

Problem:     Every honesty rule in section 4.1 and section 6.8 is unenforced until something
             asserts that the refusal happens. A rule that is only prose is a
             rule that regresses silently, which is the failure mode the whole
             design is built against.
Premise:     Read. `TOOL.md` section 9 names three: `--network=none` must **fail** with
             a named reason; `-v ...:ro` must be **rejected**; a Go payload under
             `interpose` must be **declined** rather than silently unvirtualized.
Approach:    Assert all three, plus the two this corpus adds:
             `--strict` turns every Degraded and Stub into a refusal (T-0804),
             and `-t` is a named refusal where `/dev/ptmx` is absent (T-0503).
             ⛔ `experiments/30-attribution-census.sh` must exit 0 or 2, never 1.
             A 1 means the runtime moved or a probe stopped discriminating, and
             either is a finding rather than a flake.
Decision:    Negative tests live in `experiments/` with the positive ones and
             share their exit-code convention, rather than in a separate suite.
             A refusal that regresses is the same class of defect as a feature
             that regresses, and splitting them makes one of the two easier to
             skip.
Prove:       `./experiments/140-negative-tests.sh` exits 0
