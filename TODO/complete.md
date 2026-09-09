# complete

`crates/podbox-complete`. `TOOL.md` section 6.4, milestone M5.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

The layer that makes it work without the user learning anything. Before any
payload runs, prepare the rootfs from the image plus distro detection,
idempotently, logging each fixup.

⭐ **Each row below was found because it broke something.** The ten-distro
sweep in M5's acceptance is not an arbitrary set: every member is there because
one of these fixups was missing.

---

### T-0401 Device shims as regular files

Source:      `TOOL.md` section 6.4
Category:    complete
Priority:    P0
Effort:      S
Status:      done

Problem:     `mknod` is denied, so an extracted rootfs has no `/dev/null`. The
             first shell redirection into it then **creates a growing regular
             file**, silently, and the container fills its own filesystem.
Premise:     Read, and independently observed in the corpus: the `ruri` capture
             in `references/Azathothas__container-research/tree/verification/README.md:126`
             records the same trap, `/dev/null` absent and shell-created.
Approach:    Create `/dev/null` as an empty regular file: writes succeed and
             reads give EOF, which is the observable behaviour most payloads
             need. `/dev/zero` and `/dev/urandom` as pre-filled regular files
             where a file suffices. `getrandom(2)` already covers TLS and
             crypto, so `/dev/urandom` is a compatibility shim rather than an
             entropy source and the banner says so.
             ⛔ A shimmed device is reported as shimmed. `inspect` carries the
             list.
Approach note: T-0708's `mknod` interception makes the same substitution from
             inside the payload. This entry covers the payloads the interposer
             cannot reach, which is every static and every Go one.
Decision:    A regular file, not a fifo. A fifo blocks a writer with no reader,
             which turns a redirection into a hang, and a hang is the failure
             mode [RULES.md](RULES.md) section 8 exists to prevent.
Prove:       `podbox run --rm alpine:latest sh -c 'echo x >/dev/null && test ! -s /dev/null'`


**Done 2026-09-09.** `crates/podbox-complete/src/devices.rs`, and the shape
changed while it was being built.

⭐ **`mknod` IS TRIED FIRST and the shim is the fallback.** The entry assumed
the wall; the code asks the kernel. Probed by calling it rather than inferred
from a capability bit, because this runtime hands a process every capability and
denies the operation anyway. On this host `mknodat` SUCCEEDS, so
`podbox run --rm alpine ls -l /dev` reads

```
crw-rw-rw-  1 root root 1, 3 null
crw-rw-rw-  1 root root 1, 5 zero
crw-rw-rw-  1 root root 1, 8 random
crw-rw-rw-  1 root root 1, 9 urandom
```

and the four rows are `dev-node`, not `dev-shim`, and are **not** degraded.
⛔ A shim reported on a machine that did not need one is a degradation podbox
invented, which is the same class of wrong as hiding one. Where `mknod` is
refused the errno it refused with is in the banner beside each shim.

⚠ **`/dev/random` is a fourth path this entry did not name.** Its shape and its
failure are identical to `/dev/urandom`'s, so it is shimmed with it.

⚠ The shims are 1 MiB each and `statvfs` runs before the first of them
([RULES.md](RULES.md) section 8 and [T-0806](cli.md)), because three megabytes
into a small tmpfs is exactly the write that fills it.

⭐ **The repair is the point, not the creation.** `/dev/null` is truncated on
**every** run: the rootfs is content-addressed and shared between containers
([T-0204](image.md)), so a payload that redirected into a shimmed `/dev/null`
leaves a large regular file that the next container would inherit. The fixup
reports how large it had grown.

⛔ **The test drives BOTH arms.** `what_each_row_claims_is_what_is_on_disk`
holds on every machine whichever arm it takes: a row that says `dev-node` is a
character device on disk and is not marked degraded, and a row that says
`dev-shim` is a regular file and is. The shim arm's own tests call `shim_for`
directly, because a test that called `run` here would measure the host and
report the host's answer as the code's.

Prove, run 2026-09-09:

```
$ podbox run --rm public.ecr.aws/docker/library/alpine:3.20 \
    sh -c 'echo x >/dev/null && test ! -s /dev/null'
  exit 0
```

---

### T-0402 Always install the host's `/etc/resolv.conf`

Source:      `TOOL.md` section 6.4, section 8
Category:    complete
Priority:    P0
Effort:      S
Status:      done

Problem:     Two separate failures, one fixup. An image may ship an empty
             placeholder, and an image may bake an unreachable build-host
             resolver: `192.168.122.1`, a libvirt NAT gateway, in the rocky
             family. The second failure reads as `Couldn't resolve host
             mirrors.*` with a `resolv.conf` that looks fine.
Premise:     Read. It is also docker's own semantics, which is the stronger
             argument: an agent that knows docker expects the host's resolver.
Approach:    Overwrite `/etc/resolv.conf` in the rootfs with the host's on every
             `run` and `exec`, before the payload starts. Log the fixup.
             `--dns` and friends are stubs and the banner says so, because the
             completion layer owns the resolver (T-0801).
Decision:    Always, not "if empty". An image with a wrong resolver is
             indistinguishable from one with a right one without resolving
             something, and resolving something is a network round trip on every
             container start.
Prove:       `podbox run --rm docker.io/rockylinux/rockylinux:9 cat /etc/resolv.conf | diff -q - /etc/resolv.conf`


**Done 2026-09-09.** `crates/podbox-complete/src/net.rs`, and unconditionally,
as the Decision says: an image with a wrong resolver is indistinguishable from
one with a right one without resolving something.

⚠ **The write is NOT a degradation and the skip is.** What the payload reads is
this machine's resolver byte for byte, which is what docker's payload reads too;
the difference -- podbox writes the file where docker mounts one over it -- is a
property of having no mount namespace and is stated on the banner's rung line.
Where this machine has no readable resolver, podbox leaves the image's alone,
says so, and marks THAT degraded, because the payload is then left with a
resolver that may name a host it cannot reach.

⛔ Where `/etc/resolv.conf` is a symlink podbox **replaces the link** rather
than writing through it, and the fixup line names the target it replaced.
That is [T-0405](#t-0405-etcmtab-is-a-symlink-and-writing-through-it-escapes-the-rootfs)'s
mechanism, applied here because this file has the identical shape.

⚠ **The `Prove` above cannot run as written.** It pipes `podbox run` into
`diff`, and the payload's stdout is not the only thing on it: podbox's banner is
on stderr, but `podbox run` also prints the pull transcript. What was run:

```
$ podbox run --rm public.ecr.aws/debian/debian:bookworm-slim \
    cat /etc/resolv.conf 2>/dev/null | diff -q - /etc/resolv.conf
  (no output: identical)
```

---

### T-0403 `/etc/hosts`

Source:      `TOOL.md` section 6.4, section 6.8
Category:    complete
Priority:    P2
Effort:      S
Status:      done

Problem:     A payload that resolves its own container name, or `localhost`,
             gets nothing.
Premise:     Read.
Approach:    Write `127.0.0.1 localhost <container-name>` plus `::1 localhost`,
             and append every `--add-host` entry. `--add-host` is Native in the
             parity table and this is where it is honoured.
Decision:    Write the file rather than intercept the resolver. The interposer
             does not reach static payloads, and `/etc/hosts` is read by the
             libc of whatever is running.
Prove:       `podbox run --rm --name hostprobe --add-host foo:10.0.0.1 alpine:latest sh -c 'getent hosts foo && getent hosts hostprobe'`


**Done 2026-09-09.** `crates/podbox-complete/src/net.rs`, `hosts`.
`127.0.0.1 localhost <container-name>`, `::1 localhost ip6-localhost
ip6-loopback`, and one line per `--add-host`.

⛔ **`--add-host` splits on the FIRST colon, not the last.** The value may be
IPv6: `--add-host six:2001:db8::1` is the name `six` at `2001:db8::1`, and a
split on the last colon loses the address. There is a test for exactly that.

⚠ A malformed value is refused **by name** with the flag's own spelling, rather
than dropped.

---

### T-0404 Synthesize `/etc/passwd` and `/etc/group`, and let the payload's writes persist

Source:      `TOOL.md` section 6.4, section 8; `references/indigo-dc__udocker` tracker #141
Category:    complete
Priority:    P0
Effort:      M
Status:      done

Problem:     The absence of `/etc/passwd` kills tooling before any syscall wall
             is reached, with `getpwuid(): uid not found: 0`. And a package that
             creates its own service user during install fails, which stops the
             install rather than the user creation.
Premise:     ⭐ **Read at file and line in two projects, and the maintainer's
             ruling is in a tracker.**
             `references/garywill__treesandbox/tree/src/initializing.py:69` calls
             `pwd.getpwuid(uid).pw_name` unguarded, and
             `references/garywill__treesandbox/tree/src/initializing.py:72` reads
             `/etc/hostname` unguarded, both at initialization, before any
             namespace call. Both files are absent on the target, which is why
             that tool dies before reaching the wall it was built for.
             `references/indigo-dc__udocker/tree/udocker/helper/hostinfo.py:19-24`
             returns `""` from `username()` on `KeyError`, and
             `references/indigo-dc__udocker/tree/udocker/engine/base.py:422-427`
             consumes the empty name and reports
             `Error: invalid syntax for user`. udocker tracker #258 is a user
             hitting exactly that.
             ⭐ The fix is the maintainer's, in `references/indigo-dc__udocker`
             tracker #141, comment of 2020-01-08: `run --containerauth` made
             passwd and group changes **persist in the container**, and the
             reporter confirmed that is what fixed package installation. The
             same thread carries the failure mode, from 2018-08-31: "packages
             that deploy their own service users during install/config and throw
             out errors when they try to do so, stopping the install process."
⛔ **On a glibc rootfs this entry does nothing on its own.** Whether
             `/etc/passwd` is read at all is decided by `/etc/nsswitch.conf`,
             measured in T-0410. The two land together.
Approach:    Synthesize both files in the rootfs if absent, with at least `root`
             and `nobody`, and **leave them writable and persistent** so
             `useradd` inside the container works and survives. Never bind the
             host's.
             ⚠ The ids in the synthesized file are the image's convention, and
             only `0:0` is real (T-0704). `inspect` says so.
Decision:    Synthesize into the rootfs rather than interpose `getpwuid`.
             Interposition does not reach a static or Go payload, and this is
             the fixup that decides whether M5 passes.
Prove:       `podbox run --rm alpine:latest python3 -c 'import pwd; print(pwd.getpwuid(0).pw_name)' | grep -qx root && podbox run --rm docker.io/library/debian:bookworm-slim sh -c 'useradd -r svc && grep -q ^svc: /etc/passwd'`


**Done 2026-09-09.** `crates/podbox-complete/src/identity.rs`, and it lands
with [T-0410](#t-0410-supply-etcnsswitchconf-or-the-supplied-etcpasswd-is-a-no-op)
as the entry says it must.

⛔ **Synthesized only where the file is ABSENT.** An image's own `/etc/passwd`
is the image's, and tracker #141's fix is that the payload's later writes to it
PERSIST. What podbox does to a file the image ships is narrower and is the half
that actually unblocks an install: where the mode has no owner write bit, podbox
sets it and **changes not one byte of the content**, because a `/etc/passwd`
that `useradd` cannot write is where a package that deploys a service user
stops.

⚠ Finding it took a defect in `Root::write`: comparing only the CONTENT reported
that mode change as `Unchanged` and left the file read-only. The comparison is
content **and** mode now.

⚠ The synthesized file carries `root` and `nobody`, and the fixup line says that
only `0:0` is a real id on this machine.

⚠ **The `Prove` above cannot run as written and the correction is here rather
than in place of it.** `alpine:latest` ships no `python3`, so its first half
tests whether `apk` has run rather than whether `getpwuid` answers. What was run
instead, on all ten rows of `experiments/240-distro-sweep.sh`, is `getent passwd
0`, which goes through NSS on glibc and is therefore also T-0410's question:

```
$ podbox run --rm <each of the ten rows> sh -c 'getent passwd 0 | cut -d: -f1'
  root, on every row that has getent
```

---

### T-0405 `/etc/mtab` is a symlink, and writing through it escapes the rootfs

Source:      `TOOL.md` section 6.4; `paper_final.md` section 8.3 and F10
Category:    complete
Priority:    P1
Effort:      S
Status:      done

Problem:     `printf ... > "$R/etc/mtab"` follows the link. If the target is
             absolute, the write lands **outside the rootfs**, on the host.
Premise:     Read. F10 also establishes that `/etc/mtab` is not required by
             pacman in general: it is read only when `CheckSpace` is enabled,
             which Arch's shipped `pacman.conf` leaves commented out. So this
             fixup applies where the package set reads it and nowhere else.
             The extraction side of the same fact is T-0305: an absolute symlink
             target is legitimate and is rebased, not rejected.
Approach:    Remove the link before writing, then write a regular file. Generate
             its contents from the real topology; never ship another machine's
             device numbers.
             ⛔ Every write the completion layer makes into the rootfs goes
             through the same `O_NOFOLLOW` open on the final component, not only
             this one. `/etc/resolv.conf` is a symlink to
             `../run/systemd/resolve/stub-resolv.conf` in several images and has
             the identical shape.
Decision:    `O_NOFOLLOW` plus unlink-then-create for every completion write,
             rather than a special case for `mtab`. A special case covers the
             one instance somebody found.
Prove:       `./experiments/85-completion-symlink-escape.sh` exits 0


**Done 2026-09-09.** `crates/podbox-complete/src/write.rs` is the mechanism and
the Decision's shape: **one door**, and no special case for `mtab`.

Every completion write resolves each component through
`podbox_extract::safety::open_parent` -- `openat2(RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS)` where the kernel has it, an `O_NOFOLLOW` walk where it
does not -- opens the final component `O_NOFOLLOW`, and **unlinks before it
creates**. ⛔ Unlink-then-create rather than truncate: truncating an existing
name follows a symlink exactly as an ordinary open does, so "the file is already
there" is the case that escapes.

⚠ `/etc/mtab` itself is written only where it is **broken**: absent, or a
symlink. An image that ships a regular file keeps it, because `paper_final.md`
F10 establishes the file is read only when `CheckSpace` is enabled and Arch
ships that commented out. Its content is generated from this machine's own
mount table, never shipped: another machine's device numbers in a mount table is
a fabricated number.

Prove, run 2026-09-09 -- `experiments/85-completion-symlink-escape.sh`, which
extracts a real image, plants **five** symlinks out of the rootfs where the
completion layer writes, runs the shipped binary, and reads the canaries off the
filesystem rather than off an exit code:

```
$ ./experiments/85-completion-symlink-escape.sh
  A /etc/mtab           : intact
  B /etc/resolv.conf    : intact
  C /etc/hosts          : intact
  E /etc/passwd         : intact
  D through a directory : intact
  5 doors, 0 escapes, and every write landed inside the rootfs.
  exit 0
```

---

### T-0406 pacman: `DownloadUser` and the keyring

Source:      `TOOL.md` section 6.4, section 8; `paper_final.md` section 8.3 and F9
Category:    complete
Priority:    P1
Effort:      S
Status:      done

Problem:     `pacman -Sy` fails with `failed to chown temporary download
             directory ...: Invalid argument`. `DownloadUser = alpm` names a uid
             this runtime cannot map, and the errno is the section 9.1 wall again.
Premise:     Read, from a measured session: an Arch rootfs entered by plain
             `chroot` installs **signed** packages from live repositories under
             this runtime, after two fixups and not before.
Approach:    Comment out `DownloadUser` in `/etc/pacman.conf`. Initialize the
             keyring with `pacman-key --init` and `pacman-key --populate` if it
             is not already initialized. Both are idempotent and both are logged.
             ⚠ Do not enable `CheckSpace`. It is what makes `/etc/mtab` load
             bearing (T-0405), and Arch ships it commented out.
Decision:    Edit `pacman.conf` rather than create the `alpm` user. Creating it
             does not help: the id still has no mapping, so the `chown` still
             returns `EINVAL`.
Prove:       `podbox run --rm docker.io/library/archlinux:latest sh -c 'pacman -Sy --noconfirm gcc >/dev/null && gcc --version'`


**Done 2026-09-09.** `crates/podbox-complete/src/pkg.rs`, and the two halves
landed differently because only one of them can be done from outside the chroot.

⭐ **`DownloadUser` is commented out**, matched on the line's FIRST WORD rather
than as a substring, so `#DownloadUser` is not counted or commented twice and
`DownloadUserAgent` is left alone. The comment podbox inserts names this entry,
so a reader inside the container can see who did it. ⛔ Creating the `alpm` user
is not the fix and is not attempted: the id still has no mapping, so the `chown`
still returns `EINVAL`.

⚠ `CheckSpace` is deliberately **not** enabled, which is this entry's own
warning: it is what makes `/etc/mtab` load-bearing
([T-0405](#t-0405-etcmtab-is-a-symlink-and-writing-through-it-escapes-the-rootfs)),
and Arch ships it commented out.

⛔ **The keyring half is a READING and not an edit, and that is a limit rather
than a choice.** `pacman-key --init` and `--populate` run `gpg` INSIDE the
rootfs, and the completion layer runs on the host before the chroot. podbox
reports whether `/etc/pacman.d/gnupg/trustdb.gpg` is there and, where it is not,
says which two commands to run and marks the row degraded. ⚠ Measured on the
pinned `ghcr.io/pkgforge-dev/archlinux` image: it ships an initialized keyring,
so the row reads `Unchanged` there and the diagnostic is for a rootfs that does
not.

Prove, run 2026-09-09 through `experiments/240-distro-sweep.sh`, against
`ghcr.io/pkgforge-dev/archlinux:latest` rather than the Docker Hub tag the
`Prove` names -- the sweep touches no quota-bearing registry:

```
  archlinux  glibc  pacman  install 0  build 0  ran 42
```

---

### T-0407 apt: the sandbox user, https sources and the CA bundle

Source:      `TOOL.md` section 6.4, section 8
Category:    complete
Priority:    P1
Effort:      S
Status:      done

Problem:     Two failures. `_apt` is an unmapped user, and apt drops privileges
             to it for downloads. And tcp/80 is broken, so a default
             `http://deb.debian.org` source produces `Method http has died` or
             `no Release file`, which reads as a mirror problem.
Premise:     Read. The protocol half follows from T-0201's premise.
Approach:    Set `APT::Sandbox::User=root` in a drop-in under
             `/etc/apt/apt.conf.d/`. Rewrite the sources to `https://` in place,
             preserving the mirror. Install the host CA bundle where the image
             has none.
             ⛔ Never disable certificate verification to make https work. If
             the bundle cannot be found, that is a named refusal.
Decision:    A drop-in file rather than editing `apt.conf`, so the fixup is
             visible, removable and idempotent, and an image that already sets
             the key is not clobbered.
Prove:       `podbox run --rm docker.io/library/debian:bookworm-slim sh -c 'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq gcc >/dev/null && gcc --version'`


**Done 2026-09-09.** `crates/podbox-complete/src/pkg.rs`, and the entry named
two failures where the measurement found **three**.

1. **`APT::Sandbox::User "root"`**, in a drop-in at
   `/etc/apt/apt.conf.d/99podbox`, as the Decision says: visible, removable,
   idempotent, and it does not clobber an image that already sets the key.
2. **The https rewrite is [T-0411](#t-0411-a-payload-whose-package-sources-are-http-on-a-runtime-where-tcp80-hangs)'s**
   and is applied to every distribution rather than to apt alone.
3. ⭐ **THE THIRD, AND IT IS WHY M5 COULD NOT BE MEASURED AT ALL AT FIRST.**
   Measured on 2026-09-09: `apk add gcc` inside `alpine:3.20` failed with
   `certificate verify failed`, and it failed **identically under docker**, so
   the cause is the machine and not either runtime. This host's egress is
   intercepted and it announces its own root in `$SSL_CERT_FILE`,
   `$CURL_CA_BUNDLE` and `$REQUESTS_CA_BUNDLE`. `curl`, `python`, `node` and
   `cargo` all read those; a container that does not is the anomaly.

   So the fixup has two arms, and the discriminator is the **announcement**:

   - the image ships **no** bundle: install this machine's, which is what this
     entry already said;
   - the image ships one **and the machine announced its own in one of those
     three variables**: ⛔ **APPEND** it, keeping the image's own roots, under a
     marker line so the block is findable and the append is idempotent.

   ⛔ A bundle found only at a default path is that distribution's own trust
   store and is **no announcement**: nothing is appended on that basis, because
   adding a root to somebody else's image would be a trust change nobody asked
   for. `--no-host-cas` refuses the announced case too, and the fixup then says
   what the caller gave up.

   ⚠ The append is marked **degraded**: this machine's trust is now the
   container's, and `--strict` refuses a run that needed it.

⛔ **Verification is never disabled to make https work**, which is this entry's
own rule and is unchanged. Where no bundle can be found at all, that is a named
refusal and the scheme rewrite does not happen.

Prove, run 2026-09-09 through `experiments/240-distro-sweep.sh`:

```
$ podbox run --rm public.ecr.aws/debian/debian:bookworm-slim sh -c \
    'apt-get update -qq && apt-get install -y -qq gcc && gcc --version'
  gcc (Debian 12.2.0-14+deb12u1) 12.2.0
  exit 0
```

---

### T-0408 zypper: fix the RIS index, not `repos.d`

Source:      `TOOL.md` section 6.4, section 8
Category:    complete
Priority:    P2
Effort:      S
Status:      done

Problem:     A naive edit to `/etc/zypp/repos.d/` reverts. `refresh-services`
             regenerates that directory from the RIS index and overwrites
             whatever was written there.
Premise:     Read, from a measured distro sweep.
Approach:    Edit the service definitions under
             `/usr/share/zypp/local/service/` first, then let the refresh
             regenerate `repos.d` from the corrected index.
Decision:    Edit the source of the generated file. The alternative, disabling
             the refresh, changes what the payload's package manager does in a
             way the payload can observe and did not ask for.
Prove:       `podbox run --rm registry.opensuse.org/opensuse/leap:latest sh -c 'zypper -n refresh && zypper -n install gcc >/dev/null && gcc --version'`

---

### T-0409 Ownership failures from `dpkg`, `rpm` and `xbps` are warnings

Source:      `TOOL.md` section 6.4
Category:    complete
Priority:    P2
Effort:      S
Status:      done

Problem:     A package manager that cannot `chown` a file it just unpacked
             prints an error. Treating that as fatal fails an install that
             otherwise completed.
Premise:     Read: ten distributions were driven end to end this way and none
             hit a fatal ownership wall at install time, **once extraction was
             ownership-neutral**. The premise depends on T-0302 having landed;
             without it the same failures are fatal for a different reason.
Approach:    Where the interposer is active (T-0704) the `chown` succeeds and is
             recorded, and there is nothing to downgrade. Where it is not, do
             not translate the package manager's exit status: report the
             warnings in the banner's fixup log and let the payload's own exit
             code stand.
             ⛔ podbox never rewrites a payload's exit code. The exit code is the
             payload's answer, and a runtime that improves it is lying in the one
             field an automated caller reads first.
Decision:    Report, do not suppress and do not translate. Suppressing hides a
             real failure of the same shape; translating breaks `docker run`
             parity on exit codes, which is T-0802.
Prove:       `podbox run --rm docker.io/voidlinux/voidlinux-musl:latest sh -c 'xbps-install -Sy gcc >/dev/null 2>&1; gcc --version'`


**Done 2026-09-09.** ⛔ **A rule rather than a code path, and it is enforced in
two places that cannot disagree.**

1. `podbox_complete` never touches an exit code at all: `complete()` returns a
   report, and the caller prints it. A fixup that failed is a `Failed` row and a
   banner line, and the payload's status is whatever the payload said.
2. [T-0802](cli.md) is the other half, and it is measured rather than reviewed:
   `experiments/330-exit-codes.sh` clause 5 runs the same payload twice, once
   with the fixups and once with `--no-source-fixup`, and asserts **both**
   report the payload's own 7.

⚠ Where the interposer is active ([T-0704](interpose.md)) the `chown` succeeds
and there is nothing to downgrade. Where it is not, the package manager's
warnings are the package manager's and podbox neither suppresses nor translates
them.

⭐ The premise held: on all ten rows of `experiments/240-distro-sweep.sh` no
install hit a fatal ownership wall, with extraction ownership-neutral
([T-0302](extract.md)).


**Done 2026-09-09.** `crates/podbox-complete/src/pkg.rs`, and the reading is
that this rootfs regenerates or it does not, rather than that it always does.

The RIS index is looked for at `/usr/share/zypp/local/service` and
`/etc/zypp/services.d`. Where one is present, the scheme fixup
([T-0411](#t-0411-a-payload-whose-package-sources-are-http-on-a-runtime-where-tcp80-hangs))
is applied **there as well as** to `/etc/zypp/repos.d`, which is the Decision:
edit the source of the generated file, or `refresh-services` regenerates
`repos.d` from the uncorrected index and reverts it. Where none is present, the
fixup says so and editing `repos.d` in place holds.

⚠ The index is a tree rather than a directory of files -- one directory per
service, each holding a `repo/` -- and the walk is exactly two levels deep. A
recursive walk of somebody else's image looking for URLs to edit is a much
larger claim than this fixup makes.

---

### T-0410 Supply `/etc/nsswitch.conf`, or the supplied `/etc/passwd` is a no-op

Source:      `experiments/results/nsswitch-contract.txt`
Category:    complete
Priority:    P0
Effort:      S
Status:      done

Problem:     T-0404 synthesizes `/etc/passwd` and `/etc/group`. On a glibc
             rootfs, whether anything reads them is decided by a different file.
             glibc resolves a user through NSS, and `/etc/nsswitch.conf` names
             which service answers; only `files` reads `/etc/passwd`.
Premise:     ⭐ **Measured, with a user name no distribution ships, so a hit
             cannot come from the image's own `/etc/passwd`.**
             `experiments/results/nsswitch-contract.txt` check A, the same
             supplied `/etc/passwd` and the same static probe against a pinned
             `ubuntu:20.04`:

             ```
             nsswitch=files  -> FOUND
             nsswitch=other  -> NOTFOUND
             ```

             ⛔ Supplying `/etc/passwd` alone is a shim that silently does
             nothing on any rootfs whose `nsswitch.conf` names another service,
             and the failure surfaces as a user that does not exist rather than
             as a file that was not read.
             ⚠ **This is a glibc-rootfs requirement.** Check B reads three
             pinned images: `ubuntu:20.04` and `debian:12` both name `files`,
             and `alpine:3.22` ships no `nsswitch.conf` at all, because musl has
             no NSS plugin system and reads `/etc/passwd` directly.
             ⚠ A statically linked payload does not escape this. The probe in
             check A is `-static` and still answered `NOTFOUND`: static linking
             removes the `PT_INTERP`, not the NSS dispatcher.
Approach:    Probe the rootfs rather than assuming, in this order:
             1. no `/etc/nsswitch.conf` and no glibc in the rootfs: musl, do
                nothing, `/etc/passwd` is read directly;
             2. `/etc/nsswitch.conf` present and `passwd` names `files` first:
                do nothing, it already works;
             3. `/etc/nsswitch.conf` present and `passwd` does not name `files`:
                ⚠ do not overwrite it. Prepend `files` to the `passwd` and
                `group` lines, keep the rest, and report the edit on the one
                line T-0108's banner allows;
             4. glibc in the rootfs and no `/etc/nsswitch.conf`: write a minimal
                one naming `files`.
             Every write follows T-0404's rule that the payload's own later
             writes persist.
Decision:    Edit rather than replace. A rootfs whose `nsswitch.conf` names
             `sss` or `systemd` may be doing so for a reason podbox cannot see,
             and prepending `files` restores the supplied file without removing
             what was there.
Prove:       `./experiments/90-nsswitch-contract.sh` exits 0 and `podbox run --rm debian:12 sh -c 'id podboxsupplied'`


**Done 2026-09-09.** `crates/podbox-complete/src/identity.rs`, `nsswitch`, and
all four cases the Approach lists are arms of one `match`:

1. no `nsswitch.conf` and no glibc: musl, and nothing is written;
2. `passwd` already names `files` **first**: nothing is written;
3. it does not: ⛔ `files` is **prepended** and every other service on the line
   survives, comment included;
4. glibc and no `nsswitch.conf`: a minimal one naming `files`.

⭐ **"Names `files`" is not enough; it has to be FIRST**, and the entry's own
rule 3 said "does not name files". `passwd: sss files` asks the directory
service before the file podbox supplied, and a directory that answers for a
different user is a wrong answer rather than a missing one. Where `files`
appears later it is MOVED to the front rather than duplicated: glibc tolerates
the duplicate and a line carrying it twice is one no reader can tell podbox from
the image on.

⭐ **The unit test is the measurement.** `experiments/125-across-distributions.sh`
read **6 distinct `passwd` shapes** across eleven pinned images, and
`every_measured_passwd_shape_ends_up_naming_files_first` drives those six
strings -- the measured ones, not examples -- through the editor and asserts
`files` is first and appears once.

⚠ **A reading this entry did not have: `alpine:3.20` DOES ship
`/etc/nsswitch.conf`**, with a comment saying musl does not support NSS and that
some third-party DNS implementations read it. The entry's check B read
`alpine:3.22` and found none. Both are true and neither changes the fixup: case
2 leaves the file alone either way, and musl reads `/etc/passwd` directly.

Prove, run 2026-09-09:

```
$ ./experiments/90-nsswitch-contract.sh
  exit 0
$ podbox run --rm <each of the ten rows of 240-distro-sweep.sh> \
    sh -c 'getent passwd 0 | cut -d: -f1'
  root, on every row that ships getent
```

---

### T-0411 A payload whose package sources are `http://`, on a runtime where tcp/80 hangs

Source:      Raised by the operator on 2026-09-09, beside [T-0213](image.md)
Category:    complete
Priority:    P1
Effort:      M
Status:      done

Problem:     ⛔ **The registry half is fixed and the payload half is not.**
             [T-0213](image.md) settles how **podbox** reaches a registry over
             plain HTTP. It says nothing about the package sources **inside an
             image**, and many base images ship `http://` ones: debian's
             `deb http://deb.debian.org`, alpine's `http://dl-cdn.alpinelinux.org`,
             archlinux's mirrorlist. ⚠ On the runtimes podbox targets tcp/80
             egress is black-holed, so `apt-get update` inside a container does
             not fail, it **hangs** until its own timeout, and an agent reads
             that as podbox being slow.
Premise:     ⭐ **Read, and it is the same reading T-0201 rests on.**
             `TOOL.md` section 6.4 describes the M5 fixups as **protocol**
             fixups rather than mirror fixups, which is exactly this: the
             distribution's own mirrors are reachable, and the scheme is not.
             ⚠ **NOT MEASURED HERE YET**, and it cannot be until M3: measuring
             it means running `apt-get update` inside a container, and there is
             no `podbox run`. The reading that exists is the host's, and the
             host is not the target.
Approach:    In the completion layer, beside the other section 6.4 fixups.
             1. **Rewrite the scheme, never the host.** `http://deb.debian.org`
                becomes `https://deb.debian.org`; the mirror the image chose is
                the mirror podbox uses. ⛔ Substituting a mirror is a supply
                chain change made on the payload's behalf, and it is not
                podbox's to make.
             2. ⛔ **Say so, per file, in the banner.** podbox edited a file the
                payload will read, and [T-0804](cli.md)'s honesty rules have no
                exception for a helpful edit.
             3. **`--no-source-fixup` turns it off**, for a payload whose mirror
                genuinely has no HTTPS, and then the hang is the caller's
                choice and is named as such.
             4. ⚠ **Verify the mirror speaks HTTPS before rewriting**, once, with
                a bounded timeout, and leave the file alone when it does not.
                Rewriting a source that then 404s is worse than the hang,
                because the failure no longer names the cause.
Decision:    Rewrite in the extracted rootfs at `run` time rather than at
             `extract` time. ⚠ The rootfs is content-addressed and shared
             between containers ([T-0204](image.md)); editing it at extraction
             would make one container's fixup another's, and `--no-source-fixup`
             would then depend on which container extracted first.
             ⛔ **This is not the same decision as [T-0213](image.md)'s.** There
             the caller names a registry and podbox obeys. Here podbox is
             changing a file inside somebody else's image, so the default is the
             conservative one and the disclosure is per file.
Prove:       `podbox run --rm debian:latest sh -c 'apt-get update' ` completes rather than hanging, the banner names each file rewritten, and `--no-source-fixup` leaves every source file byte-identical to the extracted tree

**Done 2026-09-09.** `crates/podbox-complete/src/sources.rs`, and all four
rules of the Approach are enforced:

1. ⛔ **the scheme and nothing else.** `http://deb.debian.org` becomes
   `https://deb.debian.org`; the host, the port, the path, the suite and every
   option around it are the image's. Substituting a mirror is a supply chain
   change made on the payload's behalf and it is not podbox's to make;
2. **per file, on the banner**, with the hosts it rewrote named;
3. `--no-source-fixup` turns it off **and undoes** a rewrite an earlier run
   made. ⭐ That second half is not decoration: the rootfs is shared between
   containers ([T-0204](image.md)), so an "off" that only meant "not this time"
   would depend on which container started first. The image's own bytes are kept
   beside the rootfs in `.complete-orig/`, never inside it, for the same reason
   the ownership sidecar is outside;
4. **the mirror is asked first**, once per host, with a two-second connect and a
   two-second read. Any HTTP status is a yes, including 404: the question is
   whether the host terminates TLS on 443. A host that does not answer keeps its
   `http://`, and the banner says which host and why.

⚠ **`localhost` and a literal address are never rewritten.** A mirror at
`http://127.0.0.1:8080` is the caller's own cache; tcp/80 is not in it and
`https://` there is a refused connection rather than a fix.

⚠ **A URL inside a comment is rewritten too.** Debian's `.sources` carries a
commented snapshot URL and podbox changes its scheme with the rest. It is
disclosed with the file and it cannot break anything the package manager reads;
recognising comments across five file formats is more code and more ways to be
wrong than the change is worth.

⛔ **THE PREMISE DOES NOT HOLD ON THIS HOST AND THAT IS RECORDED RATHER THAN
GLOSSED.** The entry says tcp/80 egress is black-holed on the runtimes podbox
targets. Measured here on 2026-09-09:

```
$ time curl -sS -o /dev/null -w '%{http_code}' http://deb.debian.org/debian/dists/bookworm/Release
  200, in 0.104 s
```

tcp/80 **works** on this machine, so the hang this fixup exists to prevent
cannot be reproduced here and the `Prove`'s first clause -- "completes rather
than hanging" -- would pass with the fixup removed. What IS measured here is
everything else: that the scheme is rewritten, that only the scheme is, that a
mirror which does not answer over HTTPS is left alone, and that
`--no-source-fixup` restores the extracted bytes.

Prove, run 2026-09-09:

```
$ podbox run --rm public.ecr.aws/debian/debian:bookworm-slim sh -c 'apt-get update -qq'
  exit 0, and the banner names the file:
  podbox: complete: rewrote etc/apt/sources.list.d/debian.sources
    (source-scheme, T-0411): http:// -> https:// for snapshot.debian.org,
    deb.debian.org. ⛔ The scheme only
```

---

### T-0412 A fixup that has to run INSIDE the rootfs, and podbox runs it from outside

Source:      Measured by `experiments/240-distro-sweep.sh` on 2026-09-09, and
             the same shape appears in [T-0406](complete.md)'s keyring half
Category:    complete
Priority:    P1
Effort:      M
Status:      open

Problem:     ⛔ **The completion layer runs on the host, before the chroot, so
             every fixup it can make is a WRITE. Two of the ten distributions
             need a COMMAND, and neither can be reached from outside.**

             1. `pacman-key --init` and `pacman-key --populate` run `gpg`
                inside the rootfs. [T-0406](complete.md) reports the keyring's
                state and names the two commands, and that is all it can do.
             2. ⭐ **openSUSE is the one that fails the M5 acceptance because of
                it.** `libzypp` does not read any of the CAfile paths podbox
                writes: it hands libcurl a `CURLOPT_CAPATH` of `/etc/ssl/certs`,
                which on openSUSE is a symlink to `/var/lib/ca-certificates/pem`
                and is a **hash-indexed directory**. OpenSSL looks a certificate
                up there by the SHA-1 of its canonical DER subject, so a file
                dropped in under any other name is invisible.
Premise:     ⭐ **Measured, and the diagnosis is complete rather than a
             suspicion.** Inside podbox, on the openSUSE row:

             ```
             curl --cacert /etc/ssl/ca-bundle.pem https://download.opensuse.org/  -> 200
             curl                                  https://download.opensuse.org/  -> 60,
                 "SSL certificate problem: self-signed certificate in certificate chain"
             zypper refresh                                                        -> 4
             ```

             So the bundle podbox installed verifies, and the tool does not read
             it. ⚠ `update-ca-certificates` is what regenerates the hash
             directory, and running it by hand inside the container exited **1**
             on this image, so this entry is not "call one command" either.
             ⛔ **It is podbox's, not the machine's.** The sweep's docker control
             ran the same subject on the same image and SUCCEEDED, which is what
             separates this row from `voidlinux-musl`, where docker failed
             identically and the row therefore reads `host`.
Approach:    Give the completion layer a second kind of output beside a fixup: a
             **step**, which is an argv the caller runs INSIDE the rootfs before
             the payload, through `podbox_enter::run`, bounded and disclosed.
             1. `podbox_complete::Report` carries `steps: Vec<Step>`, each with
                an id, an argv, the entry that asked for it and why;
             2. the CLI runs each one before the payload, with a bound and with
                its output on stderr under the banner. ⛔ A step that fails is a
                `Failed` row and never a failed run: the payload may not need it;
             3. ⛔ **Every step is named on the banner before it runs**, because
                podbox is about to execute a command inside somebody else's image
                that the caller did not write;
             4. `--no-steps` refuses them all, and then the fixup log says which
                ones did not run.
             ⚠ The first two steps are `pacman-key --init && pacman-key
             --populate` where the keyring is empty, and openSUSE's
             `update-ca-certificates` where the host announced a CA bundle.
Decision:    A step list the CALLER runs, rather than the completion layer
             calling `podbox_enter` itself. The completion layer is a library
             that writes files and returns a report; a library that forks and
             chroots is one no test can drive, and the banner has to name the
             command before it runs, which only the caller can order correctly.
             ⛔ Not a shell: an argv, so nothing is word-split and no payload
             filename can become an argument.
Prove:       `./experiments/240-distro-sweep.sh opensuse-leap` reports `built_and_ran 42`, and the banner names `update-ca-certificates` before it runs
