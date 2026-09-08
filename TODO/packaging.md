# packaging

`TOOL.md` section 3.4, section 6.7 and milestone M7.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

A single static binary. The interposer is the one object in the tree that is
not static, and it is embedded in the launcher rather than shipped beside it.

---

### T-1001 A single static binary with no `PT_INTERP`

Source:      `TOOL.md` section 3.4, section 5 M7
Category:    packaging
Priority:    P0
Effort:      S
Status:      done 2026-09-08

Problem:     The artefact must run on the target with no libraries present, and
             the memfd launch rung needs a static, dependency-free entrypoint.
Premise:     ⭐ **Measured here, on the empty skeleton.** `cargo build --release
             --target x86_64-unknown-linux-musl` succeeds and produces a
             389,656-byte static-PIE. `readelf -l` shows ten program headers and
             no `PT_INTERP`. One machine, one day: kernel `6.18.44-fc-v24`,
             rustc 1.94.1.
             The settings are `.cargo/config.toml:9-10` for `+crt-static` and
             `Cargo.toml:34-39` for the release profile.
Approach:    Hold it. This is the property every dependency decision in
             [deps.md](deps.md) is measured against, and the one a C dependency
             takes away.
             ⚠ A shell entrypoint skips the memfd rung entirely. That is why
             T-0803 installs `docker` and `podman` as symlinks rather than as
             wrapper scripts, and it is recorded in the corpus at
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:133-148`,
             which refuses memfd mode for an entrypoint with shared library
             dependencies and explains that after `exec` nothing can be said
             about it any more.
Decision:    `crt-static` per target rather than under `[build]`. Setting it
             globally also applies it to build scripts and proc macros compiled
             for the host, which is a link failure on some hosts.
Prove:       `cargo build --release --target x86_64-unknown-linux-musl && readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP | grep -qx 0`

**Done on the skeleton, and it stays open to regression rather than to work.**
The `Prove` command above was run on 2026-09-08 and exits 0:

```
$ file target/x86_64-unknown-linux-musl/release/podbox
ELF 64-bit LSB pie executable, x86-64, static-pie linked, stripped
$ readelf -l target/x86_64-unknown-linux-musl/release/podbox | grep -c INTERP
0
$ stat -c%s target/x86_64-unknown-linux-musl/release/podbox
389656
```

T-0910 wires the size into the gate so the property cannot be lost quietly.

---

### T-1002 Embed the interposer as bytes and place it inside the rootfs

Source:      `TOOL.md` section 4.3, section 6.7
Category:    packaging
Priority:    P1
Effort:      M
Status:      open

Problem:     One artefact, not two. And an `LD_PRELOAD` path from outside a
             chroot does not resolve inside it, so extracting the object to the
             store is not enough.
Premise:     Measured for the build half:
             `experiments/results/interposer-libc.txt` records that the crate
             builds for both `x86_64-unknown-linux-musl` and
             `x86_64-unknown-linux-gnu` with `-crt-static`, at 14,064 and
             267,680 bytes on this host.
             The placement half is T-0702's premise and rests on the upstream
             maintainer's ruling in
             `references/fritzw__ld-preload-open/api/comments.json`.
Approach:    Build both objects with `scripts/build-interpose.sh`, embed both as
             byte arrays, and on first use write the one T-0706 selected into
             the rootfs at a fixed path under a podbox-owned directory. Set
             `LD_PRELOAD` to that path as the payload will see it.
             ⛔ The interposer must not depend on the rest of the tree. It is
             excluded from the workspace at `Cargo.toml:20-25` for that reason,
             and a `path` dependency back into the workspace is a build failure
             rather than a review comment.
Decision:    Write it into the rootfs rather than keep it in the store and bind
             it in. There is no attach path on this runtime, so a bind is not
             available, and a copy per container is a few hundred kilobytes.
Prove:       `podbox run --rm alpine:latest sh -c 'test -r /.podbox/interpose.so && grep -q /.podbox/interpose.so /proc/self/environ'`

---

### T-1003 The launch ladder, and a single file with an embedded rootfs

Source:      `TOOL.md` section 5 M7, section 6.7; `references/qaidvoid__onelf`
Category:    packaging
Priority:    P2
Effort:      L
Status:      open

Problem:     A runtime that can only start from a filesystem cannot start on a
             machine whose writable paths are full, and the machines this is for
             have a 64 MiB `/tmp`.
Premise:     ⭐ Read at file and line, and the shape is worth copying whole.
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:128`
             names the order: memfd, then FUSE, then an ephemeral tmpfs, then a
             private run directory, then a persistent cache. Each **forced** mode
             refuses with a named reason rather than falling through:
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:216-218`,
             `:235-237` and `:255-257`. The on-disk mode runs only when asked
             for, and otherwise the tool refuses:
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:261-273`.
             ⭐ **And one mechanism that is easy to miss and costs a session to
             rediscover.**
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:125-131`:
             `ONELF_MODE` **requests** a mode and the mode actually chosen is
             reported under a **different** name, `ONELF_ACTIVE_MODE`, "so a
             packed app that launches another one does not hand it a directive".
             podbox has exactly that hazard: `podbox run` inside `podbox run`.
             The request variable and the result variable must not share a name.
             ⚠ On this runtime the ladder is shorter than onelf's: FUSE needs
             `/dev/fuse`, which `mknod` cannot create, and an ephemeral tmpfs
             needs a mount. Both rungs are refused here by probe, not omitted.
Approach:    Implement memfd, then the private run directory, then the
             persistent cache, each with a named refusal. Probe for the two
             rungs this runtime lacks rather than assuming their absence, so the
             same binary uses them where they exist.
             Where the payload's own relocation is the problem rather than the
             filesystem, `references/VHSgunzo__sharun` is the lineage that solves
             it and `references/VHSgunzo__ulexec` drives both rungs from one
             tool. Neither substitutes for section 6.7: they solve getting a binary to
             run from nowhere, not filesystem virtualization.
Decision:    Two environment variables, request and result, from the start. The
             single-variable form works until podbox runs inside itself, and
             then it is a bug that reproduces only under nesting.
Prove:       `PODBOX_MODE=memfd podbox run --rm alpine:latest true 2>&1 | grep -q 'memfd' && podbox run --rm alpine:latest sh -c 'test -z "$PODBOX_MODE" && test -n "$PODBOX_ACTIVE_MODE"'`

---

### T-1004 A reproducible build, and the artefact's own inputs recorded

Source:      `TOOL.md` section 10.9 via `paper_final.md` section 10.9
Category:    packaging
Priority:    P3
Effort:      M
Status:      open

Problem:     A single-file artefact whose inputs are not recorded cannot be
             traced back to what produced it, and the audience is automated and
             will not remember.
Premise:     Read.
Approach:    Record, in the binary and reported by `podbox version --verbose`:
             the git commit, the rustc version, the target triple, the digest of
             each embedded interposer object, and the achieved mode of the build
             (whether it is static). Verify that two builds of the same commit
             on the same toolchain produce the same bytes, and where they do not,
             name what differed rather than dropping the claim.
             ⛔ No fabricated number: if reproducibility is not achieved, the
             field says so.
Decision:    Report rather than promise. `TOOL.md` says a single-file artefact
             "should record its immutable inputs", which is a reporting
             requirement; bit-for-bit reproducibility is a stronger claim and it
             is measured before it is made.
Prove:       `./experiments/120-reproducible-build.sh` exits 0 or 1, never 2, and its output is committed to `experiments/results/`
