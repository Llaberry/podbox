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
Status:      done 2026-09-08

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

**Done 2026-09-08.** Measured as two areas rather than one, because the entry
names two candidates: `experiments/results/bloat-syscalls-libc.txt` and
`experiments/results/bloat-syscalls-rustix.txt`.

| candidate | crates | delta | what it covers |
| --- | --- | --- | --- |
| `libc` 0.2 | 1 | **0 bytes**, below the instrument's resolution | types and constants |
| `rustix` 0.38 | 3 | **0 bytes**, below the instrument's resolution | the ordinary half |
| hand-declared | 0 | the baseline | all of it, in 330 lines |

⭐ **The size argument is void in both directions, so the decision is the one
the entry predicted for the other reason.** Neither crate moves a static musl
binary measurably: `libc` on this target is mostly declarations against a libc
already linked, and `rustix`'s `linux_raw` backend emits the same instruction
`crates/podbox-probe/src/sys.rs` emits. ⚠ Both readings passed the instrument's
scaffold control, so a zero here is a reading and not an unmeasured run.

**Ruling: hand-declare, and neither crate lands.** `crates/podbox-probe/src/sys.rs`
is 330 lines including its documentation and its tests, and it already covers
the half the entry expected to stay hand-written anyway: the new mount API,
`seccomp`, `landlock_create_ruleset`, `kcmp` and `pidfd_getfd`. The 46 numbers
and 19 constants in it were checked against the kernel's own headers, which is
the correctness a crate would have been bought for.

⚠ `libc` is priced at zero and may be taken later for a struct definition
without re-arguing this entry. What it may not be taken for is the errno: a
libc wrapper answers with the library's `setuid` semantics rather than the
kernel's, which is [T-0101](probe.md)'s whole point.

---

### T-0902 Sweep: seccomp BPF

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P2
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** `experiments/results/bloat-seccomp.txt`: `seccompiler` 0.5
costs **+8,192 bytes** and 2 crates.

**Ruling: hand-emit, and it does not land.** The filter podbox installs is one
BPF instruction, already emitted in `crates/podbox-probe/src/probes.rs` as four
fields of a `#[repr(C)]` struct, and the reference emits a real one in about
sixty lines. 8 KB for sixty lines fails the third row of this file's own
vendor-and-registry table: the dependency is bigger than the code it replaces.

⚠ The entry's "revisit if the notification tier becomes reachable" is now
answered rather than pending: [T-0606](supervise.md) establishes the tier has
neither a race-safe channel nor a way to acquire one on this runtime, and
[T-0107](probe.md)'s rung selection refuses it for that reason. There is no
larger filter surface coming.

---

### T-0903 Sweep: Landlock

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P3
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08.** `experiments/results/bloat-landlock.txt`: the `landlock`
crate 0.4 costs **0 bytes** (below the instrument's resolution, scaffold
control answered) and **13 crates**.

**Ruling: hand-declare, and it does not land.** Zero bytes is not the whole
price: thirteen crates is thirteen more things to audit, pin and rebuild for a
mechanism podbox uses one syscall of. What podbox needs today is the ABI probe,
and that is `landlock_create_ruleset(NULL, 0, 1)`, already one row of
`crates/podbox-probe/src/probes.rs`. Revisit if podbox ever installs a ruleset,
which needs a kernel that has Landlock at all.

---

### T-0904 Sweep: OCI registry client and types

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      M
Status:      done 2026-09-08

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

**Done 2026-09-08.** `experiments/results/bloat-oci.txt`: `oci-spec` 0.7 with
only its `image` feature costs **+69,664 bytes** and **40 crates**, against
**+32,768 bytes and 13 crates** for `serde_json` plus structs
(`experiments/results/bloat-json.txt`).

**Ruling: write it, confirming the entry's recommendation.** Twice the bytes and
three times the crates, for four types. ⭐ The scaffold is itself part of the
finding: `oci-spec`'s `ImageManifest` and `Descriptor` refused literals that a
hand-written `#[derive(Deserialize)]` accepts, because it validates a larger
surface than the endpoints of [T-0201](image.md) need. That validation is not
free and it is not asked for.

⚠ `oci-client` was NOT measured here and is not refused here: it drags an HTTP
stack, which [T-0906](#t-0906-sweep-http) prices on its own, and measuring the
two together would have credited one with the other's cost.

---

### T-0905 Sweep: TLS

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P0
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08, and it lands.** Three arms, because the entry asks for the
root-store question to be measured separately:

| arm | crates | delta | roots seen |
| --- | --- | --- | --- |
| host bundle only, `rustls-pemfile` | 14 | +831,536 | 152 |
| ⭐ host bundle, `webpki-roots` as the fallback | 15 | **+901,168** | 152 |
| bundled roots only, `webpki-roots` | 15 | +880,688 | 121 |

`experiments/results/bloat-tls.txt` and `bloat-tls-hostroots.txt`. The static
half of the `Prove` holds: `readelf -d` reports **0** `NEEDED` entries, so the
artefact is still one static-pie file.

⚠ **TWO RUNS OF THE SAME ARM CAN DIFFER, AND THESE DID.** The first readings of
these arms were +819,248 and +897,072, taken before
`experiments/110-bloat-delta.sh` recorded the dependency declaration and the
scaffold beside the number. Re-taken so they carry both, they read +831,536 and
+901,168: 1.5% and 0.5%. The pins are version RANGES, so cargo resolves what is
newest at the moment of the run, and the bundled-roots row above is the one
reading not re-taken and is therefore quoted as the older one. ⭐ Nothing in the
ruling turns on 1.5%, and the reason the difference is visible at all is that a
result file now carries what produced it. A sweep that needs two runs to be
comparable to the byte needs a committed lock, which no entry asks for.

⛔ **THE PREMISE IS CORRECTED, AND THE CORRECTION IS THE FINDING.** It reads
that pure Rust is non-negotiable because the alternative pulls OpenSSL and
breaks `crt-static`. `rustls` **is** pure Rust and its crypto provider is not:
`ring` compiles C and assembly, and the first build for the musl target failed
with

```
error occurred in cc-rs: failed to find tool "x86_64-linux-musl-gcc"
```

So the C question is not answered by choosing `rustls` over OpenSSL. What
changes is which C, how much, and whether it is reproducible. ⭐ It does not
break `crt-static`: the artefact still has no `PT_INTERP` and no `NEEDED`. It
costs a C cross-compiler, and `scripts/zig-cc.sh` with `.cargo/config.toml` is
the answer, pinned and checksummed by
`scripts/common/bootstrap-env.sh`.

**Ruling: `rustls` lands, with the host bundle first and `webpki-roots` as the
fallback.** There is no pure-Rust-all-the-way-down alternative in a comparable
maintenance state, and HTTPS is the only transport that works
([T-0201](image.md)), so the alternative to this cost is no image acquisition
at all. The host bundle is read anyway for [T-0407](complete.md), and it found
more roots here than the bundled set (152 against 121); the bundled set is what
makes a machine with no bundle still work.

---

### T-0906 Sweep: HTTP

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      S
Status:      done 2026-09-08

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

**Done 2026-09-08, and it lands.** `experiments/results/bloat-http.txt`: `ureq`
2 with its own TLS stack costs **+1,024,152 bytes and 69 crates** against the
empty baseline, which is **about +123,000 bytes on top of the TLS arm** that has
to be paid either way. ⚠ That second figure is a subtraction between two runs
rather than a measurement, and [T-0905](#t-0905-sweep-tls) records why two runs
of one arm can differ by a percent. The order of magnitude is what the decision
rests on.

**Ruling: `ureq`, blocking, confirming the entry's recommendation.** 127 KB buys
chunked transfer encoding, redirects and the `Range` header, which the entry
argues correctly is more code than the crate and is the class that is wrong in
ways a suite finds slowly. The scaffold exercised exactly the surface the entry
lists: `GET`, `HEAD`, a redirect budget, a `Range` header and a bearer token.

⚠ **69 crates is the real price and it is not the bytes.** It is 69 things to
pin, audit and rebuild. The entry's premise that podbox has no reason to be
async holds and is what keeps that number from being far larger.

⭐ **One duplicate found while landing it, and the pin records it.** `ureq 2.12`
depends on `webpki-roots 0.26.11`, which is a compatibility shim over
`webpki-roots 1.0.9`. Naming `0.26` in `Cargo.toml` would have put podbox behind
the shim too; naming `1` shares the crate that actually holds the roots. The
resolve graph is quoted at the pin.

---

### T-0907 Sweep: tar, gzip, zstd

Source:      `TOOL.md` section 3.5, section 6.3
Category:    deps
Priority:    P0
Effort:      M
Status:      done 2026-09-08

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

**Done 2026-09-08, and all three land.**

| arm | crates | delta |
| --- | --- | --- |
| `tar` + `flate2` with `rust_backend` | 9 | +65,536 |
| the same plus `ruzstd` | 12 | **+69,632** |

`experiments/results/bloat-archive.txt` and `bloat-archive-zstd.txt`. No `cc` in
the graph with `rust_backend` selected, which is the trap question 3 exists for
and the reason that feature is named rather than defaulted. ⭐ Read out of
`cargo metadata`'s resolve graph rather than assumed: the whole set is
`adler2, cfg-if, crc32fast, filetime, flate2, libc, miniz_oxide, ruzstd,
simd-adler32, static_assertions, tar, twox-hash`, with neither `cc` nor
`libz-sys` in it.

⭐ **The zstd question the entry left open is settled by 4,096 bytes.** Adding
`ruzstd` costs one page over the gzip pair. A named refusal for a zstd layer
would have been a legitimate v1 answer against a large delta; against one page
it is a feature removed to save nothing.

⚠ The scaffold used the entry-level API only, which is what [T-0301](extract.md)
requires: a header, a path and a reader, with no implicit filesystem action.
`unpack()` was never called and never will be, because path safety, whiteouts
and the ownership sidecar are podbox's decisions.

⚠ The `Prove`'s second half names `podbox-extract`, and the measurement was
taken with the scaffold in `podbox-image`, which is where every arm of this
sweep put it so the arms are comparable. The `cc` check was read from the
workspace graph instead and is the same answer: no `cc` anywhere.

---

### T-0908 Sweep: digests, JSON, argument parsing, ELF

Source:      `TOOL.md` section 3.5
Category:    deps
Priority:    P1
Effort:      M
Status:      done 2026-09-08

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

**Done 2026-09-08.** Measured as four areas rather than one, because the entry
says they are independent decisions that a single "add the obvious four" would
bundle. `experiments/results/bloat-digest.txt`, `bloat-json.txt`,
`bloat-argparse.txt` and `bloat-elf.txt`.

| candidate | crates | delta | ruling |
| --- | --- | --- | --- |
| `sha2` | 9 | +8,192 | ⭐ **lands** |
| `serde_json` + `serde` | 13 | +32,768 | ⭐ **lands** |
| `goblin` | 11 | +32,768 | ⛔ no |
| `clap` | 4 | **+159,744** | ⛔ no |

⭐ **`clap` is the finding the entry predicted and the number is worse than
expected.** 159,744 bytes is nineteen times `sha2` and five times `serde_json`,
for a parser podbox drives from a **table** ([T-0801](cli.md)) rather than a
grammar. The measurement was taken with `default-features = false` and only
`std`, `help`, `usage` and `error-context` enabled, so it is the small clap and
not the large one. ⚠ Its help generation is the part podbox would use least:
docker's help text is text to **match**, not to generate.

**`goblin` costs the same as `serde_json` and replaces far less.** podbox's ELF
needs are `PT_INTERP` presence and a Go build-marker check
([T-0706](interpose.md)), which are program-header walks of a few dozen lines
each. The scaffold confirmed it: `interp=None sections=25` on podbox's own
static-pie binary is one field and one count, from a crate that parses the whole
format.

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

---

### T-0911 The syscall table and the kernel structs come from a crate, per architecture

Source:      Found by the operator on 2026-09-09, reading `crates/podbox-probe/src/sys.rs`
Category:    deps
Priority:    P0
Effort:      M
Status:      done 2026-09-09

Problem:     ⛔ **podbox was a single-architecture runtime because of a table
             somebody typed.** `crates/podbox-probe/src/sys.rs` declared 46
             x86_64 syscall numbers and a `#[repr(C)] struct stat` by hand, and
             opened with a `compile_error!` for every other architecture. The
             `compile_error!` was the right call given the table, a wrong
             number is a verdict reported for a syscall nobody named, but the
             table was the wrong thing to have. docker and podman run every
             architecture Linux ships; podbox refused to compile for seven of
             them.
Premise:     ⭐ **Measured on 2026-09-09, and the premise it overturns is
             [T-0901](#t-0901-sweep-syscalls)'s.** T-0901 ruled `libc` and
             `rustix` out at a delta below the instrument's resolution, on the
             grounds that the probe set is hand-declared and needs no crate.
             What that ruling priced was the bytes. What it did not price was
             the cost of hand-declaring, which is one architecture.

             ⚠ The requirement T-0901 was protecting is real and survives:
             [probe.md](probe.md) T-0101 needs **the kernel's** errno, not a
             libc wrapper's, because glibc's `setuid(3)` runs an id-change dance
             and returns the library's answer. `syscalls::raw` is not a libc
             wrapper: it issues the instruction and returns the raw value, so
             nothing passes through `errno`'s thread-local. The rule is kept and
             the table is gone.

             | | |
             | --- | --- |
             | architectures that compiled the workspace, before | **1** |
             | after | **6**, and one named blocker |
             | shipping binary, x86_64, before | 2,282,224 bytes |
             | after | **2,282,352 bytes**, +128 |

             ⭐ **The size question answered itself.** The operator ruled that
             features outrank bytes here, and the trade did not have to be made:
             both crates are `const` tables and inlined assembly, so `+128 bytes`
             buys six architectures. ⚠ `+128` is one build on one host and is
             below anything this instrument resolves; it is quoted as the
             reading it is and not as a law.
Approach:    `syscalls` for the numbers and the trap, `linux-raw-sys` for the
             kernel structs. Both `default-features = false`, both `no_std`,
             neither pulling a libc.
             ⛔ **What replaced the transcription risk rather than removing it.**
             A name a kernel does not define is a **compile error**, never a
             wrong number: `Sysno` has no such variant. Three cfg lists remain
             and each is a claim about the kernel that the compiler checks:
             1. the `asm-generic/unistd.h` architectures, which have no `open`,
                `stat`, `mkdir`, `unlink`, `chown`, `lchown`, `readlink`,
                `mknod` or `dup2`;
             2. the three names one syscall has, `newfstatat`, `fstatat` and
                `fstatat64`, **paired with the struct each writes**, because on
                a 32-bit architecture `fstatat64` writes `struct stat64` and
                taking it with `struct stat`'s buffer is a kernel write of the
                wrong shape into a right-sized hole;
             3. nothing else.
             Every path-taking **utility** routes through the `*at` form on every
             architecture, which is what `open(path)` means anyway and removes
             nine per-architecture cases. ⛔ The three calls where the entry
             point **is** the measurement, `chown`, `lchown`, `mknod`, keep
             their identity in `sys::Entry`, and on an architecture that has
             only `fchownat` the row prints `fchownat`. A probe that calls one
             syscall and names another is the dishonesty the probe set exists to
             refuse.
Decision:    Take the crates. ⚠ **`syscalls` 0.8.1 gates powerpc, s390x and mips
             behind `asm_experimental_arch`, and that gate is stale**: clause 3
             of `experiments/260-multiarch.sh` compiles powerpc64 inline
             assembly on stable rustc 1.98.1 and it succeeds. So
             `powerpc64le-unknown-linux-musl` does not build, for a reason that
             is not a rustc limit and is not podbox's code.
             ⛔ It is **not closed as somebody else's problem**
             (`docs/AGENTS.md` absolute 5). It stays named in clause 2 of that
             script, which goes **red** the day it starts building, and either of
             two things clears it: a `syscalls` release dropping the gate, or
             podbox taking the numbers from `linux-raw-sys`, which carries
             `__NR_*` for every architecture with no gate at all, and carrying
             its own six-line trap for those two. The second is entirely within
             this tree and is why the first is not a dependency on anyone.
             ⚠ `docs/AGENTS.md` absolute 2 says a defect in vendored code is
             fixed here. This crate is a registry dependency and not vendored,
             so the fix that applies is the second option, and it is recorded
             rather than taken because six architectures ship today without it.
Prove:       `./experiments/260-multiarch.sh` exits 0, and `cargo check --workspace --target aarch64-unknown-linux-musl` exits 0

**Done 2026-09-09.** `experiments/260-multiarch.sh` exits 0 and
`experiments/results/multiarch.txt` is its reading.

⭐ **podbox does not merely compile for aarch64, it runs there**, and clause 4
drives `podbox probe` under `qemu-aarch64` and reads back a rung, an identity
block and free-space rows. ⛔ **And clause 4 is a warning rather than a win.**
A probe run under `qemu-user` measures **QEMU**: it answers `EINVAL` to
`clone(CLONE_NEWNS)`, `ENOSYS` to `fsmount`, and reports `Seccomp: 0` whatever
the host is under. The rung it prints is the emulator's and podbox may not
report it as the machine's. That is the same mistake as the reconstruction's
`/dev` answering out of the mount table the question doubts, and it matters more
here: a foreign-architecture container runs its payload under exactly this
emulator, so [enter.md](enter.md) T-0506 owns saying so at runtime.

⭐ **What clause 4 does prove** is the part that could have been silently wrong:
the numbers, the struct layouts and the trap are right for aarch64. A `statfs`
layout off by one field gives garbage block counts, and the rows are sane.

⚠ **One trap cost the session real time and is written into clause 5 so it
cannot cost another.** A `binfmt_misc` magic emitted as **raw bytes** is
truncated by the kernel's parser at the first NUL, leaving a 7-byte magic that
matches **every** 64-bit ELF. Every native binary on the machine is then routed
to the aarch64 interpreter and dies with `ELOOP`, including the shell needed to
undo it. The registration string is written with backslash escapes the **kernel**
parses, and clause 5 reads the magic back and refuses to run a payload unless it
is the full 40 hex characters.

⭐ **2026-09-09, the operator pointed at
`pkgforge-dev/docker-archlinux`'s `.github/workflows/build-deploy.yml`, line 226
onwards, as bearing on the `powerpc64le` blocker. It bears on a DIFFERENT
blocker, and saying which is the finding.**

| axis | what stops podbox | what the workflow is about |
| --- | --- | --- |
| **compile** | `syscalls` 0.8.1 gates powerpc, s390x and mips behind `asm_experimental_arch`, so `cargo check --target powerpc64le-unknown-linux-musl` dies with `error[E0554]: #![feature] may not be used on the stable release channel` | nothing |
| **run** | nothing yet, because nothing builds | `docker/setup-qemu-action` ships nine emulators and `qemu-ppc`/`qemu-ppc64` are not among them, so a big-endian PowerPC image has no interpreter unless one is registered by hand |

⛔ **And the compile blocker was already measured NOT to be a rustc limit.**
Clause 3 of `experiments/260-multiarch.sh` asks rustc directly rather than
believing the cfg, and `experiments/results/multiarch.txt` records the answer:
powerpc64 inline assembly **compiles on stable rustc 1.98.1**. So the gate is
the crate's and it is stale, which means podbox can clear it without waiting for
anybody: take the numbers from `linux-raw-sys` and carry its own trap for these
two architectures. [T-0912](deps.md) is that work.

⭐ **What the workflow does contribute is the runtime half, and it corroborates
podbox's design rather than changing it.** It registers `qemu-ppc` through
`multiarch/qemu-user-static --reset -p yes`, asserts the registration took by
reading `/proc/sys/fs/binfmt_misc/<handler>` back, and checks the `F` flag,
because without it the kernel opens the interpreter inside the container's own
filesystem. ⚠ That is the same three things `crates/podbox-probe/src/binfmt.rs`
reads and the same reason `experiments/260-multiarch.sh` reads its magic back
before proceeding. podbox's answer on a machine with no such registration is
already the right one: refuse by name, with the ELF machine it looked for.

---

### T-0912 The powerpc gate is the crate's and it is stale, so podbox can clear it

Source:      Measured by `experiments/260-multiarch.sh` clause 3 on 2026-09-09, and raised again by the operator on the same day
Category:    deps
Priority:    P2
Effort:      M
Status:      open

Problem:     `powerpc64le` is the one architecture the workspace does not build
             for, and [T-0911](deps.md) records it as blocked on `syscalls`
             0.8.1's `asm_experimental_arch` gate. A blocker nobody can clear is
             a fact; this one can be cleared here.
Premise:     ⭐ **Measured, and the measurement is why this is an entry rather
             than a wish.** Clause 3 of `experiments/260-multiarch.sh` compiles
             powerpc64 inline assembly against the real rustc rather than
             believing the crate's cfg, and
             `experiments/results/multiarch.txt` records that it **compiles on
             stable rustc 1.98.1**. The gate is therefore the crate's and not
             the compiler's.
             ⚠ Clause 2 of the same script goes **red** the day `powerpc64le`
             starts building, which is the point: it is how this project finds
             out rather than a failure to suppress.
             ⚠ The runtime half is separate and is not this entry: an emulator
             for big-endian PowerPC is absent from `docker/setup-qemu-action`,
             which [T-0911](deps.md) now records.
Approach:    Take the syscall numbers for powerpc64, s390x and mips from
             `linux-raw-sys`, which podbox already depends on for the kernel
             structs, and carry podbox's own trap for those architectures in
             `crates/podbox-probe/src/sys.rs` beside the ones it already has.
             ⛔ Not a fork of `syscalls` and not a patch to it in this tree: the
             numbers are already available from a crate podbox takes, and one
             more source of them is one more place for them to disagree.
             ⚠ Then flip clause 2's expectation in the same change, because a
             clause asserting a blocker that has been cleared is a check that
             fires on the truth.
Decision:    Clear it here rather than wait for a `syscalls` release. Waiting is
             a dependency on somebody else's schedule for a value this project
             has already measured, and `docs/AGENTS.md` absolute 5 is that a
             blocked entry names what would clear it.
Prove:       `./experiments/260-multiarch.sh` reports 7 architectures checking the workspace and 0 blocked, and clause 3 says the gate was the crate's
