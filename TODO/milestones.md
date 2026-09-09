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
Status:      done 2026-09-09

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
Prove:       `./experiments/70-whiteout-contract.sh` exits 0; `./experiments/220-extract-path-safety.sh` exits 0; `podbox pull alpine:latest && podbox extract alpine:latest && test -f "$(podbox inspect --format '{{.RootfsPath}}' alpine:latest)/etc/shadow"`; `podbox pull voidlinux/voidlinux-musl:latest && podbox extract voidlinux/voidlinux-musl:latest && ! test -L "$(podbox inspect --format '{{.RootfsPath}}' voidlinux/voidlinux-musl:latest)/var/cache/xbps"`; then, when M3 lands, the same two images under `podbox run --rm`

**Partial, 2026-09-08.** Every clause above ran and matched, on this host, with
`crates/podbox-extract` and the `extract` verb. [extract.md](extract.md) T-0301
to T-0307 are all `done`, and this entry stays `partial` for one reason, named
below.

| clause | reading |
| --- | --- |
| `70-whiteout-contract.sh` | exits 0 |
| `220-extract-path-safety.sh` | exits 0, six checks |
| `alpine` has `etc/shadow` | present, 515 entries, the one gid-42 row recorded |
| `voidlinux` has no `var/cache/xbps` link | gone, whiteouted by layer 2; 543 other symlinks survive |

⭐ **CLOSED 2026-09-09, AND THE LAST TWO CLAUSES RAN UNDER `run --rm`.**

| clause, from inside the container | reading |
| --- | --- |
| `podbox run --rm alpine:latest sh -c 'test -f /etc/shadow'` | exit 0, `shadow-present` |
| `podbox run --rm voidlinux/voidlinux-musl:latest sh -c 'test -L /var/cache/xbps'` | not a link |

⭐ **And running from inside showed the ownership wall for the first time.**
`ls -ln /etc/shadow` inside the container reports **gid 0**, not the 42 the image
declares. That is [extract.md](extract.md) T-0302 working exactly as designed
and now visible from the payload's own side: podbox could not apply the id, it
did not pretend to, and `.meta.jsonl` beside the rootfs carries what the image
meant. ⚠ A caller who needs the real gid needs M6's interposer, and until then
the sidecar is the honest answer rather than the convenient one.

⚠ **What follows was true when this entry was `partial` and is kept** because it
is why the `Prove` has the shape it has.

⛔ **THIS ENTRY COULD NOT CLOSE UNTIL M3, AND THAT WAS KNOWN BEFORE M2 STARTED.**
The `Prove` as authored ran `podbox run --rm`, which is [T-1104](milestones.md).
M2 can implement and drive extraction and cannot enter the tree it produced, in
exactly the way [T-0107](probe.md), [T-0108](probe.md) and [T-0204](image.md)
are `partial` now. What is left is one line: re-run the two image clauses under
`run --rm` instead of against the extracted rootfs, and close this entry.

⚠ **THE `Prove` WAS REWRITTEN, AND NOT ONLY TO REMOVE `run`.** As authored it
was one `&&` chain of six commands, and that shape cannot report what this
milestone needs:

1. ⛔ **it collapses the third state.** `70-whiteout-contract.sh` exits **2**
   when it cannot run,  no docker, no network,  and in an `&&` chain a 2 stops
   the chain and reads as a failure. `docs/AGENTS.md` absolute 4 makes "could
   not run" a state of its own precisely so it never reads as "ran and did not
   match";
2. ⛔ **the composite status names no clause.** `podbox` exits 125 on a runtime
   failure and 2 on invalid input, an experiment exits 2 for "could not run",
   and the chain reports one number for all six. A reader cannot tell which
   link produced it, which is the same defect `docs/AGENTS.md` absolute 8
   states for a pipe.

The clauses are therefore separate commands, each read from the process that
produced it. This is the shape [T-1101](milestones.md) already closed against:
one script per question, three-state exit, per-clause verdicts.

⚠ **The experiment was renumbered from `80-` before a line of it was written.**
80 is `experiments/80-interposer-abi.sh`; `experiments/README.md` rules a number
is never reused. [T-1205](gate.md) found this and three more, and the gate now
holds the rule.

---

### T-1104 M3 `run` on the chroot rung

Source:      `TOOL.md` section 5 M3
Category:    milestones
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     ⭐ This is the product requirement the whole specification exists
             for: an agent that knows docker must need zero new knowledge.
Premise:     Read. The work is [enter.md](enter.md) T-0501 through T-0505 and
             [cli.md](cli.md) T-0801 through T-0806.
Approach:    That exact command, inside the reconstruction, with the mode banner
             on stderr and nothing but the payload's output on stdout.
Decision:    The banner goes to stderr. A banner on stdout corrupts every
             pipeline the payload is in, and the payload's stdout is data.
Prove:       `./experiments/300-run.sh` exits 0. Clause 7 is the command above, with the store pre-pulled outside and staged in

**Done, 2026-09-09.** `experiments/300-run.sh`, **eight clauses, exit 0**, and
the three entries this milestone was `partial` for are closed:
[T-0505](enter.md) is clause 8, and [T-0801](cli.md) and [T-0803](cli.md) are
`experiments/320-cli-contract.sh`. Clause 7 is this entry's own acceptance, run
inside the reconstruction:

| | |
| --- | --- |
| stdout | `hi`, and nothing else |
| exit | 0 |
| rung selected inside the reconstruction | **`chroot`** |
| the same podbox on this host | `namespace` |
| the banner | `this mode does NOT provide: process, network, IPC or mount isolation` |

⭐ **The rung differing between the two is the point.** Every other clause runs
on this host, where `mount(2)` succeeds and podbox selects `namespace`; the
reconstruction is where it is `EPERM` and podbox has to fall to the rung this
milestone is named for. A run that selected `namespace` in both would have
proven nothing about `chroot`.

⚠ **The store is pre-pulled outside and staged in, and clause 7 runs
`--pull never`.** The reconstruction has no CA bundle, so a pull from inside it
fails on an unverifiable certificate. That is a fact about the reconstruction
and not about `run`; acquisition is M1's and `150-image-acquisition.sh` proves
it.

⭐ **The three entries this was `partial` for closed on the same day**, and
each brought its own clause rather than a claim: [T-0505](enter.md) is clause 8
here, and [T-0801](cli.md) and [T-0803](cli.md) are clauses 1 to 5 of
`experiments/320-cli-contract.sh`. [T-0802](cli.md)'s exit codes are clause 2.

⚠ **[T-0506](enter.md) stays `partial` and is NOT this milestone's blocker.**
Its `run` half is clause 5 above; its remaining half is that a `podbox probe`
run inside a foreign-architecture container measures the emulator and does not
yet say so, which is a probe question rather than a `run` one.

---

### T-1105 M4 the lifecycle, twenty times

Source:      `TOOL.md` section 5 M4
Category:    milestones
Priority:    P0
Effort:      L
Status:      done 2026-09-09

Problem:     The prior art's own capture of this lifecycle contains a failed run
             because it decided "running" by sleeping and looking.
Premise:     Read. The work is [supervise.md](supervise.md) T-0601 through
             T-0607.
Approach:    `create`, `start`, `ps`, `logs`, `stop`, `rm`, `exec`, `inspect`,
             `kill`, `wait`, `cp`. The loop is create, detached start, `ps` shows
             it running, `exec` prints a marker, `stop`, `rm`.
Decision:    Twenty consecutive passes, and a single failure fails the milestone.
             Retrying a failure is how a race becomes a published pass.
Prove:       `./experiments/230-lifecycle-loop.sh 20` exits 0

**Done, 2026-09-09.** `./experiments/230-lifecycle-loop.sh 20` reports **20 of
20 consecutive passes** and its three following clauses all run.

⭐ **What is in.** `create`, `start`, `ps`, `logs`, `stop`, `kill`, `wait`, `rm`,
`cp`, and `exec` and `inspect` against a container, plus `run -d` and `--name`.
Running state is launcher state: one detached supervisor per container holds the
payload's pidfd, holds the container's lock, owns a control socket, and is the
only thing that writes `running` or `exited` into
`crates/podbox-supervise/src/table.rs`. Nothing reads `/proc` to decide
membership and nothing sleeps to decide readiness.

⭐ **The acceptance failed three times first, and that is the milestone
working.** 10 of 20, 9 of 20 and 1 of 3, always at `stop`, and the cause was a
real race in `stop`: it connects to the launcher twice, and a container that
stopped fast let the launcher tear its socket down in between.
[T-0602](supervise.md) carries it. ⛔ Nothing was retried to get the pass: the
defect the failure named was fixed and the loop then ran clean.

⚠ Four defects the building of this found, the race above and three more, each recorded where it belongs:
`std::fs::read("/dev/urandom")` has no EOF and allocated 13 GB before the OOM
killer took it ([T-0601](supervise.md)'s crate); a readiness pipe without
`O_CLOEXEC` is inherited through the payload's `execve`, so `run -d` blocked for
exactly as long as the container ran; and `si_status` sits at byte 24 of a
`siginfo_t` and not 20, which reported a SIGTERMed payload as exit 128.

---

### T-1106 M5 environment completion, ten distributions

Source:      `TOOL.md` section 5 M5, section 9
Category:    milestones
Priority:    P1
Effort:      L
Status:      partial

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
Prove:       `./experiments/240-distro-sweep.sh` exits 0


**Partial, 2026-09-09. Eight of the ten rows build and run a C program; one is
the machine's and one is podbox's, and the sweep says which is which by running
the same subject under docker.**

`experiments/240-distro-sweep.sh`. ⛔ The mechanics are T-1203's, not a second
runner: `scripts/common/distro-matrix.sh` holds the pinning, the `no-pull` row,
the exit-2-when-nothing-ran rule and the reference qualification, and
`125-across-distributions.sh` sources the same file. This entry supplies the
subject and the row list.

⛔ **NO DOCKER HUB.** `ghcr.io`, `public.ecr.aws` and the distributions' own
registries, all ten pinned by manifest digest on 2026-09-09.

```
ROW            LIBC     PM            INSTALL   BUILD   RAN
alpine         musl     apk           0         0       42
debian         glibc    apt-get       0         0       42
ubuntu         glibc    apt-get       0         0       42
archlinux      glibc    pacman        0         0       42
almalinux      glibc    dnf           0         0       42
rocky          glibc    dnf           0         0       42
rocky-minimal  glibc    microdnf      0         0       42
fedora         glibc    dnf           0         0       42
opensuse-leap  glibc    zypper        104       127     no-compiler
voidlinux-musl musl     xbps-install  2         127     no-compiler
```

⭐ **A ROW THAT FAILS IS RUN AGAIN UNDER DOCKER, and that control is what makes
the two failures different answers rather than one number.**

- `voidlinux-musl` fails **identically under docker** on the same image, with
  the same `SSL_connect returned 1` from `xbps`. It is the machine, not either
  runtime, and the row reads `host`.
- `opensuse-leap` **succeeds under docker**, so it is podbox's.
  [T-0412](complete.md) is authored with the whole diagnosis: `libzypp` hands
  libcurl a `CURLOPT_CAPATH` of `/etc/ssl/certs`, a hash-indexed directory,
  and every CAfile podbox writes is invisible to it. `curl --cacert` on the
  bundle podbox installed returns 200 on the same host where the default
  returns error 60.

⭐ **What the sweep found, which is the reason for a matrix rather than a smoke
test.** Each of these passed on some rows and failed on others, and a one-image
test would have found none of them:

| finding | seen on | not on |
| --- | --- | --- |
| an image config with no `PATH`, so gcc could not find `cc1` | rocky, rocky-minimal | almalinux, the same gcc |
| `./` as a tar member, refusing the whole layer | every debian-family image | alpine, arch |
| `/etc/ssl/certs` a symlink, so the CA fixup could not write | opensuse | everything else |
| `/lib` a symlink, so the libc probe read `unknown` | void | everything else |
| a CAfile at a path the TLS stack does not read | void, opensuse | everything else |

⚠ **Why it is `partial` and not `done`.** Nine of the ten fixups are in and the
acceptance runs, but the entry's own bar is ten rows and one of them is podbox's
to fix. It stays open, names the blocker and names what would clear it, which is
[RULES.md](RULES.md) section 5.

Prove, run 2026-09-09:

```
$ ./experiments/240-distro-sweep.sh
  rows 10, ran 10, no_pull 0, harness_failed 0
  built_and_ran 8, host_not_runtime 1
  exit 1, because opensuse-leap is podbox's
```

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
Status:      partial

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
Prove:       `./experiments/250-negative-tests.sh` exits 0

**Partial, 2026-09-09.** `experiments/250-negative-tests.sh` drives eleven
refusals through the shipped binary and exits **2**, because three of them
cannot be measured on this machine and one of those needs M6.

⛔ **Every clause asserts TWO things and the second is the one that rots**: the
exit code, read from the process that produced it, and that the message NAMES
the reason. "unknown option" where the parity table has a reason is a regression
even though the code is unchanged.

What ran and held:

```
  run --network=none                 rc=125  named: "no network namespace to select"
  run -v host:/mapped:ro             rc=125  named: "a copy pretending to be a mount"
  run --strict                       rc=125  4 reasons listed, each on its own line
  the same run without --strict      rc=0
  an unlisted flag                   rc=125  named: "no row in the parity table"
  a None flag names its status       rc=125  named: "status None"
  a None VERB names its reason       rc=125  named the row's own note
  pull http://…                      rc=1    named "HTTPS only", and NOT 124
  30-attribution-census.sh           rc=2    the third state, never 1
```

⚠ **Three clauses did not run here, and each says so rather than passing
quietly**:

1. a Go payload under `interpose` declined rather than silently unvirtualized:
   ⛔ M6 has no interposer, and this clause is [T-0709](interpose.md)'s `Prove`;
2. `-t` refused by name: this machine's `/dev/ptmx` IS usable, so the arm that
   ran is the positive one and the refusal could not be driven;
3. the `wait`-refuses-a-dead-container clause skipped once, on a container whose
   launcher pid read 0. ⚠ That is the observation [T-0608](supervise.md)
   records, and it is why the clause reports what it found rather than skipping
   silently.

⛔ The `pull http://` clause runs under a `timeout` and asserts the code is not
124, because the failure that refusal exists to prevent is a **hang** rather
than an error: a clause with no bound would pass by hanging.

