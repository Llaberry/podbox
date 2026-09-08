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
Status:      open

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

---

### T-0402 Always install the host's `/etc/resolv.conf`

Source:      `TOOL.md` section 6.4, section 8
Category:    complete
Priority:    P0
Effort:      S
Status:      open

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

---

### T-0403 `/etc/hosts`

Source:      `TOOL.md` section 6.4, section 6.8
Category:    complete
Priority:    P2
Effort:      S
Status:      open

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

---

### T-0404 Synthesize `/etc/passwd` and `/etc/group`, and let the payload's writes persist

Source:      `TOOL.md` section 6.4, section 8; `references/indigo-dc__udocker` tracker #141
Category:    complete
Priority:    P0
Effort:      M
Status:      open

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

---

### T-0405 `/etc/mtab` is a symlink, and writing through it escapes the rootfs

Source:      `TOOL.md` section 6.4; `paper_final.md` section 8.3 and F10
Category:    complete
Priority:    P1
Effort:      S
Status:      open

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

---

### T-0406 pacman: `DownloadUser` and the keyring

Source:      `TOOL.md` section 6.4, section 8; `paper_final.md` section 8.3 and F9
Category:    complete
Priority:    P1
Effort:      S
Status:      open

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

---

### T-0407 apt: the sandbox user, https sources and the CA bundle

Source:      `TOOL.md` section 6.4, section 8
Category:    complete
Priority:    P1
Effort:      S
Status:      open

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

---

### T-0408 zypper: fix the RIS index, not `repos.d`

Source:      `TOOL.md` section 6.4, section 8
Category:    complete
Priority:    P2
Effort:      S
Status:      open

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
Status:      open

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
