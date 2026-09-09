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
Status:      done 2026-09-09

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
Prove:       `./experiments/320-cli-contract.sh` clauses 1 to 3, whose first clause is `podbox system info --format '{{json .Parity}}' | jq -e 'length >= 60 and all(.status | IN("Native","Degraded","Stub","None"))'`

**Done, 2026-09-09.** `crates/podbox-cli/src/parity.rs` is the table,
`crates/podbox-cli/src/system.rs` is `podbox system info`, and
`experiments/320-cli-contract.sh` drives all three of the contracts below
against the shipped binary.

| | |
| --- | --- |
| rows | **131** |
| of which verbs | **53** |
| statuses used | Native, Degraded, Stub, None, and no fifth |
| rows with no reason | **0** |

⭐ **THE TABLE DECIDES, and that is the whole difference from a table that
merely describes.** Every argument beginning with `-` goes through
`parity::admit` before any match arm, so the two failures a hand-written parser
has are both structural rather than discouraged: a flag with no row cannot be
quietly accepted, because it never reaches an arm; and an arm for a flag with no
row is unreachable, because `admit` refused it first. A unit test walks the
table and asserts every row the table admits reaches an arm, ⚠ **by the exit
code rather than by a shape parsing**: `--pull` takes a closed set of values and
rejects anything a test could invent, so what the test asserts is that no shape
returns the fallback arm's own `EXIT_RUNTIME_ERROR`.

⭐ **The refusal carries the row's own reason**, so a caller that reads the
table and then runs the verb gets the same sentence back from the same place:
`podbox run --name c1` exits 2 with "`--name` is in the parity table with status
None: a name identifies a container, and podbox has none until M4", and
`podbox ps` exits 125 with that verb's row. Clause 3 asserts the message a verb
prints is byte-for-byte the note the table published.

⛔ **`--format` grew exactly one function, `json`.** `{{json .Parity}}` is
docker's own spelling and it is T-0801's `Prove`, so refusing it would have made
the acceptance unwritable. It is not a general function call: a verb declares
which of its fields are already documents, `{{json .X}}` on a plain string
quotes and escapes it, and every other function is still refused by name. ⚠ The
alternative, guessing from the value's first byte, makes a string that happens
to begin with `[` come out unquoted, which is a wrong answer that parses.

⚠ **A `None` row is not a placeholder.** It is the sentence a caller gets
instead of a flag being ignored, so deleting one makes podbox quieter and less
honest rather than smaller. A test asserts every one of them carries a reason
longer than a milestone number, which caught eleven rows whose whole note was
"the lifecycle is M4".

---

### T-0802 docker's exit codes, unaltered

Source:      `TOOL.md` section 6.8
Category:    cli
Priority:    P0
Effort:      S
Status:      done

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


**Done 2026-09-09, and the entry's own description of docker's convention was
incomplete.** `experiments/330-exit-codes.sh` runs the same case through both
binaries on one host and compares the numbers, which is the only way this can be
asserted: docker's contract is not written anywhere podbox can read, it is what
the binary does.

⭐ **THE DISCRIMINATOR IS NOT THE ONE THE ENTRY DESCRIBES, AND IT IS THE
FINDING.** docker exits **125** for anything its FLAG PARSER refuses and **1**
for anything the verb refuses afterwards. Measured on docker 29.3.1:

| case | docker | what podbox did before |
| --- | --- | --- |
| `run --badflag`, `--pull=bogus`, `--memory=notasize` | **125** | 2 |
| `images --format '{{.Nope}}'`, `run` with no image | **1** | 2 |
| `rmi no-such-image` | **1** | 125 |
| a verb neither tool has | **1** | 125 |
| the bare name, no arguments | **0**, help on stdout | 125 |
| the command found and not invocable | **126** | **127** |
| the command not found | 127 | 127 |
| the payload's own status, `128+N` for a signal | as docker | as docker |

Four of those eight were wrong. ⛔ The 126/127 one is the one that costs a
caller most: every `execve` failure was folded into "not found", so
`podbox run alpine /etc/passwd` said 127 where docker says 126, and a script
branching on 127 retries with another path. The split is on the **errno**:
`EPERM`, `ENOEXEC`, `EACCES`, `EISDIR` and `ETXTBSY` are 126; `ENOENT` and
anything about resolving the path are 127.

⭐ **The codes now live in ONE file**, `crates/podbox-probe/src/exit.rs`, and
are served as data by `podbox system info --format '{{json .ExitCodes}}'`.
⛔ They had been written out in **three**: `podbox-image::error`,
`podbox-cli::main` and `podbox-enter`, and two of the copies had already
diverged -- `main.rs` still carried the old usage code while `error.rs` carried
the corrected one, so two verbs of one binary disagreed about what a flag error
is. `scripts/common/exit-codes.sh` is how a shell script reads the table, and
six clauses across four experiments read it there instead of carrying a `2`.

⛔ **A fixup never moves the payload's code**, which is what
[T-0409](complete.md) depends on: clause 5 of the measurement runs the same
payload with and without the completion layer's source rewrite and asserts both
report the payload's own 7.

Prove, run 2026-09-09:

```
$ ./experiments/330-exit-codes.sh
  15 cases, podbox and docker agree on every one
  exit 0
```

---

### T-0803 Answer to `docker` and `podman` on PATH

Source:      `TOOL.md` section 2.0, section 6.8
Category:    cli
Priority:    P1
Effort:      S
Status:      done 2026-09-09

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
Prove:       `./experiments/320-cli-contract.sh` clauses 4 and 5: both names run the payload and say which tool ran, and the `docker` name is refused where a daemon answers unless `--force`

**Done, 2026-09-09.** `crates/podbox-cli/src/names.rs`, the banner line in `run`
and `exec`, `podbox system install-names`, and clauses 4 and 5 of
`experiments/320-cli-contract.sh`.

⭐ **Multicall on `argv[0]`, and the banner names which name was used.** Taking
the name is the product requirement; taking it silently is what `TOOL.md`
section 4.1 forbids, so every `run` and `exec` reached as `docker` or `podman`
carries one extra line saying this is podbox and where the differences are
listed.

⛔ **The daemon check is on a REACHABLE DAEMON and it is a real request.** The
socket named by `$DOCKER_HOST`, or `/var/run/docker.sock`, is connected to and
asked `/_ping` under a two-second timeout, so a stale socket file with nothing
behind it reads as absent, which is the state the machines podbox is for are
actually in. ⚠ A `$DOCKER_HOST` that is not a `unix://` socket is a THIRD state:
podbox says the guard itself is degraded and continues, which is the half of
`dockless`'s posture worth keeping.

⭐ **The ruling is about the `docker` name only.** On this machine, where a
daemon answers, `podbox system install-names` installs `podman`, refuses
`docker` with the reason, and exits 125; `--force` installs it. ⚠ Clause 5 reads
which state the machine is in first and asserts the other half where it can, so
a machine with no daemon reports the refusal half as unmeasured rather than as a
pass.

⛔ **Symlinks, never copies and never a wrapper script**, and the clause asserts
it on the filesystem rather than on an exit code. A file that is already a link
to this binary is left alone; anything else needs `--force`.

---

### T-0804 The honesty rules, and one switch that makes every degradation fatal

Source:      `TOOL.md` section 4.1, section 6.8; `paper_final.md` section 10.1
Category:    cli
Priority:    P0
Effort:      M
Status:      done

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


**Done 2026-09-09.** All four rules, and the third and fourth needed a defect
fixed rather than a switch added.

1. **The banner prints on every `run` and `exec`**, suppressible by
   `<store>/config` with `banner = quiet` and never from the command line. ⭐ A
   file a machine's operator sets once is the Decision: `ruri`'s
   `--disable-warnings` is the `sandlock` failure with a flag in front of it.
   ⚠ Whether a machine has suppressed it is itself reported, by
   `podbox system info --format '{{.Banner}}'`, because a silent banner nobody
   can tell from an absent one is the same shape again. ⛔ A `--strict` refusal
   prints whether or not the banner is quiet: the switch silences a notice,
   never a refusal.
2. **A request that requires isolation fails with a named reason.** Every
   isolation flag -- `--network`, `--privileged`, `--cap-add`, `-m`, `--cpus`,
   `--hostname`, `-v`, `-p`, `-u` -- is a `None` row and
   [T-0801](#t-0801-the-verb-and-flag-parity-table-as-data)'s `parity::admit`
   refuses it up front with the row's own reason.
3. **`inspect` reports the true mode per container**, and the container record
   now carries `Complete.Fixups` and `Complete.Degraded` beside `Rung`. A
   `/dev/null` that is a regular file is part of the mode, and a report that
   left it out would say devices exist when they do not.
4. ⛔ **THE FOURTH RULE WAS BEING BROKEN, AND BY THE BANNER ITSELF.** The banner
   printed `mode=` **the rung the PROBE selected**, and `podbox_enter` performs
   a plain `chroot` on every machine. On this host, where the probe selects
   `namespace`, every `podbox run` printed

   ```
   podbox 0.1.0: mode=namespace (namespaces: as configured; mounts: full; ...)
   ```

   for a payload that had no namespace and no mounts at all. It now reads the
   rung the entry sequence IMPLEMENTS, from one constant
   (`podbox_enter::ENTERED_RUNG`), and prints the machine's own answer beside
   it rather than dropping it:

   ```
   podbox 0.1.0: mode=chroot (namespaces: uts-only; mounts: none; devices: real;
   ownership: real; network: host-shared; pids: host-shared)
   this mode does NOT provide: process, network, IPC or mount isolation
   ⚠ this machine would permit `namespace`; podbox's entry sequence is `chroot`
     and creates no namespace and mounts nothing
   ```

⭐ **`--strict` reads three inputs and names every reason at once**, rather than
the first one it finds:

- **the flags this invocation passed**, against the parity table's own status.
  A `Degraded` or `Stub` row is a difference this run actually incurs;
- **the rung podbox entered with**, against `Selection::STRICT_FLOOR` -- the same
  constant `podbox probe --strict` gates on, not a second spelling of it;
- **the completion layer's report**, one row per fixup marked degraded.

⛔ A `None` row was already fatal without `--strict`, and has been since T-0801
made the table binding, so the switch is about the two statuses that otherwise
let a run proceed.

⚠ **THE `Prove` ABOVE CANNOT PASS AND ITS SECOND CLAUSE IS THE REASON.** It
expects `podbox run --rm --memory=1g ... true` to exit **0** and the same run
under `--strict` to fail. T-0801 landed after this entry was written and made
every `None` flag a refusal in the parser, so `--memory=1g` exits 125 with or
without `--strict` -- the entry's own rule 2, enforced earlier than it expected.
The rewrite, run 2026-09-09:

```
$ podbox run --rm public.ecr.aws/docker/library/alpine:3.20 true; echo $?
  0, and the banner carries `mode=chroot` and the two lines above
$ podbox run --strict --rm public.ecr.aws/docker/library/alpine:3.20 true; echo $?
  podbox run: --strict, and this run is degraded in 2 way(s). podbox refuses
  rather than running and letting the payload discover them:
    - the selected rung is `chroot` and not `namespace`, so the payload shares
      this machine's process table, network, IPC and mount namespaces
    - rewrote etc/mtab (T-0405): ...
  125
$ podbox run --rm --memory=1g public.ecr.aws/docker/library/alpine:3.20 true; echo $?
  podbox run: -m, --memory is in the parity table with status None: resource
  limits need a cgroup this runtime does not grant
  125
```

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
Status:      done

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


**Done 2026-09-09.** All three properties, and each is asserted rather than
reviewed.

1. **No code path reads stdin.** podbox never opens or reads fd 0: the payload
   inherits the caller's, which is what makes `-i` a `Stub` rather than a
   feature. ⚠ `experiments/330-exit-codes.sh` runs every case with stdin
   redirected from `/dev/null` under a `timeout`, so a read that blocked would
   arrive as exit 124 rather than as a hang nobody attributed.
2. **Every wait has an upper bound and a distinct outcome for reaching it.**
   The launcher's are [T-0602](supervise.md)'s. M5 added one more and it is
   bounded too: the HTTPS reachability probe [T-0411](complete.md) makes before
   rewriting a package source gets two seconds to connect and two to read, and
   a host that does not answer keeps its `http://` and is named on the banner.
3. **`statvfs` for blocks and inodes before every large write.** [T-0203](image.md)
   covers the download and the extraction; M5 added the third caller, and it is
   the one a caller with NOTHING can reach: the `/dev` shims are three megabytes
   into a rootfs that may be on a small tmpfs, so `space::require` runs before
   the first of them with the destination named in the error.

⚠ **Where podbox needs a tool it does not have, it says so at the point of use.**
The keyring half of [T-0406](complete.md) is the instance: podbox cannot run
`pacman-key` from outside the chroot, so the fixup reports the state and names
the two commands, rather than refusing the run. That is the Decision's own
shape, against `lilipod`'s at
`references/89luca89__lilipod/tree/pkg/utils/utils.go:194-214`, which refuses
before every subcommand including `pull`.

Prove, run 2026-09-09:

```
$ podbox run --rm public.ecr.aws/docker/library/alpine:3.20 true </dev/null; echo $?
  0
$ timeout 30 podbox pull public.ecr.aws/docker/library/alpine:3.20 </dev/null; echo $?
  0, and never 124
```

---

### T-0807 `podbox images --format` refuses a template no verb can answer, before it looks at the store

Source:      Found by driving the CLI cold against an empty store
Category:    cli
Priority:    P2
Effort:      S
Status:      done 2026-09-08

Problem:     A `--format` template was validated inside the loop over records.
             An empty store means zero iterations, so an invalid template was
             never seen: `podbox images --format '{{.Nope}}'` printed nothing
             and exited 0, and a caller's typo read as an empty result set.
Premise:     ⭐ **Measured by running it**, not by reading it, on 2026-09-08.
             Three templates exited 0 against an empty store where all three are
             refusals: an unknown field, an unterminated `{{`, and docker's
             `table` prefix, which rendered the word `table` beside the values.
Approach:    One walk over the template, shared by a `check` that has no record
             in hand and a `render` that does, in
             `crates/podbox-cli/src/format.rs`. Both verbs call `check` before
             the store is opened, against a named list of their own fields.
             ⛔ The field-name list and the field builder are two places holding
             one value, so a test asserts they agree, which is the remedy
             `docs/conventions/forbidden-patterns.md` names for exactly that row.
Decision:    Refuse `table` rather than rendering it as literal text. It selects
             a column layout in docker and podbox does not have one, so printing
             it is output shaped like something podbox does not do.
Prove:       `podbox images --format '{{.Nope}}' >/dev/null 2>&1; test $? -eq 2`

**Done 2026-09-08.** Driven against an empty store: the unknown field, the
unterminated brace, the `table` prefix and a pipeline all exit 2 and name what
is wrong; `{{.Digest}}` exits 0. `inspect` reports a bad template before it
reports a missing image, because the template is the caller's own input.
