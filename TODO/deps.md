# deps

`TOOL.md` section 3.5. The dependency policy, applied.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

⛔ **Every dependency is a decision that has to be argued, and the default
answer is no.** The artefact is packed into a single file, is a candidate for
the memfd launch rung, and ships to machines with a 64 MiB `/tmp`. Binary size
is a functional requirement here.

⛔ **`Cargo.toml`'s `[workspace.dependencies]` is empty and that is deliberate.**
`TOOL.md` section 3.5 printed a table of fifteen crates in an earlier revision, and it
read as a shopping list. What follows is the sweep, one entry per area, and each
closes with **five answers and a measured `cargo bloat` delta**, not a
`cargo add`.

## The five questions every entry below answers

| | |
| --- | --- |
| 1 | What does it do that we would otherwise write, and roughly how much code is that? |
| 2 | What does it cost, as measured `cargo bloat` and `cargo tree` figures against the same build without it? |
| 3 | Does it pull C? A C dependency breaks `crt-static` cleanliness or drags a toolchain. Prefer the pure-Rust path and **measure** the performance claim before choosing the C one. |
| 4 | Is it maintained, and is its licence compatible with redistribution under 0BSD? Determined per `docs/methodology/references.md` step 3, **tracker read**, and recorded in [reference-map.md](reference-map.md). |
| 5 | Vendor or registry? |

## The vendor and registry split

It is not about size. It is about **whether we will end up patching it**.

| | what | why |
| --- | --- | --- |
| **Vendor and patch, in-tree** | anything niche, small, unmaintained, or sitting on a seam we will need to move: the interposer's building blocks, anything ELF- or syscall-shaped, anything forked | We will need behaviour its published interface does not expose, and `docs/methodology/vendoring.md` says fix it here, so it has to live here |
| **Registry dependency, pinned** | large, generic, well-maintained infrastructure with no seam we need to move: TLS, hashing, JSON, argument parsing, compression | Vendoring buys nothing and costs a cold compile plus every one of their lints becoming ours |
| **Write it ourselves** | anything where the dependency is bigger than the code it replaces | The default when 1 and 2 do not clearly favour the crate |

---

### T-0901 Sweep: syscalls

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      M
Status:      open

Problem:     Roughly every syscall podbox makes is a raw one, and `unsafe` is
             unavoidable. The question is whether a crate is saving enough of it
             to pay for itself.
Premise:     Read. `rustix` with its `linux_raw` backend is the starting point.
             ⚠ Going in: the new mount API, the seccomp ioctls and Landlock are
             likely hand-declared either way, so the crate covers the ordinary
             half and not the interesting half.
Approach:    Declare the section 6.1 probe set by hand first, measure the binary, then
             add the crate and measure again. `libc` alone may be enough, and it
             is already in the dependency tree of anything else considered here.
Decision:    Recommendation, to be confirmed by the measurement: hand-declare,
             with `libc` for the type and constant definitions. `TOOL.md` section 3.3
             requires `unsafe` be confined to one module per subsystem with the
             safe wrapper adjacent, and that shape is the same work whether a
             crate is under it or not.
Prove:       `./experiments/110-bloat-delta.sh syscalls` exits 0 and writes `experiments/results/bloat-syscalls.txt`

---

### T-0902 Sweep: seccomp BPF

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P2
Effort:      S
Status:      open

Problem:     podbox installs short filters. A crate that generalizes filter
             construction may be larger than the filters.
Premise:     Read, with a size reference in the corpus:
             `references/Azathothas__container-research/tree/verification/confine/main.go`
             emits a filter by hand in about sixty lines, and that is the size
             of the thing a crate would replace.
Approach:    Emit the BPF by hand, measure, then compare against `seccompiler`.
             ⚠ The notification listener is a separate question from filter
             construction and is T-0606; a crate that helps with one may not
             help with the other.
Decision:    Recommendation: hand-emit. Revisit if the notification tier ever
             becomes reachable and needs a larger filter surface.
Prove:       `./experiments/110-bloat-delta.sh seccomp` exits 0 and writes `experiments/results/bloat-seccomp.txt`

---

### T-0903 Sweep: Landlock

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P3
Effort:      S
Status:      open

Problem:     Three syscalls. A crate for three syscalls needs an argument.
             ⚠ podbox does not currently **install** a Landlock ruleset; it
             probes for one that is already there. This entry is the sweep for
             the day it does.
Premise:     Read, with a size reference:
             `references/Azathothas__container-research/tree/verification/confine/landlock.go`
             is the whole mechanism in one file.
             ⭐ The corpus also carries a design ruling that bears on it:
             `references/multikernel__sandlock` PR #29 dropped path strings from
             its notification policy surface on the reasoning that "path-based
             access control belongs in static Landlock rules". Where podbox ever
             needs path policy, Landlock is where it goes, not the notifier.
Approach:    Hand-declare, measure, compare against the `landlock` crate.
Decision:    Recommendation: hand-declare. Three syscalls, and podbox needs the
             ABI-version probe more than the ruleset builder.
Prove:       `./experiments/110-bloat-delta.sh landlock` exits 0 and writes `experiments/results/bloat-landlock.txt`

---

### T-0904 Sweep: OCI registry client and types

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      M
Status:      open

Problem:     ⚠ The heaviest candidate in the list by far. `oci-client` and
             `oci-spec` cover a much larger surface than the endpoints podbox
             uses.
Premise:     Read. podbox needs token auth, manifest by tag and digest, manifest
             lists for one platform, and blob fetch: T-0201's list.
Approach:    Write the minimal client against that list, measure it, then
             measure the same build with `oci-client` and `oci-spec` and compare.
             Count the types actually deserialized: a manifest, a manifest list,
             an image config and a descriptor.
Decision:    Recommendation, to be confirmed: write it. The endpoint set is
             small and fixed, and `TOOL.md` section 5 M1 says the registry plane is the
             least interesting part of the problem and must not be gold-plated.
Prove:       `./experiments/110-bloat-delta.sh oci` exits 0 and writes `experiments/results/bloat-oci.txt`

---

### T-0905 Sweep: TLS

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P0
Effort:      S
Status:      open

Problem:     HTTPS is the only working transport (T-0201), so TLS is not
             optional, and the wrong choice breaks the static story.
Premise:     Read. ⛔ Non-negotiable that it is pure Rust: the alternative pulls
             OpenSSL, which is a C dependency and breaks `crt-static`.
             ⚠ One tree in the corpus shows the trap:
             `references/VHSgunzo__ulexec/tree/Cargo.toml:31` takes `reqwest`
             with `native-tls-vendored` on Linux while its Windows arm at
             `references/VHSgunzo__ulexec/tree/Cargo.toml:24` takes `rustls-tls`. The vendored-native arm is the one that
             drags a C toolchain.
Approach:    `rustls` plus `webpki-roots`, measured. Also measure the root store
             question separately: bundling roots costs binary size, and reading
             the host bundle costs a file that may not exist. podbox needs the
             host bundle anyway for T-0407, so measure "host bundle with
             `webpki-roots` as the fallback" against "roots bundled".
Decision:    Recommendation: `rustls`. There is no pure-Rust alternative with a
             comparable maintenance state, and the C path is ruled out by the
             static requirement rather than by preference.
             ⛔ Never disable certificate verification to work around the
             TLS-interception on a build host. That is a red line in
             `TOOL.md` section 11.1 and it is the one that turns a build problem into a
             shipped vulnerability.
Prove:       `./experiments/110-bloat-delta.sh tls` exits 0 and `readelf -d target/x86_64-unknown-linux-musl/release/podbox | grep -c NEEDED | grep -qx 0`

---

### T-0906 Sweep: HTTP

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      S
Status:      open

Problem:     An async runtime is a large dependency with nothing to do here.
Premise:     Read. podbox has no reason to be async: it fetches blobs
             sequentially and its concurrency is one child per container, held
             by a pidfd.
Approach:    Measure `ureq` against a hand-rolled client over `rustls`. Count
             what podbox actually needs: `GET` and `HEAD`, redirects, chunked
             bodies, a `Range` header for resumption, and bearer tokens.
Decision:    Recommendation: `ureq`, blocking. The hand-rolled path has to
             re-implement chunked transfer encoding and redirects, which is more
             code than the crate and is the class of code that is wrong in ways
             a test suite finds slowly.
Prove:       `./experiments/110-bloat-delta.sh http` exits 0 and writes `experiments/results/bloat-http.txt`

---

### T-0907 Sweep: tar, gzip, zstd

Source:      `TOOL.md` section 3.5, section 6.3
Category:    deps
Priority:    P0
Effort:      M
Status:      open

Problem:     Extraction is driven at **entry level, never `unpack()`** (T-0301),
             so the crate is being used for a fraction of its surface and the
             fraction matters more than the total.
Premise:     Read. `tar`, `flate2` with `rust_backend`, and `ruzstd` are the
             starting points. ⛔ `flate2`'s default backend is C
             (`miniz_oxide` is the Rust one and is the feature to select), and
             the C backend is the trap question 3 exists for.
Approach:    Confirm the entry-level API gives header, reader and no implicit
             filesystem action. Measure with `rust_backend` selected. Measure
             `ruzstd` against skipping zstd entirely: zstd layers exist and are
             not universal, and a named refusal for a zstd layer is a legitimate
             v1 answer if the size delta is large.
Decision:    Recommendation: `tar` plus `flate2` with `rust_backend`, and a
             measurement before zstd. `TOOL.md` section 6.3 rules out shelling out and
             a hand-written tar reader is the one piece of this list that is
             genuinely small, so the measurement may still say write it.
Prove:       `./experiments/110-bloat-delta.sh archive` exits 0 and `cargo tree -e normal -p podbox-extract | grep -c ' cc ' | grep -qx 0`

---

### T-0908 Sweep: digests, JSON, argument parsing, ELF

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      M
Status:      open

Problem:     Four small questions with one large answer among them. ⚠ `clap` is
             large for what podbox needs from it.
Premise:     Read. `sha2`, `serde_json`, `clap` and `goblin` are the starting
             points. podbox's parser has a table (T-0801) rather than a grammar,
             and its ELF needs are `PT_INTERP` presence plus a Go build-marker
             check (T-0706), which is a header walk.
Approach:    Measure each separately, because they are independent decisions
             that a single "add the obvious four" would bundle.
             For `clap`, measure it against a hand-rolled parser driven by the
             section 6.8 table. For `goblin`, measure it against a program-header walk.
Decision:    Recommendation: `sha2` and `serde_json` yes, `clap` and `goblin`
             measured first and probably no. `clap`'s help generation is the
             part podbox would use least, because docker's help text is the
             text to match rather than generate.
Prove:       `./experiments/110-bloat-delta.sh cli-json-elf` exits 0 and writes `experiments/results/bloat-cli-json-elf.txt`

---

### T-0909 Vendor the memfd and userland-exec rungs, and fix the fork's regression here

Source:      `TOOL.md` section 3.5; [reference-map.md](reference-map.md)
Category:    deps
Priority:    P1
Effort:      M
Status:      blocked

Problem:     These sit on a seam podbox will move, and one of them is a fork
             maintained because the original is not. A registry dependency on an
             unmaintained crate is a blocker waiting to happen, and "blocked on
             upstream" is not an outcome this methodology has a place for.
Premise:     ⭐ **Read at file and line, and it corrects the specification in two
             places.**
             1. `TOOL.md` section 3.5 calls `userland-execve` "the second fork to
                vendor". There is no such fork.
                `references/VHSgunzo__userland-execve/PROVENANCE.md` records the
                404 beside a reachable control, and
                `references/VHSgunzo__ulexec/tree/Cargo.toml:29` shows `ulexec`
                depending on the original crate, whose repository is
                `references/io12__userland-execve-rust`. Its licence is clean:
                `tree/LICENSE` present and `tree/Cargo.toml:8` says MIT.
             2. The `memfd-exec` fork carries a regression against its own
                upstream, and both trees are in the corpus so it is a diff.
                `references/novafacing__memfd-exec/tree/src/executable.rs:546`
                passes `MemFdCreateFlag::MFD_CLOEXEC` unconditionally.
                `references/VHSgunzo__memfd-exec/tree/src/executable.rs:684-691`
                makes it conditional on `fn is_running_in_qemu() -> bool { true }`,
                a stub that always returns true, so `MFD_CLOEXEC` is **never**
                set and the memfd leaks across every later `execve`. It is
                marked `// TODO: add detect for qemu emulator`.
             What the fork adds and is worth having:
             `references/VHSgunzo__memfd-exec/tree/src/executable.rs:574-601`
             `fallback_exec()`, the launch ladder as a library, and
             `references/VHSgunzo__memfd-exec/tree/src/executable.rs:694-704`,
             which checks `is_exe("/proc/self/fd/<mfd>")` before writing so a
             non-executable memfd becomes `EACCES` there rather than a confusing
             `fexecve` failure later.
Approach:    Vendor `io12/userland-execve-rust` whole: 512 lines across six
             files, MIT, licence file present. Do **not** vendor either
             `memfd-exec` until its licence resolves; `memfd_create` plus
             `fexecve` is three syscalls and podbox declares them itself, with
             the `is_exe` probe and the `MFD_CLOEXEC` flag both taken as
             mechanisms rather than as code.
             ⚠ Carry the stack-size costing from
             `references/io12__userland-execve-rust`'s tracker: issue #1 records
             that the synthetic stack was 1 MB and GCC's `cc1plus` overflowed
             it, fixed by raising it to 10 MB, because GCC normally raises its
             own stack with `setrlimit()` and a synthetic stack does not honour
             that. M5 builds a C toolchain project in ten distributions, so this
             is on podbox's path. Its issue #5 is open: non-PIE ELF binaries
             cannot be loaded by that rung at all.
Decision:    Vendor `userland-execve` and write podbox's own three-syscall memfd
             path, rather than vendoring `memfd-exec` once its licence resolves.
             The fork's value is two mechanisms and a ladder, all of which are
             smaller re-implemented than carried, and carrying it would import a
             regression this tree would then have to patch out
             (the `MFD_CLOEXEC` stub above) for no gain.
Status note: **blocked** on the `memfd-exec` licence, for the vendoring half
             only. The userland-exec half is not blocked and can proceed.
             ⛔ Nothing is opened on anybody's repository to unblock it. What
             would clear it is a licence file appearing in either tree at a later
             commit, checked at the next reconciliation.
Prove:       `test -f vendor/userland-execve/LICENSE && grep -q '^license = "MIT"' vendor/userland-execve/Cargo.toml && ./experiments/110-bloat-delta.sh memfd` exits 0

---

### T-0910 The `cargo bloat` baseline, committed, and checked at the gate

Source:      `TOOL.md` section 3.5; `docs/methodology/experiments.md`
Category:    deps
Priority:    P0
Effort:      S
Status:      done 2026-09-08

Problem:     ⛔ A dependency that lands without a before-and-after number has not
             landed. Without a committed baseline there is no "before".
Premise:     Measured, for the empty skeleton, on this host: the release binary
             is 389,656 bytes with no dependencies at all. That is the baseline
             every delta below is measured against, and it is quoted here as one
             machine on one day, as
             `docs/conventions/prose.md:156-159` requires.
Approach:    `experiments/110-bloat-delta.sh <area>` takes the current binary
             size and the `cargo bloat` breakdown, with and without the
             dependency under test, and writes both to
             `experiments/results/bloat-<area>.txt`. It prints its conditions and
             exits 0, 1 or 2 per the convention.
             ⭐ Wire the total into the gate as a ceiling, so a dependency that
             lands quietly fails CI rather than being noticed at release.
Decision:    A ceiling on the total, not on the delta. A delta ceiling permits
             an unbounded number of small dependencies, which is how a binary
             gets large without any single decision being wrong.
Prove:       `./experiments/110-bloat-delta.sh baseline` exits 0, `./scripts/check-todo.py` exits 0, and `./scripts/plant.sh` reports cases 17a, 17b and 17c caught

**Done 2026-09-08.** The `Prove` commands were run and exit 0. The reading is
`experiments/results/bloat-baseline.txt`: **496,184 bytes and zero third-party
crates**, against 389,656 for the empty skeleton of T-1100. M0's probe is the
whole of the difference and it took no dependency, which is the measurement
[T-0901](#t-0901-sweep-syscalls) asked for before a syscall crate is considered.

⚠ **The ceiling moved house rather than moving.** Its value is unchanged; where
it lives is not. It was a bare literal in the gate workflow with nothing behind
it, and the same number was quoted in this entry's own `Prove`. It now lives in
`CEILING_BYTES` in `experiments/110-bloat-delta.sh`, the workflow calls that
script, and check 17 of `scripts/check-todo.py` refuses a tree where any other
file this project wrote names the number. ⭐ That check fired on the first run,
against the sentence you are reading, which is why neither it nor the `Prove`
above spells the number out: under the check they were the second and third
copies.

⚠ The `cargo bloat` breakdown is taken from a **second build of the same
profile with `strip` turned off**, into its own target directory, because
`strip = "symbols"` leaves cargo-bloat nothing to read. Its proportions are the
artefact's; its absolute sizes are not, and the file says so. Where
`cargo-bloat` is not installed the script exits 2 and records that the
breakdown was not taken, because a skip is not a pass.
