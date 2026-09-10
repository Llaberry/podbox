//! The verb and flag parity table, as DATA.
//!
//! [`TODO/cli.md`](../../../TODO/cli.md) T-0801, `TOOL.md` section 6.8.
//!
//! ⭐ **A tool that needs its user to learn its differences has not replaced
//! anything.** Thousands of agents reach for `docker` because it is the only
//! container language they know, so the differences have to be enumerable
//! rather than discoverable: `podbox system info --format '{{json .Parity}}'`
//! prints this table, and a caller decides from it before running anything.
//!
//! ⛔ **This is the table, not a description of one.** The parsers ask it
//! whether a flag exists, so a flag with no row here is refused by the parser
//! with a message saying the table has no row for it, rather than quietly
//! working. That is what makes "the flag exists and is unlisted" impossible
//! instead of merely discouraged.
//!
//! ⭐ The posture is `dockless`'s, at
//! `references/ylang-ylang__dockless/tree/dockless/run.py:129-160`: it refuses
//! `run --name` up front because the engine beneath it cannot honour it, rather
//! than accepting the flag and losing it. ⛔ dockless carries no licence
//! statement of any kind ([`TODO/reference-map.md`](../../../TODO/reference-map.md)),
//! so the posture is adopted as a design and no line of it is copied.
//! ⚠ Its other half is worth keeping too: where it cannot determine the target
//! it says the guardrail itself is degraded and continues. A guard that goes
//! quiet is worse than one that says it went quiet.

/// The four statuses `TOOL.md` section 6.8 defines, and podbox has no fifth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Real semantics: what docker does, for the same input.
    Native,
    /// Works, with a documented difference that the banner states every time.
    Degraded,
    /// Accepted and does nothing. Listed here, so a caller can see it is a
    /// no-op instead of concluding it worked.
    Stub,
    /// Fails, with a named reason. ⛔ Never silently ignored.
    None,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Native => "Native",
            Status::Degraded => "Degraded",
            Status::Stub => "Stub",
            Status::None => "None",
        }
    }
}

/// One row: a verb, or a flag of a verb.
///
/// ⚠ `flag` carries every spelling the parser accepts, comma-separated, because
/// the row is the flag rather than one of its names: a table listing `--env`
/// and a parser accepting `-e` is two answers to one question.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub verb: &'static str,
    pub flag: Option<&'static str>,
    pub status: Status,
    pub note: &'static str,
}

impl Row {
    /// True when `name` is one of this row's spellings.
    fn names(&self, name: &str) -> bool {
        match self.flag {
            None => false,
            Some(spellings) => spellings.split(',').any(|s| s.trim() == name),
        }
    }
}

use Status::{Degraded, Native, None as NoneStatus, Stub};

/// ⛔ THE TABLE. Every verb podbox answers to and every flag it accepts, plus
/// the docker verbs and flags it does not, each with the reason it does not.
///
/// ⚠ A `None` row is not a placeholder for future work: it is the sentence a
/// caller gets instead of a flag being ignored. Removing one makes podbox
/// quieter and less honest, not smaller.
pub const TABLE: &[Row] = &[
    // ------------------------------------------------------------- the verbs
    Row { verb: "run", flag: Option::None, status: Degraded, note: "enters a chroot, never a namespace. The banner names what the selected rung does not provide, on every run" },
    Row { verb: "exec", flag: Option::None, status: Degraded, note: "a fresh chroot re-entry sharing only the filesystem, never an entry into a running container's namespaces (T-0505)" },
    Row { verb: "pull", flag: Option::None, status: Native, note: "HTTPS only. A registry offering only http:// is a named refusal, never a downgrade" },
    Row { verb: "images", flag: Option::None, status: Native, note: "one record per platform of a tag" },
    Row { verb: "rmi", flag: Option::None, status: Native, note: "refuses an image a running container holds, and names it" },
    Row { verb: "tag", flag: Option::None, status: Native, note: "points a second name at one manifest digest; nothing is fetched" },
    Row { verb: "image", flag: Option::None, status: Native, note: "ls, rm, prune, tag, inspect, pull and extract" },
    Row { verb: "inspect", flag: Option::None, status: Degraded, note: "images only. podbox has no containers until M4, so a container reference is not resolvable" },
    Row { verb: "system", flag: Option::None, status: Degraded, note: "info, install-names and abi. df, events and prune are not implemented; the last two are podbox's own and docker has neither" },
    Row { verb: "info", flag: Option::None, status: Degraded, note: "podbox has no daemon, so the server half of docker's output is the rung this machine permits instead" },
    Row { verb: "version", flag: Option::None, status: Native, note: "one artefact, so there is one version and no client/server split" },
    Row { verb: "probe", flag: Option::None, status: Native, note: "podbox's own verb, with no docker equivalent: what this machine permits, and the rung podbox selects" },
    Row { verb: "extract", flag: Option::None, status: Native, note: "podbox's own verb, with no docker equivalent: unpack the layers and write the ownership sidecar" },
    // ⭐ M4, and each says the difference from docker's rather than implying
    // there is none. TODO/supervise.md T-0601 to T-0607.
    Row { verb: "create", flag: Option::None, status: Native, note: "writes a created record and starts nothing, as docker's does. ⚠ It is served by run's PARSER, so it takes run's flag set: those rows are listed once, under `run`, rather than copied here where the two could diverge (T-0801)" },
    Row { verb: "start", flag: Option::None, status: Native, note: "returns when the payload has reached its execve, established by a pipe rather than by a sleep (T-0602)" },
    Row { verb: "stop", flag: Option::None, status: Degraded, note: "SIGTERM then SIGKILL to the PAYLOAD. podbox has no PID namespace, so a grandchild that reparented is outside its reach and is not signalled" },
    Row { verb: "restart", flag: Option::None, status: NoneStatus, note: "not implemented: `stop` then `start` is the same thing and says which half failed" },
    Row { verb: "kill", flag: Option::None, status: Degraded, note: "signals the payload. A pidfd addresses one process; it does not reach descendants that reparent" },
    Row { verb: "rm", flag: Option::None, status: Native, note: "removes the record and the container's own directory; -f kills a running one first" },
    Row { verb: "ps", flag: Option::None, status: Degraded, note: "reads the container table, never /proc. A container whose launcher was killed reads `dead` with the time it was noticed, and no exit code (T-0604)" },
    Row { verb: "logs", flag: Option::None, status: Degraded, note: "the payload's stdout and stderr, interleaved into one file opened before the chroot. -f is not implemented (T-0605)" },
    Row { verb: "wait", flag: Option::None, status: Degraded, note: "blocks on the launcher, bounded. ⛔ A container podbox did not see end has NO exit code and `wait` refuses rather than printing one" },
    Row { verb: "cp", flag: Option::None, status: Degraded, note: "one file at a time, either way, gated through the containment check. A directory copy is not implemented" },
    Row { verb: "top", flag: Option::None, status: NoneStatus, note: "a chroot shares this machine's process table, so `top` would list the host's processes and call them the container's" },
    Row { verb: "attach", flag: Option::None, status: NoneStatus, note: "not implemented: `logs` is the same bytes and it does not need a signal proxy" },
    Row { verb: "pause", flag: Option::None, status: NoneStatus, note: "freezing a process group needs a cgroup this runtime does not grant" },
    Row { verb: "unpause", flag: Option::None, status: NoneStatus, note: "the counterpart of a verb podbox does not have" },
    Row { verb: "stats", flag: Option::None, status: NoneStatus, note: "resource accounting needs a cgroup this runtime does not grant" },
    Row { verb: "diff", flag: Option::None, status: NoneStatus, note: "podbox extracts into one directory and keeps no upper layer to difference against" },
    Row { verb: "port", flag: Option::None, status: NoneStatus, note: "podbox publishes no ports: the payload shares this machine's network namespace" },
    Row { verb: "rename", flag: Option::None, status: NoneStatus, note: "not implemented: `rm` and a fresh `create` under the other name is what podbox offers" },
    Row { verb: "update", flag: Option::None, status: NoneStatus, note: "there are no resource limits to update" },
    // Building and moving images.
    Row { verb: "build", flag: Option::None, status: NoneStatus, note: "building runs a payload per layer and commits the result; podbox can run one but cannot commit one" },
    Row { verb: "commit", flag: Option::None, status: NoneStatus, note: "podbox cannot restore ownership, so a committed layer would misrepresent what it holds (TODO/extract.md T-0302)" },
    Row { verb: "push", flag: Option::None, status: NoneStatus, note: "podbox reads registries and does not write to them" },
    Row { verb: "login", flag: Option::None, status: NoneStatus, note: "a credential never enters this tree (TODO/image.md T-0209)" },
    Row { verb: "logout", flag: Option::None, status: NoneStatus, note: "the counterpart of a verb podbox does not have" },
    Row { verb: "save", flag: Option::None, status: NoneStatus, note: "not implemented; the store holds OCI blobs and nothing exports them yet" },
    Row { verb: "load", flag: Option::None, status: NoneStatus, note: "not implemented; `pull` is the only way into the store" },
    Row { verb: "export", flag: Option::None, status: NoneStatus, note: "an exported rootfs would carry the ownership podbox could not apply, and would misrepresent what it holds" },
    Row { verb: "import", flag: Option::None, status: NoneStatus, note: "not implemented: podbox reads OCI images from a registry and nothing else builds a record" },
    Row { verb: "history", flag: Option::None, status: NoneStatus, note: "not implemented; `inspect` prints the record podbox holds" },
    Row { verb: "events", flag: Option::None, status: NoneStatus, note: "there is no daemon to emit events" },
    Row { verb: "search", flag: Option::None, status: NoneStatus, note: "not implemented; podbox resolves a reference and does not browse a registry" },
    Row { verb: "network", flag: Option::None, status: NoneStatus, note: "the payload shares this machine's network namespace, so there is nothing to create or attach" },
    Row { verb: "volume", flag: Option::None, status: NoneStatus, note: "podbox cannot mount, so a volume would be a copy pretending to be a mount" },
    Row { verb: "compose", flag: Option::None, status: NoneStatus, note: "not implemented; it needs the lifecycle and a network" },
    Row { verb: "swarm", flag: Option::None, status: NoneStatus, note: "not implemented, and out of the shape TOOL.md section 2.0 describes" },
    Row { verb: "builder", flag: Option::None, status: NoneStatus, note: "the counterpart of a verb podbox does not have" },
    Row { verb: "context", flag: Option::None, status: NoneStatus, note: "there is no daemon to point a context at" },
    Row { verb: "container", flag: Option::None, status: NoneStatus, note: "the sub-command group is not implemented; every verb it holds is a top-level verb here and is listed above" },
    // ------------------------------------------------------- run's own flags
    Row { verb: "run", flag: Some("--rm"), status: Native, note: "removes the extracted rootfs when the payload exits" },
    Row { verb: "run", flag: Some("-e, --env"), status: Native, note: "repeatable; a later one wins" },
    Row { verb: "run", flag: Some("-w, --workdir"), status: Native, note: "chdir inside the new root, after the chroot" },
    Row { verb: "run", flag: Some("--entrypoint"), status: Native, note: "as docker: replacing it also drops the image's Cmd" },
    Row { verb: "run", flag: Some("--platform"), status: Native, note: "a bare word is an architecture, as docker reads it" },
    Row { verb: "run", flag: Some("--pull"), status: Native, note: "never, missing (default) or always" },
    Row { verb: "run", flag: Some("--insecure-registry"), status: Native, note: "docker's flag and docker's meaning; every use is disclosed on stderr" },
    Row { verb: "run", flag: Some("--tls-verify"), status: Native, note: "podman's flag and podman's meaning; every use is disclosed on stderr" },
    Row { verb: "run", flag: Some("-t, --tty"), status: Degraded, note: "REFUSED BY NAME where /dev/ptmx is unusable, rather than running without a pty and letting the payload discover it (T-0503)" },
    Row { verb: "run", flag: Some("-i, --interactive"), status: Stub, note: "accepted and a no-op: podbox never detaches stdin, so it is already interactive when the caller's is" },
    Row { verb: "run", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "run", flag: Some("-d, --detach"), status: Native, note: "starts a detached launcher and prints the container id, returning when the payload is running" },
    Row { verb: "run", flag: Some("--name"), status: Native, note: "names the container. A name already in use is refused rather than making the second one unreachable" },
    Row { verb: "run", flag: Some("-v, --volume"), status: NoneStatus, note: "podbox cannot mount(2) on this runtime, so a volume would be a copy pretending to be a mount" },
    Row { verb: "run", flag: Some("-p, --publish"), status: NoneStatus, note: "the payload shares this machine's network namespace, so a published port is already this machine's port" },
    Row { verb: "run", flag: Some("--network"), status: NoneStatus, note: "there is no network namespace to select" },
    Row { verb: "run", flag: Some("-u, --user"), status: NoneStatus, note: "setuid to an id this machine does not map returns EINVAL; podbox runs as uid 0 and says so" },
    Row { verb: "run", flag: Some("--privileged"), status: NoneStatus, note: "podbox holds every capability bit already and can still do none of what they name" },
    Row { verb: "run", flag: Some("--cap-add, --cap-drop"), status: NoneStatus, note: "capabilities are not what is denied here; the filter is" },
    Row { verb: "run", flag: Some("-m, --memory"), status: NoneStatus, note: "resource limits need a cgroup this runtime does not grant" },
    Row { verb: "run", flag: Some("--cpus"), status: NoneStatus, note: "resource limits need a cgroup this runtime does not grant" },
    Row { verb: "run", flag: Some("--restart"), status: NoneStatus, note: "restarting needs a supervisor, which is M4" },
    Row { verb: "run", flag: Some("--hostname"), status: NoneStatus, note: "sethostname needs a UTS namespace this runtime does not grant" },
    // ⭐ M5 and TODO/cli.md T-0804. Four flags docker does not have, and each
    // is here for the same reason every other row is: a surface with no row is
    // a surface nobody documented.
    Row { verb: "run", flag: Some("--add-host"), status: Native, note: "name:ip, appended to the /etc/hosts the completion layer writes. Repeatable (T-0403)" },
    Row { verb: "run", flag: Some("--no-source-fixup"), status: Native, note: "podbox's own: leave the image's package sources exactly as extracted, http:// and all, and UNDO any rewrite an earlier run made (T-0411)" },
    Row { verb: "run", flag: Some("--no-host-cas"), status: Native, note: "podbox's own: do not append this machine's announced CA bundle ($SSL_CERT_FILE and friends) to the image's own trust store, even where the machine intercepts TLS (T-0407)" },
    Row { verb: "run", flag: Some("--no-steps"), status: Native, note: "podbox's own: run no command inside the rootfs before the payload. Two fixups cannot be made from outside the chroot -- pacman-key for an empty keyring and openssl rehash for a hash-indexed CA directory -- and this refuses both (T-0412)" },
    Row { verb: "run", flag: Some("--strict"), status: Native, note: "podbox's own: refuse rather than run where anything about this invocation is Degraded or Stub -- a flag, the selected rung, a completion fixup, or a step podbox would run inside the image (T-0804)" },
    // ------------------------------------------------------ exec's own flags
    Row { verb: "exec", flag: Some("-e, --env"), status: Native, note: "repeatable; a later one wins" },
    Row { verb: "exec", flag: Some("-w, --workdir"), status: Native, note: "chdir inside the new root, after the chroot" },
    Row { verb: "exec", flag: Some("--platform"), status: Native, note: "which platform of a multi-platform image to enter" },
    Row { verb: "exec", flag: Some("-t, --tty"), status: Degraded, note: "REFUSED BY NAME where /dev/ptmx is unusable (T-0503)" },
    Row { verb: "exec", flag: Some("-i, --interactive"), status: Stub, note: "accepted and a no-op, as in run" },
    Row { verb: "exec", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "exec", flag: Some("-d, --detach"), status: NoneStatus, note: "not implemented: an exec podbox does not watch has no exit code to report, which is the state T-0604 exists to avoid" },
    Row { verb: "exec", flag: Some("-u, --user"), status: NoneStatus, note: "podbox runs as uid 0 and cannot change to an unmapped id" },
    Row { verb: "exec", flag: Some("--add-host"), status: Native, note: "name:ip, appended to the /etc/hosts the completion layer writes (T-0403)" },
    Row { verb: "exec", flag: Some("--no-source-fixup"), status: Native, note: "as in run: leave the image's package sources as extracted, and undo an earlier rewrite (T-0411)" },
    Row { verb: "exec", flag: Some("--no-host-cas"), status: Native, note: "as in run: leave the image's own trust store alone (T-0407)" },
    Row { verb: "exec", flag: Some("--no-steps"), status: Native, note: "as in run: run no command inside the rootfs before the command asked for (T-0412)" },
    Row { verb: "exec", flag: Some("--strict"), status: Native, note: "as in run: refuse rather than re-enter where anything about this invocation is Degraded or Stub (T-0804)" },
    // ------------------------------------------------------ pull's own flags
    Row { verb: "pull", flag: Some("--platform"), status: Native, note: "a bare word is an architecture, as docker reads it" },
    Row { verb: "pull", flag: Some("--insecure-registry"), status: Native, note: "docker's flag and docker's meaning" },
    Row { verb: "pull", flag: Some("--tls-verify"), status: Native, note: "podman's flag and podman's meaning" },
    Row { verb: "pull", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "pull", flag: Some("-a, --all-tags"), status: NoneStatus, note: "podbox resolves one reference; fetching every tag of a repository is not implemented" },
    Row { verb: "pull", flag: Some("-q, --quiet"), status: NoneStatus, note: "not implemented; the transcript is the output of this verb" },
    // ---------------------------------------------------- images' own flags
    Row { verb: "images", flag: Some("--format"), status: Native, note: "{{.Field}} placeholders and literal text. No pipelines, no functions, and the `table` prefix is refused by name" },
    Row { verb: "images", flag: Some("-q, --quiet"), status: Native, note: "image IDs only, the same as --format '{{.ID}}'" },
    Row { verb: "images", flag: Some("--digests"), status: Native, note: "show the DIGEST column" },
    Row { verb: "images", flag: Some("--no-trunc"), status: Native, note: "print full IDs and digests" },
    Row { verb: "images", flag: Some("-a, --all"), status: Stub, note: "accepted for parity: podbox stores no intermediate images, so every image is already listed" },
    Row { verb: "images", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "images", flag: Some("-f, --filter"), status: NoneStatus, note: "not implemented: --format plus a caller's own filter is what podbox offers instead" },
    // ------------------------------------------------------- the rest's flags
    Row { verb: "rmi", flag: Some("-f, --force"), status: Stub, note: "accepted for parity. podbox never prompts, so there is no confirmation to suppress, and it refuses a held image whether or not this is given" },
    Row { verb: "rmi", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "inspect", flag: Some("-f, --format"), status: Native, note: "the same template shape as `images --format`" },
    Row { verb: "inspect", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "extract", flag: Some("--force"), status: Native, note: "extract again over an existing rootfs" },
    Row { verb: "extract", flag: Some("--platform"), status: Native, note: "which platform, where the store holds more than one" },
    Row { verb: "extract", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "probe", flag: Some("--json"), status: Native, note: "the findings as one JSON document on stdout" },
    Row { verb: "probe", flag: Some("--rows"), status: Native, note: "every probe as one row, in the format verification/probe emits" },
    Row { verb: "probe", flag: Some("--strict"), status: Native, note: "exit non-zero below the namespace rung, so a caller gates without parsing" },
    Row { verb: "probe", flag: Some("--cached"), status: Native, note: "serve $store/probe.json where its key still holds" },
    Row { verb: "probe", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    // ------------------------------------------------ the lifecycle's flags
    Row { verb: "ps", flag: Some("-a, --all"), status: Native, note: "list containers that are not running too" },
    Row { verb: "ps", flag: Some("-q, --quiet"), status: Native, note: "ids only" },
    Row { verb: "ps", flag: Some("--no-trunc"), status: Native, note: "print full container ids" },
    Row { verb: "ps", flag: Some("--format"), status: Native, note: "the same template shape as the other verbs" },
    Row { verb: "ps", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "ps", flag: Some("-f, --filter"), status: NoneStatus, note: "not implemented: --format plus a caller's own filter is what podbox offers instead" },
    Row { verb: "stop", flag: Some("-t, --time, --timeout"), status: Native, note: "seconds between SIGTERM and SIGKILL. Default 10, and a kill is reported on stderr" },
    Row { verb: "stop", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "kill", flag: Some("-s, --signal"), status: Native, note: "by name or number. ⛔ An unknown one is refused rather than defaulted: sending the wrong signal is not something a caller can notice" },
    Row { verb: "kill", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "rm", flag: Some("-f, --force"), status: Native, note: "kill a running container before removing it" },
    Row { verb: "rm", flag: Some("-v, --volumes"), status: Stub, note: "accepted for parity: podbox has no volumes, so there are none to remove" },
    Row { verb: "rm", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "logs", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "logs", flag: Some("-f, --follow"), status: NoneStatus, note: "not implemented: the log is a file in the store and `tail -f` on it is the same thing" },
    Row { verb: "wait", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "start", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "cp", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    Row { verb: "system", flag: Some("--format"), status: Native, note: "the same template shape as the other verbs, plus `json .Field` for a field that is a document" },
    Row { verb: "system", flag: Some("--dir"), status: Native, note: "install-names: where to put the symlinks. Default: the directory this binary is in" },
    Row { verb: "system", flag: Some("--force"), status: Native, note: "install-names: take the `docker` name even where a docker daemon answers, and replace a file that is not already a link to this binary" },
    Row { verb: "system", flag: Some("-h, --help"), status: Native, note: "prints this verb's usage and exits 0" },
    // ⭐ TODO/cli.md T-0803. The names podbox answers to are rows here for the
    // same reason every flag is: a surface with no row is a surface nobody
    // documented, and `names.rs` asserts these exist.
    Row { verb: "docker", flag: Option::None, status: Degraded, note: "podbox answers to the `docker` name on PATH through argv[0], and says so in the banner. It REFUSES to install that name where a working docker daemon is reachable, unless --force (T-0803)" },
    Row { verb: "podman", flag: Option::None, status: Degraded, note: "podbox answers to the `podman` name on PATH through argv[0], and says so in the banner (T-0803)" },
];

/// The verb's own row, if podbox names it at all.
pub fn verb(name: &str) -> Option<&'static Row> {
    TABLE.iter().find(|r| r.verb == name && r.flag.is_none())
}

/// The row for one flag of one verb, under any spelling the parser accepts.
///
/// ⛔ This is what makes the table binding rather than descriptive. A parser
/// asks here first, so a flag with no row cannot be quietly accepted and a row
/// with no parser arm cannot be quietly ignored.
/// Which verb's rows a verb's flags are listed under.
///
/// ⭐ `create` is served by `run`'s PARSER, so it takes `run`'s flag set. The
/// table holds ONE copy of those rows rather than two that can diverge, and this
/// is the single place that says where a verb's rows live. ⛔ Without it,
/// naming the caller's verb in the diagnostics would also have made `flag`
/// refuse every flag `create` accepts, which is the shape of hole
/// `docs/conventions/code.md` calls a second, quieter surface. T-0801.
pub fn rows_of(verb: &str) -> &str {
    match verb {
        "create" => "run",
        v => v,
    }
}

pub fn flag(verb: &str, name: &str) -> Option<&'static Row> {
    // ⚠ `--flag=value` is one argument to the shell and two things here. The
    // row is for the flag, so the value is cut off before the lookup.
    let name = name.split('=').next().unwrap_or(name);
    let rows = rows_of(verb);
    TABLE.iter().find(|r| r.verb == rows && r.names(name))
}

/// Decide whether a verb's parser may go on to handle `arg`, and say why not.
///
/// ⛔ **This is the guard T-0801 is for.** `Ok(())` means the table has a row
/// and the flag is not refused; `Err(code)` means the caller has already been
/// told, in one line, either that podbox has no such flag or that podbox has it
/// and will not honour it. A parser that decided this itself would be a second
/// table, and the one nobody reads is the one that drifts.
///
/// ⚠ A `Stub` row passes: it is accepted and does nothing, which is a
/// difference the table states rather than a refusal.
pub fn admit(verb: &str, arg: &str, usage: &str) -> Result<(), i32> {
    match flag(verb, arg) {
        Option::None => {
            // ⚠ Point at the verb whose ROWS were consulted, which is not always
            // the one the caller typed: `podbox create` is served by `run`'s
            // parser. Saying "this verb" would send a caller to a row set that
            // does not exist.
            let under = rows_of(verb);
            let where_ = if under == verb {
                "lists every flag this verb takes".to_string()
            } else {
                format!("lists every flag it takes under `{under}`, whose parser serves it")
            };
            eprintln!(
                "podbox {verb}: unknown option {arg:?}. It has no row in the parity \
                 table; `podbox system info` {where_}"
            );
            eprint!("{usage}");
            Err(podbox_image::error::EXIT_FLAG_ERROR)
        }
        Some(r) if r.status == Status::None => {
            // ⛔ Refused UP FRONT, with the reason, rather than accepted and
            // silently lost. `dockless`'s posture, and the whole point of the
            // table being consulted rather than described.
            eprintln!(
                "podbox {verb}: {} is in the parity table with status None: {}",
                r.flag.unwrap_or(arg),
                r.note
            );
            Err(podbox_image::error::EXIT_FLAG_ERROR)
        }
        Some(_) => Ok(()),
    }
}

/// The arm a verb's parser reaches when [`admit`] passed a flag and the parser
/// has no case for it.
///
/// ⛔ **That is a defect in podbox, not a caller mistake**, and it is reported
/// as one. ⚠ It is also the case a test has to be able to see, and since
/// T-0802 it can no longer be seen in the exit code: a flag error and a runtime
/// failure are both docker's 125, so the sentinel the parser tests used to
/// compare against became equal to the legitimate refusals it was distinguishing
/// itself from. Under `cfg(test)` this panics with the flag's own name, which is
/// a stronger assertion than the code comparison it replaces; in a shipped
/// binary it returns 125 and says what happened, because a runtime that panics
/// on its own argument surface is worse than one that refuses.
pub fn no_arm(verb: &str, flag: &str) -> i32 {
    let msg = format!(
        "podbox {verb}: {flag:?} is in the parity table and this parser has no arm \
         for it. That is a bug in podbox (TODO/cli.md T-0801)"
    );
    if cfg!(test) {
        panic!("{msg}");
    }
    eprintln!("{msg}");
    podbox_image::error::EXIT_RUNTIME_ERROR
}

/// The whole table as a JSON array, one object per row.
///
/// ⭐ T-0801's `Prove` reads exactly this: `length >= 60`, and every `status`
/// one of the four words.
pub fn json() -> String {
    let rows: Vec<serde_json::Value> = TABLE
        .iter()
        .map(|r| {
            serde_json::json!({
                "verb": r.verb,
                "flag": r.flag,
                "status": r.status.word(),
                "note": r.note,
            })
        })
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// The same table for a person, one row per line.
pub fn text() -> String {
    let mut out = String::new();
    let mut last = "";
    for r in TABLE {
        if r.verb != last {
            last = r.verb;
        }
        let name = match r.flag {
            Option::None => r.verb.to_string(),
            Some(f) => format!("{} {}", r.verb, f),
        };
        out.push_str(&format!("{:<34} {:<9} {}\n", name, r.status.word(), r.note));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ T-0801's `Prove`, as a unit test as well as a command: the table is
    /// the contract, so its size and its vocabulary are asserted here rather
    /// than only in an experiment somebody has to remember to run.
    #[test]
    fn the_table_is_at_least_sixty_rows_and_uses_only_the_four_statuses() {
        assert!(TABLE.len() >= 60, "the table has only {} rows", TABLE.len());
        for r in TABLE {
            assert!(
                ["Native", "Degraded", "Stub", "None"].contains(&r.status.word()),
                "{} has a fifth status",
                r.verb
            );
            assert!(!r.note.is_empty(), "{} has no note", r.verb);
        }
    }

    /// ⛔ A `None` row exists to be a sentence, so an empty reason makes the row
    /// worse than absent: the caller is told no and not why.
    #[test]
    fn every_unimplemented_row_says_why() {
        for r in TABLE.iter().filter(|r| r.status == Status::None) {
            assert!(
                r.note.len() > 20,
                "{} {:?} refuses without a reason a caller can act on",
                r.verb,
                r.flag
            );
        }
    }

    #[test]
    fn a_flag_is_found_under_every_spelling_the_parser_accepts() {
        assert_eq!(flag("run", "-e").map(|r| r.status), Some(Status::Native));
        assert_eq!(flag("run", "--env").map(|r| r.status), Some(Status::Native));
        // ⚠ `--env=A=1` is one argument and the row is for the flag.
        assert_eq!(
            flag("run", "--env=A=1").map(|r| r.status),
            Some(Status::Native)
        );
        // ⚠ `--name` was a `None` row until M4 and is `Native` now, so the
        // refusal case is taken from a row that is still one rather than from a
        // name that changed meaning under the test.
        assert_eq!(
            flag("run", "--name").map(|r| r.status),
            Some(Status::Native)
        );
        assert_eq!(
            flag("run", "--privileged").map(|r| r.status),
            Some(Status::None)
        );
        assert!(flag("run", "--nope").is_none());
        // ⚠ Per verb, not global: `--entrypoint` is run's and exec has no such
        // flag, so exec must not find run's row.
        assert!(flag("exec", "--entrypoint").is_none());
    }

    #[test]
    fn no_verb_carries_the_same_flag_twice() {
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for r in TABLE {
            let Some(spellings) = r.flag else { continue };
            for s in spellings.split(',') {
                let key = (r.verb, s.trim());
                assert!(!seen.contains(&key), "{} carries {} twice", key.0, key.1);
                seen.push(key);
            }
        }
    }

    #[test]
    fn every_flag_row_names_a_verb_the_table_also_has_a_row_for() {
        for r in TABLE.iter().filter(|r| r.flag.is_some()) {
            assert!(
                verb(r.verb).is_some(),
                "{} has flags and no verb row",
                r.verb
            );
        }
    }

    /// ⛔ `podbox create --no-steps` was ACCEPTED and acted on while
    /// `podbox system info` listed `--no-steps` for `run` and `exec` only, so
    /// the table under-described the binary and the refusal message sent a
    /// caller to a row set that does not exist. Both halves are asserted here:
    /// a verb that borrows another's rows resolves them, and it says whose.
    #[test]
    fn a_verb_served_by_another_parser_resolves_that_verbs_rows_and_says_so() {
        assert_eq!(rows_of("create"), "run");
        assert_eq!(rows_of("run"), "run", "a verb with its own rows keeps them");
        assert_eq!(rows_of("exec"), "exec");

        // The flag `create` was silently taking, now found through the table.
        assert!(
            flag("create", "--no-steps").is_some(),
            "create is served by run's parser, so run's rows must resolve for it"
        );
        assert!(
            flag("create", "--no-such-flag").is_none(),
            "borrowing rows must not admit a flag that has no row anywhere"
        );

        // ⚠ The row a reader is sent to has to name the borrowing, or the
        // caller looks for `create --no-steps` in the table and finds nothing.
        let note = verb("create").expect("create has a verb row").note;
        assert!(
            note.contains("run's flag set"),
            "the create row must say whose rows it takes, got {note:?}"
        );
    }

    #[test]
    fn the_json_carries_every_row_and_the_status_word() {
        let doc: serde_json::Value = serde_json::from_str(&json()).unwrap();
        let arr = doc.as_array().unwrap();
        assert_eq!(arr.len(), TABLE.len());
        assert!(arr.iter().all(|r| r["status"].is_string()));
        assert!(arr
            .iter()
            .any(|r| r["verb"] == "run" && r["flag"].is_null()));
    }
}
