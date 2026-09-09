# interpose

`crates/podbox-interpose`. `TOOL.md` section 6.7, milestone M6.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

⛔ **`interpose` is a compatibility tier and never a security property**, and
the banner says so.

⭐ **Interposition is two jobs, and calling them one is the mistake the research
measures.** Path virtualization is proven to work on this runtime with
`mount(2)` denied. Ownership virtualization is a separate job that a path
interposer does not do, and it is the half that clears the wall stopping the
most tools. `pathmap` is the corpus's evidence for both halves at once, because
it has the first and demonstrably not the second.

---

### T-0701 The cdylib build constraints

Source:      `TOOL.md` section 6.7; `experiments/results/interposer-libc.txt`
Category:    interpose
Priority:    P0
Effort:      M
Status:      open

Problem:     This object runs inside other people's processes. A default Rust
             cdylib exports `rust_eh_personality` and friends into every process
             it is loaded into, allocates on paths it has intercepted, and can
             deadlock its host.
Premise:     ⭐ **Measured here.** `experiments/60-interposer-libc.sh` check A
             records that the crate cannot be built as a cdylib at all under the
             workspace's own `+crt-static`:

             ```
             error: cannot produce cdylib for `podbox-interpose` as the target
             `x86_64-unknown-linux-musl` does not support these crate types
             ```

             `scripts/build-interpose.sh:9-15` records why the fix is `RUSTFLAGS`
             and not a crate-local config: cargo merges config files up the
             directory tree and appends the parent's rustflags **after** the
             child's, so a local `-crt-static` is followed by the root's
             `+crt-static` and loses.
Approach:    Four constraints, none optional:

             1. a version script exporting **only** the interposed symbols;
             2. no allocation and no locks on an interposed path;
             3. no `println!`; write to fd 2 directly;
             4. it must survive the payload forking, which every workload here
                does constantly.

             ⚠ **Constraint 2 is too strong as stated and the corpus says so.**
             `references/pkgforge-dev__cross-libc-dlopen/tree/src/cross-libc-dlopen.c:585-598`
             holds a lock on an interposed path and writes down what makes it
             safe: it is a `__sync_lock_test_and_set` spin with `sched_yield`
             rather than a pthread mutex, and **nothing under it can re-enter
             it**. Without any lock, two threads both pass a bounds check and
             both post-increment an index, writing one entry past the end of a
             table. The rule podbox holds is **no re-entry under the lock**.
             `references/fritzw__ld-preload-open` paid for the other half of
             this twice: tracker PR #4 "fix memory corruption" and PR #3 "Cache
             the results of dlsym".
             ⚠ **And forwarding re-enters the payload's allocator, which is
             fine.** `fopen` returns a `FILE *` this object cannot construct and
             `opendir` a `DIR *`, so forwarding through `dlsym(RTLD_NEXT, ...)`
             is the only correct implementation, and it makes linking against a
             libc unavoidable: there is no raw-syscall object that dodges the
             question. What constraint 2 forbids is **this object's own**
             allocation on an interposed path, not the payload's.
Decision:    A `dlsym(RTLD_NEXT, ...)` result cached in a `static` per entry
             point, resolved on first use, as
             `references/VHSgunzo__pathmap/tree/path-mapping.c:603-606` does.
             The alternative, resolving on every call, is a `dlsym` inside every
             intercepted `open`.
Prove:       `./scripts/build-interpose.sh && nm -D --defined-only crates/podbox-interpose/target/x86_64-unknown-linux-gnu/release/libpodbox_interpose.so | grep -c ' T rust_eh_personality' | grep -qx 0`

---

### T-0702 One object per libc, and it must live inside the rootfs

Source:      `experiments/results/interposer-abi.txt`; `references/fritzw__ld-preload-open` tracker
Category:    interpose
Priority:    P0
Effort:      M
Status:      open

Problem:     Two separate failures share this entry because the fix is the same
             artefact. A preload object built against one libc cannot serve a
             payload built against another, and an absolute `LD_PRELOAD` path
             from outside a chroot does not resolve inside it.
Premise:     ⚠ **The premise as first written was wrong, and the correction is
             below rather than in place of it.** The entry was written expecting
             `experiments/60-interposer-libc.sh` to show a musl-linked object
             failing to load into a glibc payload. It shows no such thing: on
             this host the musl-target object records `libc.so.6` in
             `DT_NEEDED`, because `--target x86_64-unknown-linux-musl` hands the
             link to the host's own C toolchain and there is no musl here. The
             script now reads `DT_NEEDED` and **exits 2** rather than answering
             from an object that is not musl-linked.
             What is measured, and settles the requirement from the other side:
             `references/pkgforge-dev__cross-libc-dlopen/tree/docs/limits.md:20`
             records that `regoff_t` is 4 bytes on glibc and 8 on musl, and that
             the `FTW_*` constants are off by one, and that "the offset is
             compiled into the object, so no preload can reach it". ⭐ Symbol
             **versioning** across a libc boundary is bridgeable, by the memfd
             rewrite at
             `references/pkgforge-dev__cross-libc-dlopen/tree/src/cross-libc-dlopen.c:8-12`.
             Struct **layout** is not. An interposer that hands a `struct stat`
             back to the payload must use the payload's layout.
             Upstream agrees at the build level:
             `references/VHSgunzo__pathmap/tree/Makefile:13-19` carries separate
             `path-mapping-glibc.so` and `path-mapping-musl.so` targets.
             ⭐ **And the cross-libc load is now measured, in both directions,
             with both controls.** `experiments/results/interposer-abi.txt`
             check B, taken with `musl-gcc` installed and the C reference
             interposer as the subject:

             ```
             control glibc object -> glibc        rc=0
             glibc object -> musl payload         rc=127  Error relocating /i.so: __snprintf_chk: symbol not found
             control musl object -> musl          rc=0
             QUESTION B musl object -> glibc      rc=127  /lib/x86_64-linux-gnu/libc.so: invalid ELF header
             ```

             ⭐ **And on 2026-09-08 it was measured a second time, with podbox's
             OWN Rust cdylib rather than the C reference interposer, which is
             what the correction above said could not be done here.**
             `experiments/60-interposer-libc.sh` exits **0** for the first time,
             and `experiments/results/interposer-libc.txt` carries:

             ```
             linker for musl: scripts/zig-cc.sh (zig 0.16.0)
             x86_64-unknown-linux-musl      OK   286056 bytes  1 NEEDED entries
             musl-target DT_NEEDED   libc.so
             gnu-target  DT_NEEDED   libgcc_s.so.1 libc.so.6 ld-linux-x86-64.so.2
             musl object into a glibc payload: rc=127
               /usr/bin/env: error while loading shared libraries:
               /lib/x86_64-linux-gnu/libc.so: invalid ELF header
             ```

             What changed is the linker, not the conclusion. `rustc` passes
             `-lgcc_s` on the musl target even under `panic = "abort"` and
             `musl-tools` ships no musl-linked `libgcc_s.so.1`, so the default
             `cc` linked the "musl" object against the host's glibc and the arm
             could not run. `zig cc` carries its own `compiler-rt`.
             ⭐ **Two independent objects, one C and one Rust, now give the same
             answer through the same mechanism**, and the requirement no longer
             rests on a single reference implementation.

             ⭐ The mechanism is **not** symbol versioning and not struct
             layout: it is the **SONAME**. musl's libc declares none, so an
             object linked against it records `libc.so` in `DT_NEEDED`, and on a
             glibc host `/lib/x86_64-linux-gnu/libc.so` is a GNU ld script.
             The loader rejects it at the ELF header, before a symbol is read.
             That makes the discriminator one `readelf` away, with nothing run,
             which is what T-0709 turns into a selection rather than an attempt.
Approach:    Build one object per libc, embed both, and select on the payload's
             `PT_INTERP` (T-0706). Then place the selected object **inside the
             rootfs** before the chroot and set `LD_PRELOAD` to its path as the
             payload will see it.
             ⭐ The absolute-path half is the upstream maintainer's ruling,
             recorded in `references/fritzw__ld-preload-open/api/comments.json`
             on issue #5, 2024-12-04: a relative `LD_PRELOAD` "would break as
             soon as your preloaded process changes to a different directory and
             spawns a new process". `TOOL.md` section 6.7 says the object is "extracted
             to the store on first use", and the store is outside the chroot, so
             that placement does not survive step 3 of section 6.5.
             Also from that tracker, issue #6 and its 2025-02-17 comment: the C
             source needs `struct stat64` to `struct stat`, `DISABLE_FTW`, and a
             hand-defined `__OPEN_NEEDS_MODE` to build against musl at all.
             podbox declares the syscalls rather than the libc struct variants
             and avoids that whole class.
Decision:    Two objects, not one with runtime detection. Runtime detection
             cannot change a struct offset the compiler already emitted.
Status note: **no longer blocked.** The measurement this entry waited on is
             taken, by `experiments/80-interposer-abi.sh`, which needs only
             `musl-gcc` and not a musl `libgcc_s`. What remains is
             implementation, plus one narrower gap that does not block it:
             podbox's own Rust cdylib still cannot be linked against musl on
             this host, because rustc passes `-lgcc_s` on that target even under
             `panic = "abort"` and `musl-tools` ships no musl-linked
             `libgcc_s.so.1`. `experiments/60-interposer-libc.sh` names that
             shortage and exits 2 on it. A musl cross toolchain carrying its own
             libgcc clears it; the design above does not wait on that.
Prove:       `./experiments/80-interposer-abi.sh` exits 0, and `podbox run --rm alpine:latest sh -c 'grep -q "$(readlink -f /.podbox/interpose.so)" /proc/self/environ'`

---

### T-0703 Path virtualization: the entry-point set and `*at` resolution

Source:      `TOOL.md` section 6.7; `references/VHSgunzo__pathmap`
Category:    interpose
Priority:    P0
Effort:      L
Status:      open

Problem:     A payload asks for a path that is not there. Without a rewrite in
             front of every path-taking entry point, `-v` is a copy and nothing
             else.
Premise:     ⭐ **Measured here, and it corrects a number in the
             specification.** `TOOL.md` section 7 records pathmap as "129 interposed
             entry points". Building
             `references/VHSgunzo__pathmap/tree/path-mapping.c` with its own
             Makefile and reading `nm -D --defined-only` gives **129 exported
             defined symbols**, of which **113** are libc entry points and 16
             are internals with a `_` or `pm_` prefix. 129 is the symbol count;
             113 is the entry-point count.
             ⭐ **And the family that is easiest to under-cover is measured.**
             `experiments/results/interpose-symbols.txt` checks A and B: an
             interposer defining `execve` alone catches **1 of 7** exec entry
             points, and the same seven with every entry point defined catch
             **7 of 7**. `execvp`, `execl`, `execlp`, `posix_spawn`,
             `posix_spawnp` and `execv` each resolve the path themselves and
             reach glibc's internal exec, which does not go through the PLT.
             Arm B is what makes arm A's zeros the entry points rather than a
             broken harness.
             ⚠ On this host `execvp` is imported by 14 shared objects,
             `posix_spawn` by 12 and `posix_spawnp` by 7, among them
             `libglib-2.0.so.0`, four `libpython3.*`, `libdbus-1`, `libarchive`,
             `libmagic` and `libsystemd`. A count on another host will differ;
             the mechanism will not.
             ⭐ **Keep both the `64` and the plain names, and the `__xstat`
             shape too.** Measured: all four `libpython3.*` and
             `libglib-2.0.so.0` import only `stat64`, `lstat64` and
             `fstatat64@GLIBC_2.33`, while `libarchive` and `libdbus-1` import
             the plain `stat`, `lstat` and `fstatat`. Both sets are live in one
             process, so neither can be dropped as a duplicate. `__xstat`,
             `__lxstat` and `__fxstatat` are the pre-2.33 shape and glibc 2.39
             still exports them at `GLIBC_2.2.5` and `GLIBC_2.4`, so they are
             not dead code either: the **payload's** libc decides which shape
             its libraries call, not the host the interposer was built on.
             ⚠ The adopted mechanism already defines all of them. Measured
             against the built object: every exec entry point above is present,
             and the path-taking names it does not define are `fstat`, `fstat64`
             and `fexecve`, which take a descriptor, plus `system` and `popen`,
             which take a shell command and reach the interposer through the
             child shell.
Approach:    ⭐ **Do not maintain a list. Produce the gap with a command.**
             `experiments/100-interpose-symbols.sh` check D reads a pinned
             rootfs from outside, enumerates the path-taking libc names its own
             binaries import, and subtracts what the interposer defines. On a
             pinned debian rootfs on 2026-09-08 that is **38 names reached, 3
             not defined**: `eaccess`, `mount` and `umount`. A list somebody
             maintains rots; a number a command produces does not.
             ⚠ Read the rootfs from **outside**. A reader run inside the image
             measures that image's package list rather than its binaries, and
             most images ship no `readelf` at all. It is also the wrong
             position: podbox extracts a rootfs and inspects it from outside.
             Cover the families, not a list of names: `open*`, `stat*`, `exec*`,
             `chdir`, `readdir`, `getcwd`, `readlink`, `realpath`, the xattr
             family, `link*`, `rename*`, `mkdir*`, `mk*temp`, `glob`, `scandir`,
             `ftw`, and the `*at` variants of each.
             ⭐ The `*at` mechanism to copy is
             `references/VHSgunzo__pathmap/tree/path-mapping.c:161-187`
             `absolute_from_dirfd()`: a relative path is made absolute by
             `readlink`ing `/proc/self/fd/<dirfd>`, and `/proc/self/cwd` for
             `AT_FDCWD`, before any matching happens.
             ⚠ **And one defect in it that podbox must not copy.** On a
             `readlink` failure that function does `return path`, handing the
             **unmapped relative path** to the real call with no diagnostic. A
             path that could not be resolved then reaches the real tree
             silently. podbox fails the call with `ENOENT` and writes one line
             to fd 2, because a silent escape from a virtual view is the
             `sandlock` failure mode in another costume.
Decision:    A macro over the families, as
             `references/VHSgunzo__pathmap/tree/path-mapping.c:250-262` does, so
             a new entry point is one line rather than a copied body. Hand-write
             only the ones whose argument shape the macro cannot express, which
             in pathmap is `fchownat` at
             `references/VHSgunzo__pathmap/tree/path-mapping.c:1242-1256`.
Prove:       `./experiments/50-interpose-tier.sh` exits 0 and `nm -D --defined-only crates/podbox-interpose/target/x86_64-unknown-linux-gnu/release/libpodbox_interpose.so | grep -c ' T ' | awk '$1 >= 60'`

---

### T-0704 Ownership virtualization: the half a path interposer does not have

Source:      `TOOL.md` section 6.7, `paper_final.md` section 9.3; `references/salsa-debian__fakeroot`
Category:    interpose
Priority:    P0
Effort:      L
Status:      open

Problem:     `chown 0:42` fails identically with and without a path interposer
             loaded. This is the wall that stops the most tools, and clearing it
             is what makes M6 worth doing.
Premise:     ⭐ **Measured, and the reason is visible at file and line.**
             `references/VHSgunzo__pathmap/tree/path-mapping.c:1240-1241` routes
             `chown` and `lchown` through the same `OVERRIDE_FUNCTION` macro as
             every other entry point: the **path** is rewritten and the uid and
             gid are passed through untouched. So the interposer sees the call
             and changes nothing about the ids, and the kernel answers `EINVAL`
             exactly as it would have without it.
             The model for the other half is fakeroot's
             `references/salsa-debian__fakeroot/tree/libfakeroot.c:875-907`:
             read the real metadata, overwrite `st_uid` and `st_gid` with the
             **intended** values, memo them, then attempt the real call, and
             swallow the failure.
Approach:    Intercept the `chown` family and return success after recording the
             intended owner to the sidecar T-0302 writes. Report the memo back
             from the `stat` family, so a workload that asks who owns a file
             gets the image's answer. Do the same for `setuid`, `setgid` and
             `setgroups`: success plus an identity memo.
             ⛔ **fakeroot's errno test is wrong for this runtime and copying it
             would reproduce the bug.**
             `references/salsa-debian__fakeroot/tree/libfakeroot.c:903` is
             `if(r&&(errno==EPERM)) r=0;`, and there are eight such sites at
             `:903`, `:930`, `:960`, `:995`, `:1033`, `:1066`, `:1100` and
             `:1134`. **Here the errno is `EINVAL`**, so every one of them would
             return the error to the caller. podbox swallows `EPERM` **and**
             `EINVAL`, and records which it swallowed.
             The switch that decides whether to attempt the real call at all is
             `references/salsa-debian__fakeroot/tree/libfakeroot.c:669-678`
             `dont_try_chown()`, a cached read of an environment variable.
             podbox probes once instead of asking the user, and caches the
             verdict the same way.
Decision:    Probe once and cache, rather than fakeroot's environment variable.
             A user who has to know to set a variable has to know the wall
             exists, and the audience is automated.
Prove:       `podbox run --rm alpine:latest sh -c 'chown 0:42 /tmp/f && stat -c %u:%g /tmp/f' | grep -qx '0:42'`

---

### T-0705 Reverse mapping, so the payload reads back what it wrote

Source:      `TOOL.md` section 6.7; `references/VHSgunzo__pathmap`
Category:    interpose
Priority:    P1
Effort:      M
Status:      open

Problem:     A payload that opens `/mapped/x`, then calls `getcwd` or
             `realpath`, gets the real path back and notices the virtualization.
             `configure` scripts and build systems compare the two and fail in
             ways that name neither.
Premise:     Read at file and line.
             `references/VHSgunzo__pathmap/tree/path-mapping.c:145-152`
             `reverse_fix_path()` maps a real path back to its virtual name, and
             only for a path lying under one of the destination prefixes.
             `references/VHSgunzo__pathmap/tree/path-mapping.c:730-758` is
             `realpath` with the reverse map applied to the result.
             ⚠ A limitation stated in that code, at
             `references/VHSgunzo__pathmap/tree/path-mapping.c:752`: when the
             caller supplied the output buffer, "we cannot safely apply reverse
             mapping", so `realpath(p, buf)` is **not** virtualized while
             `realpath(p, NULL)` is.
Approach:    Reverse-map `getcwd`, `get_current_dir_name`, `readlink`,
             `realpath`, `canonicalize_file_name` and the `d_name` of every
             `readdir`. For the caller-supplied-buffer case podbox writes the
             virtual name when it fits and returns `ERANGE` when it does not,
             rather than silently returning the real one: `ERANGE` is a defined
             answer the caller handles and a real path is a leak.
Decision:    `ERANGE` over a silent real path. The buffer case is rare and a
             leaked host path in a `configure` script's output is a build that
             bakes in the wrong prefix.
Prove:       `podbox run --rm -v "$PWD:/mapped" alpine:latest sh -c 'cd /mapped && test "$(pwd)" = /mapped'`

---

### T-0706 Classify the payload and decline with a named reason

Source:      `TOOL.md` section 6.7, `paper_final.md` section 10.3
Category:    interpose
Priority:    P0
Effort:      S
Status:      open

Problem:     Starting a mode that cannot reach the payload fails later, deeper
             and less legibly than declining it.
Premise:     Read, and one row of it is measured in the research this project
             rests on: a libc interposer never sees a Go program's `lchown`,
             with or without cgo, because Go's `os` package issues it as a raw
             syscall.

             | ELF property | verdict |
             | --- | --- |
             | `PT_INTERP` present | dynamically linked; interposition may reach it |
             | `PT_INTERP` absent | static; interposition cannot reach it |
             | Go build markers present | unreachable regardless of linkage |
             | anything else | may still issue raw syscalls; no ELF property proves coverage |

             ⭐ The precise reason for row 2 is in the corpus:
             `references/pkgforge-dev__cross-libc-dlopen/tree/docs/limits.md:45`
             observes that a fully static binary "has no `LD_PRELOAD`
             mechanism, because there is no dynamic loader to honour it". It is
             not that the preload fails; nothing reads the variable.
             That page also splits the static case three ways and labels all
             three unverified, which is the honest state and worth copying.
             `references/apptainer__apptainer` is the fourth tool at the
             ownership wall whose unpacker is Go, so interposition cannot rescue
             it either.
Approach:    Read the ELF once, at `run` and at `exec`, and treat the answer as
             **advisory**. Where it says unreachable, decline the mode by name.
             ⭐ The reasoning to copy for why this happens before exec rather
             than after is
             `references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:133-148`:
             "exec of a valid ELF succeeds, and the loader's failure happens
             afterwards, in a process that is no longer ours." That is the last
             point at which anything can be said about it.
Decision:    Decline the tier, do not fall back to a copy silently. A `-v` that
             became a copy is a different program's semantics.
             The `ptrace` tier is not a fallback either: it is a separate rung
             that needs a usable `ptrace`, which this runtime denies, and
             `references/proot-me__proot/tree/src/cli/cli.c:135-138` shows what
             a tool that assumes otherwise tells the user.
Prove:       `podbox run --rm -v "$PWD:/mapped" golang:alpine /usr/local/go/bin/go version 2>&1 | grep -q 'interpose: declined'`

---

### T-0707 The paths that must not be rewritten

Source:      `references/dex4er__fakechroot`; `TOOL.md` section 6.7
Category:    interpose
Priority:    P1
Effort:      S
Status:      open

Problem:     Rewriting every path breaks the interposer's own machinery. The
             `*at` resolution reads `/proc/self/fd/<n>`, the classification
             reads the payload's ELF, and the sidecar lives in the store. If
             those go through the map, the interposer virtualizes itself.
Premise:     Read at file and line.
             `references/dex4er__fakechroot/tree/src/libfakechroot.c:116-128`
             parses `FAKECHROOT_EXCLUDE_PATH` into a prefix list, and
             `references/dex4er__fakechroot/tree/src/libfakechroot.c:174-186`
             tests every path against it and returns early on a match, taking
             care that a prefix matches only at a component boundary.
             ⛔ fakechroot is LGPL-2.1 and is read only. The mechanism is
             described here; no line of it is copied.
             `references/dex4er__fakechroot/tree/src/` holds 153 `.c` files, one
             per entry point, which is the other half of what the tree
             demonstrates: coverage is per entry point and there is no shortcut.
Approach:    A built-in exclusion list, not only a user-supplied one:
             `/proc/self/fd`, `/proc/self/cwd`, `/proc/self/exe`, the store
             path, and the interposer's own object. Match at a component
             boundary so `/proctor` is not excluded by `/proc`. Then accept a
             user list on top of it.
Decision:    Built-in first, user list second, and the built-in entries are not
             removable. A user who removes `/proc/self/fd` from the list gets an
             interposer that cannot resolve `*at` calls, and the failure names
             nothing.
Prove:       `podbox run --rm -v "$PWD:/mapped" alpine:latest sh -c 'readlink /proc/self/exe | grep -qv /mapped'`

---

### T-0708 Intercept the operations the runtime cannot provide

Source:      `TOOL.md` section 6.7
Category:    interpose
Priority:    P2
Effort:      M
Status:      open

Problem:     A payload that calls `mknod`, `mount`, `unshare` or `clone` with
             namespace flags gets an error the runtime already knows the reason
             for, and the payload usually stops rather than adapting.
Premise:     Read. `TOOL.md` section 6.7 lists four: `mknod` becomes a regular file,
             `mount` becomes an in-memory table plus success, `unshare` becomes
             success and a memo, `clone` has its namespace flags stripped before
             the real call.
             pathmap already exports `mknod`, `mount_setattr`, `move_mount`,
             `open_tree`, `pivot_root`, `umount2` and `chroot`, which is
             evidence that the entry points are reachable; what it does with
             them is rewrite the path and call through.
Approach:    Implement the four. ⛔ Every one of them is a lie to the payload and
             each must be counted and reported: `inspect` carries the tally, and
             the banner says the tier is emulating them. A `mount` that returned
             success and mounted nothing is exactly the class of defect the
             honesty rules exist for, and the only thing that makes it
             acceptable is that podbox says so.
Decision:    Strip namespace flags from `clone` rather than failing it. Failing
             it stops shells and build systems that set them speculatively;
             stripping them produces a process that works with less isolation,
             which is the mode already reported.
             ⚠ Do not strip `CLONE_NEWNET` silently: an empty netns is
             `ENETUNREACH` for every connection, which reads as a network outage
             rather than as a stripped flag. Report it.
Prove:       `podbox run --rm alpine:latest sh -c 'mknod /tmp/n c 1 3 && test -f /tmp/n' && podbox inspect --format '{{.Interpose.Emulated.mknod}}' "$(podbox ps -lq)" | grep -q '^[1-9]'`

---

### T-0709 Select the interposer by `DT_NEEDED`, and refuse on the version predicate

Source:      `experiments/results/interposer-abi.txt`
Category:    interpose
Priority:    P0
Effort:      M
Status:      done

Problem:     T-0702 establishes that one object per libc is required. That
             leaves the question of which object a given payload gets, and
             answering it by trying one and seeing whether it loads costs a
             failed container and produces an error naming a symbol, which
             sends the reader to debug the symbol rather than the mechanism.
Premise:     ⭐ **Both facts are one `readelf` away, measured with no run.**
             `experiments/results/interposer-abi.txt`:
             check A, the SONAME discriminator. glibc's libc declares SONAME
             `libc.so.6`; musl's declares none, so an object linked against it
             records `libc.so` in `DT_NEEDED`. There is no ambiguity and no
             heuristic.
             check C, the version predicate, **predicted from ELF and then
             confirmed by the loader**: the object imports up to `GLIBC_2.34`,
             the pinned payload libc declares up to `GLIBC_2.31`, the prediction
             was `refused`, and the observed failure was
             `version 'GLIBC_2.34' not found (required by /i.so)`. The loader
             named the same version the prediction did.
             check D, the trap that makes a naive implementation refuse
             everything: a shipped `libc.so.6` is stripped. On this host
             `.symtab` has **0** defined symbols and `.dynsym` has **3136**. A
             reader asking `.symtab` concludes the payload's libc defines
             nothing, refuses every artefact, and reads exactly like a check
             that works.
Approach:    At extract time, for the rootfs just written:
             1. read the rootfs's libc `DT_NEEDED` and SONAME and pick the
                matching object;
             2. for every symbol the chosen object imports, assert the payload's
                libc defines it, **at a symbol version that libc declares**,
                reading `.dynsym` and never `.symtab`. Comparing names alone
                misses the common real failure, which is a build host newer than
                the target;
             3. on a mismatch, refuse **by name**: say which libc the rootfs
                carries and which the object was built against. T-0706 owns the
                refusal channel and T-0108 the wording.
             ⚠ A coarse guard in front of it is one glob and its value is the
             message: a rootfs carrying `ld-musl-*` handed a glibc object should
             say so in those words, not through a relocation error.
Decision:    Select and assert, never try. The assertion is cheap, it runs
             before anything is executed, and it turns a runtime failure inside
             somebody's container into a refusal with a reason.
Prove:       `./experiments/80-interposer-abi.sh` exits 0, and its check E is this reader answering what the loader answered in A, B and C


**Done 2026-09-09.** `crates/podbox-enter/src/abi.rs`, driven by
`podbox system abi <object> <libc>` and asserted by check E of
`experiments/80-interposer-abi.sh`.

⛔ **The `Prove` above was amended, and the reason is that its second half could
not fail.** It read `podbox run --rm alpine:latest true 2>&1 | grep -qv 'symbol
not found'`; with no interposer to preload, no run can produce that string, so
the clause passes on a tree where nothing is implemented. Check E is the
assertion that can go red: the reader's verdict is compared with what the loader
actually did in checks A, B and C, over the same objects and the same pinned
payload libcs.

```
== E. podbox's own reader, against the same four situations
  control glibc obj -> host libc     rc=0    admitted
  glibc obj -> musl libc             rc=1    refused (names musl and glibc)
  control musl obj -> musl libc      rc=0    admitted
  musl obj -> host glibc libc        rc=1    refused
  C's situation, read not run        rc=1    refused, naming GLIBC_2.34
ok   E: the reader's answer is the loader's, on every arm that ran
```

⭐ **The libc is FOUND rather than named.** There is no table of
`lib/x86_64-linux-gnu/libc.so.6` in the reader: the payload's own `PT_INTERP`
names the loader that will run it, and on musl that file IS the C library while
on glibc the libc sits in the loader's own directory. A table would be a list of
the distributions somebody thought of, and `crates/podbox-complete/src/identity.rs`
already carries one for a coarser question.

Four things the implementation had to get right, and three of them were found by
running it rather than by reading:

1. ⛔ **`.dynsym` and never `.symtab`**, which is the entry's own trap and the
   only one that was known in advance. Check D measures it: 0 defined symbols in
   `.symtab` and 3136 in `.dynsym`;
2. ⛔ **A WEAK undefined symbol is not a requirement.** The first working reader
   refused its own control: the glibc object imports
   `_ITM_deregisterTMCloneTable`, `_ITM_registerTMCloneTable` and
   `__gmon_start__`, none of which `libc.so.6` defines, and it loads. GCC's
   `crtbegin` emits them `STB_WEAK` and the loader binds them to 0. A reader
   that treated them as requirements **refuses every object gcc produces**,
   which is the same shape as the `.symtab` trap: it reads exactly like a check
   that works;
3. ⛔ **The version is checked BEFORE the name, because that is the loader's own
   order.** The pinned glibc 2.31 payload does not define `dlsym` in `libc.so.6`
   at all -- glibc 2.34 merged `libdl` into `libc` -- so the symbol is both
   undefined and at an undeclared version. The loader says `version 'GLIBC_2.34'
   not found`; a reader asking "is it defined" first said `dlsym` is missing,
   which is true and sends the reader to look for `libdl` rather than at the
   build host;
4. ⛔ **`VER_FLG_BASE` is not a version.** The first entry of `.gnu.version_d` is
   the file's own SONAME wearing a version entry's clothes, so collecting it put
   `libc.so.6` in the declared list beside `GLIBC_2.39`, where it sorts after
   every real one and became the "declares up to" the refusal reported.

⚠ **What is still open is the OBJECT, not the reader.** Nothing preloads
anything yet: [T-0701](#t-0701-the-cdylib-build-constraints) and
[T-0702](#t-0702-one-object-per-libc-and-it-must-live-inside-the-rootfs) are the
cdylib and the placement, and this reader is what will choose between the two
objects they produce.

⭐ **A second thing this closed, in the harness rather than in podbox.**
`experiments/80-interposer-abi.sh` exited **2** on this container because
`musl-gcc` is absent from it, so question B and both musl arms of check E could
not be taken. It now falls back to `scripts/zig-cc.sh`, which is the compiler
`.cargo/config.toml` already names for the musl target and which carries the
musl sources it compiles against. The script exits **0** for the first time and
the conditions block says which compiler produced the object, because "the musl
arm ran" means something different depending on it.
