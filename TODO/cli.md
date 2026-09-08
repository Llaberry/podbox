# cli

`crates/podbox-cli`. `TOOL.md` section 6.8 and section 6.9.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

⛔ **An agent that knows docker must need zero new knowledge.** That is the
product requirement the whole specification exists for. Same verbs, same flags,
same exit codes; where a flag cannot be honoured it is accepted and reported in
the banner, and where a flag **requests isolation** it fails loudly with a
named reason.

---

### T-0801 The verb and flag parity table

Source:      `TOOL.md` section 6.8
Category:    cli
Priority:    P0
Effort:      L
Status:      open

Problem:     A tool that needs its user to learn its differences has not
             replaced anything. Thousands of agents reach for `docker` because
             it is the only container language they know.
Premise:     Read. `TOOL.md` section 6.8 is the table, in full, with a status per verb
             and per flag. Four statuses: **Native** (real semantics),
             **Degraded** (works, documented difference, stated in the banner),
             **Stub** (accepted, no-op, listed in the banner), **None** (fails
             with a named reason).
Approach:    Implement the table as data, not as a match arm per flag, so that
             `podbox system info` can print it and a test can assert every row
             is reachable. A flag with no row is a bug in the table, and the
             parser says so rather than ignoring it.
             ⭐ The posture to copy is `dockless`'s, at
             `references/ylang-ylang__dockless/tree/dockless/run.py:129-160`.
             `guard_run` refuses `run --name` up front because the engine
             beneath it cannot honour it, and warns hard when `--rm` names an
             existing container, citing the incident that produced the guard.
             ⛔ dockless has **no licence statement of any kind**
             ([reference-map.md](reference-map.md)), so the posture is described
             and adopted as a design and no line is copied.
             ⚠ Its degradation is worth noting from the other side: when it
             cannot determine the target it prints that the guardrail itself is
             degraded and continues. Naming the degradation of the guard is the
             part to keep.
Decision:    A table plus a generated parser, over a hand-written one. The table
             is what section 6.8 already is, and generating from it makes "the flag
             exists and is unlisted" impossible.
Prove:       `podbox system info --format '{{json .Parity}}' | jq -e 'length >= 60 and all(.status | IN("Native","Degraded","Stub","None"))'`

---

### T-0802 docker's exit codes, unaltered

Source:      `TOOL.md` section 6.8
Category:    cli
Priority:    P0
Effort:      S
Status:      open

Problem:     An automated caller reads the exit code first. A runtime that
             returns its own codes breaks every script that branches on
             docker's, and a runtime that improves a payload's code lies in the
             field read first.
Premise:     Read. docker's convention: the payload's own status for `run`, 125
             when the runtime itself could not run the command, 126 when the
             command was found and could not be invoked, 127 when it was not
             found.
Approach:    Return the payload's status verbatim from `run`, `start` in the
             attached case, and `wait`. Use 125, 126 and 127 for the three
             runtime-side cases and nothing else.
             ⛔ Never translate a payload's non-zero status into zero because a
             fixup succeeded, and never into non-zero because a fixup warned.
             That rule is what T-0409 depends on.
Decision:    Match docker rather than define a clearer scheme. Parity is the
             product.
Prove:       `podbox run --rm alpine:latest sh -c 'exit 42'; test $? -eq 42 && podbox run --rm alpine:latest /nonexistent; test $? -eq 127`

---

### T-0803 Answer to `docker` and `podman` on PATH

Source:      `TOOL.md` section 2.0, section 6.8
Category:    cli
Priority:    P1
Effort:      S
Status:      open

Problem:     An agent runs `docker`. If podbox is only reachable as `podbox`,
             it has replaced nothing on the machines it is for.
Premise:     Read. On the studied runtime `docker` on PATH is already a podman
             alias with no daemon, so the name is available and currently
             resolves to something that does not work.
Approach:    Multicall on `argv[0]`: `docker`, `podman` and `podbox` all enter
             the same parser, and the banner names which. Install the two extra
             names as symlinks, never as copies, so one binary is one artefact.
             ⚠ Check what is already on PATH before replacing it, and refuse to
             overwrite a real docker client without an explicit flag. A machine
             with a working daemon is a machine where podbox is the wrong tool.
Decision:    Symlinks over a wrapper script. A shell wrapper is a second
             artefact, it breaks the memfd rung (T-1001), and it is the exact
             thing `references/qaidvoid__onelf` records as skipping that rung.
             ⭐ **RULED by the operator on 2026-09-08, and no longer open:**
             podbox **refuses** to take the `docker` name where a working
             docker daemon is reachable, unless an explicit flag says otherwise,
             and says why in one line. A machine with a working daemon is a
             machine where podbox is the wrong tool.
             The alternative considered and rejected is deferring to the real
             daemon transparently by exec'ing it: it costs an exec on every
             call and it hides which tool ran, which is the class of thing
             `TOOL.md` section 4.1 exists to forbid.
             ⚠ The check is on a **reachable daemon**, not on a `docker` binary
             being present: on the target runtime `docker` on PATH is a podman
             alias with no daemon behind it, so a binary check would refuse on
             exactly the machines podbox is for.
Prove:       `ln -sf "$(command -v podbox)" /tmp/bin/docker && PATH=/tmp/bin:$PATH docker run --rm alpine:latest /bin/echo hi | grep -qx hi`, and podbox refuses to install the `docker` symlink where `docker info` succeeds unless the flag is given

---

### T-0804 The honesty rules, and one switch that makes every degradation fatal

Source:      `TOOL.md` section 4.1, section 6.8; `paper_final.md` section 10.1
Category:    cli
Priority:    P0
Effort:      M
Status:      open

Problem:     The audience is automated. An agent cannot notice that its
             "container" was a `chroot` the way a human skimming a log might, so
             the reporting is the product and not a disclaimer.
Premise:     Read, and both halves are in the corpus.
             `references/RuriOSS__ruri/tree/src/include/ruri.h:237-248` is the
             degradation macro that becomes fatal under one flag.
             `references/multikernel__sandlock` is the counter-example: a
             `--dry-run` that reports no changes while changing files, traced in
             T-0606.
Approach:    Four rules, enforced in the output path rather than in review:

             1. the banner prints on every `run` and `exec`, suppressible by
                config and never by default;
             2. a request that **requires** isolation fails with a named reason
                instead of silently using the host;
             3. `inspect` reports the true mode per container;
             4. no output may imply namespaces, cgroups or devices exist when
                they do not.

             ⭐ Add `--strict`, which turns every Degraded and every Stub into a
             refusal, so a caller that needs real semantics can demand them in
             one flag rather than checking the banner.
Decision:    Suppressible by config, never by default. `ruri` allows
             `--disable-warnings` to silence its degradation notices, which is
             the `sandlock` failure mode with a flag in front of it; podbox's
             equivalent is a config file a machine's operator sets once, and it
             cannot be set from the command line of a single run.
Prove:       `podbox run --rm --network=none alpine:latest true 2>&1 | grep -q 'network isolation'; test $? -eq 0 && podbox run --rm --memory=1g alpine:latest true; test $? -eq 0 && podbox run --strict --rm --memory=1g alpine:latest true; test $? -ne 0`

---

### T-0805 Diagnostics that name the operation, the errno, the mechanism and the remedy

Source:      `TOOL.md` section 6.9; `paper_final.md` section 10.8
Category:    cli
Priority:    P1
Effort:      M
Status:      open

Problem:     In this environment the errno alone actively misleads. `EINVAL`
             from `chown` reads as a bad argument and means an id that does not
             exist. `fork/exec <path>: operation not permitted` reads as a
             missing binary and is a denied `setgroups`.
Premise:     Read, and `TOOL.md` section 8 is a whole table of them. The target shape:

             ```
             cannot restore ownership of etc/shadow (uid 0, gid 42): EINVAL
               gid 42 is not mapped in this user namespace (/proc/self/gid_map: 0 1000 1)
               extracting without ownership; intended metadata recorded in .meta.jsonl
             ```

Approach:    Every failure carries four parts: the operation, the errno, the
             mechanism that produced it, and the remedy. The mechanism comes
             from T-0102's discriminator and the maps from T-0105, so the
             diagnostic quotes measured state rather than a guess.
             Encode `TOOL.md` section 8's table as the mapping from a raw failure to
             those four parts, so a new failure mode is a table row.
Decision:    Quote the actual `uid_map` contents rather than saying "an unmapped
             id". The map is one read (T-0105) and it is what turns a confusing
             errno into an explanation a reader can act on.
Prove:       `podbox pull alpine:latest 2>&1 | grep -A2 'gid 42' | grep -q 'gid_map'` or, where the interposer cleared it, `podbox inspect --format '{{.Ownership.Deferred}}' alpine:latest | grep -q etc/shadow`

---

### T-0806 Never prompt, never wait unbounded, and check space before every large write

Source:      `TOOL.md` section 11.3; [RULES.md](RULES.md) section 8
Category:    cli
Priority:    P0
Effort:      S
Status:      open

Problem:     A prompt with no terminal behind it is a hang, and a hang is total.
             The three ways a long autonomous run dies are the three ways podbox
             can make its caller die.
Premise:     Read, and one instance is in the corpus at file and line.
             `references/89luca89__lilipod/tree/pkg/utils/utils.go:194-214`
             checks `getsubids`, `newuidmap` and `newgidmap` with `exec.LookPath`
             and returns an unrecoverable error when any is missing, and it runs
             before **every** subcommand including `pull`. A dependency check
             ahead of every syscall wall is the same class of problem as a
             prompt: the tool refuses before it has done anything the user can
             use.
Approach:    Three properties, tested rather than reviewed:

             1. no code path reads stdin unless the user asked for it with `-i`;
             2. every wait has an upper bound and a distinct outcome for reaching
                it (T-0602);
             3. `statvfs` for blocks and inodes before every large write, with
                the destination named in the error (T-0203).

             ⚠ Where podbox needs a tool it does not have, it says so **at the
             point of use** and not at startup, so every verb that does not need
             it still works.
Decision:    Check at the point of use rather than up front. lilipod's shape
             makes `pull` fail on a machine where `pull` would have worked, and
             `pull` is the verb an agent reaches for first.
Prove:       `podbox run --rm alpine:latest true </dev/null && timeout 30 podbox pull alpine:latest </dev/null; test $? -ne 124`
