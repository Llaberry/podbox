#!/usr/bin/env bash
# bootstrap-env.sh - bring a barebones container up to what this repository's
# gate, experiments and builds need, idempotently and without a prompt.
#
# ⭐ EVERY SESSION RUNS IN A NEW CONTAINER. The tools a previous session
# installed are gone, and a session that discovers that halfway through an
# experiment has already wasted the discovery. Run this first.
#
#   ./scripts/common/bootstrap-env.sh            install what is missing
#   ./scripts/common/bootstrap-env.sh --check     report only, change nothing
#   ./scripts/common/bootstrap-env.sh rust docker install only these
#
# Components, and what needs each:
#
#   rust      cargo, rustc, and the x86_64-unknown-linux-musl target.
#             Everything. `.cargo/config.toml` builds for musl by default.
#   bloat     cargo-bloat. TODO/deps.md T-0910; experiments/110- exits 2 without it.
#   go        the reconstruction's confine and probe harnesses, built from the
#             corpus by experiments/20-enter-target.sh, which exits 2 without it.
#   cc        gcc, for the corpus's C probe and for ordinary linking.
#   zig       `zig cc`, the C cross-compiler for the musl target. scripts/zig-cc.sh
#             says why it is preferred over musl-tools.
#   docker    the daemon, STARTED. experiments/20-, 30-, 70-, 80-, 90-, 100-,
#             125- and 130- all exit 2 without it.
#   tools     jq, binutils (readelf, nm), file, xz, ca-certificates.
#   qemu      ⭐ TWO SEPARATE THINGS AND BOTH ARE NEEDED.
#             `qemu-user-static` plus binfmt_misc runs a FOREIGN-ARCHITECTURE
#             binary on this one, which is how experiments/260- measures podbox
#             on aarch64 and how TODO/enter.md T-0506 runs a foreign image.
#             `qemu-system-x86_64` boots a whole VM, which is how
#             experiments/290- answers the two questions this host's kernel
#             cannot: it has neither CONFIG_SECURITY_LANDLOCK nor
#             CONFIG_CHECKPOINT_RESTORE, and a stock distro kernel has both.
#             ⚠ There is no /dev/kvm here, so the VM runs under TCG. Measured on
#             2026-09-09: the whole boot-probe-poweroff cycle is about 6 s.
#   vmtools   busybox-static and cpio, which build experiments/290-'s initramfs.
#
# ⛔ THREE RULES THIS SCRIPT HOLDS TO, each because the environment breaks a
# session that does not:
#
#   1. NEVER INTERACTIVE. A prompt with no terminal behind it is a hang, and a
#      hang costs the session. Everything carries -y, --noconfirm,
#      DEBIAN_FRONTEND=noninteractive and </dev/null.
#   2. NEVER UNBOUNDED. Every command that touches the network, a registry or
#      another process runs under `timeout`.
#   3. ⛔ NEVER DISABLE TLS VERIFICATION. Outbound HTTPS here is intercepted,
#      and the answer is to trust the host's bundle, never to pass -k. A
#      download whose checksum this script cannot verify is a failure, not a
#      warning.
#
# ⚠ Writable space is a fixed allowance, so `df` misleads: `Avail 0` beside a
# low `Used` means the allowance is spent. Blocks AND inodes are checked before
# anything large is written.
#
# Exit: 0 everything requested is present, 1 something required failed,
#       2 could not run at all.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/../.." && pwd)"

# ⛔ PINNED. `docs/methodology/experiments.md` wants pinned inputs, and a
# toolchain that changes under a measurement makes two runs incomparable.
# The checksum is from https://ziglang.org/download/index.json for this version
# and is verified before anything is unpacked.
ZIG_VERSION="0.16.0"
ZIG_SHA256="70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00"
ZIG_PREFIX="/opt/zig"

# What a build or an image pull needs before it starts, per TODO/RULES.md
# section 8. Both, because either one exhausting is the same failure.
NEED_BLOCKS_MB=3000
NEED_INODES=50000

CHECK_ONLY=0
WANT=""
for a in "$@"; do
	case "$a" in
	--check) CHECK_ONLY=1 ;;
	-h | --help) sed -n '2,45p' "$0"; exit 0 ;;
	-*) echo "bootstrap-env: unknown option $a" >&2; exit 2 ;;
	*) WANT="$WANT $a" ;;
	esac
done
[ -z "$WANT" ] && WANT="rust bloat go cc zig docker tools qemu vmtools"

installed=0 already=0 failed=0
declare -a REPORT=()

note() { REPORT+=("$1"); }

want() {
	case " $WANT " in *" $1 "*) return 0 ;; *) return 1 ;; esac
}

# have <name> <command...>   true when the command answers
have() { command -v "$1" >/dev/null 2>&1; }

ok() { note "  ok       $1"; already=$((already + 1)); }
did() { note "  installed $1"; installed=$((installed + 1)); }
bad() { note "  FAILED   $1"; failed=$((failed + 1)); }
skipped() { note "  skip     $1"; }

# ------------------------------------------------------------------ the disk
#
# ⛔ Checked FIRST and reported whatever happens, because every failure below
# this line looks like a broken tool when it is a full allowance.
avail_mb="$(df -Pm "$ROOT" 2>/dev/null | awk 'NR==2{print $4}')"
avail_inodes="$(df -Pi "$ROOT" 2>/dev/null | awk 'NR==2{print $4}')"
echo "== disk, before anything is written"
printf '  %s MB and %s inodes available under %s\n' \
	"${avail_mb:-?}" "${avail_inodes:-?}" "$ROOT"
disk_short=0
if [ -n "$avail_mb" ] && [ "$avail_mb" -lt "$NEED_BLOCKS_MB" ]; then
	echo "  ⚠ under ${NEED_BLOCKS_MB} MB. Deletes still succeed while writes fail:" >&2
	echo "    remove build artefacts and caches before retrying." >&2
	disk_short=1
fi
if [ -n "$avail_inodes" ] && [ "$avail_inodes" -lt "$NEED_INODES" ]; then
	echo "  ⚠ under ${NEED_INODES} inodes. An image pull writes many small files." >&2
	disk_short=1
fi

# ⚠ NOT EVERY MACHINE THIS RUNS ON IS ROOT. A GitHub runner is not, and
# `apt-get` there needs `sudo`; this container is root and has no `sudo` at
# all. Choosing once, out loud, beats every call site guessing.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
	if command -v sudo >/dev/null 2>&1; then
		SUDO="sudo -n"
	else
		echo "⚠ not root and no sudo: anything needing a package install will be" >&2
		echo "  reported as FAILED rather than attempted." >&2
	fi
fi

apt_updated=0
apt_install() {
	# One update for the whole run, and only if something is actually missing.
	if [ "$(id -u)" -ne 0 ] && [ -z "$SUDO" ]; then
		return 1
	fi
	if [ "$apt_updated" -eq 0 ]; then
		DEBIAN_FRONTEND=noninteractive timeout 300 $SUDO apt-get -qq update >/dev/null 2>&1
		apt_updated=1
	fi
	DEBIAN_FRONTEND=noninteractive timeout 600 $SUDO apt-get install -y -qq "$@" \
		>/dev/null 2>&1 </dev/null
}

echo
echo "== components"

# ------------------------------------------------------------------ rust
if want rust; then
	if ! have cargo; then
		bad "rust: cargo is not on PATH and this script does not install a toolchain.
           Install rustup, or run in an image that carries one."
	elif rustup target list --installed 2>/dev/null | grep -qx x86_64-unknown-linux-musl; then
		ok "rust: $(cargo --version | cut -d' ' -f2), musl target present"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "rust: the musl target is missing (--check, not installing)"
	elif timeout 600 rustup target add x86_64-unknown-linux-musl >/dev/null 2>&1; then
		did "rust: x86_64-unknown-linux-musl target"
	else
		bad "rust: could not add the x86_64-unknown-linux-musl target"
	fi
fi

# ------------------------------------------------------------------ cc
if want cc; then
	if have gcc; then
		ok "cc: gcc $(gcc -dumpversion)"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "cc: gcc absent (--check)"
	elif apt_install gcc libc6-dev; then
		have gcc && did "cc: gcc $(gcc -dumpversion)" || bad "cc: gcc did not appear after install"
	else
		bad "cc: apt-get install gcc failed"
	fi
fi

# ------------------------------------------------------------------ tools
if want tools; then
	missing=""
	for t in jq readelf nm file xz; do have "$t" || missing="$missing $t"; done
	if [ -z "$missing" ]; then
		ok "tools: jq, binutils, file, xz all present"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "tools: missing$missing (--check)"
	elif apt_install jq binutils file xz-utils ca-certificates; then
		still=""
		for t in jq readelf nm file xz; do have "$t" || still="$still $t"; done
		[ -z "$still" ] && did "tools:$missing" || bad "tools: still missing$still"
	else
		bad "tools: apt-get install failed for$missing"
	fi
fi

# ------------------------------------------------------------------ qemu
#
# ⭐ Both halves, because they answer different questions and a session that has
# one and not the other discovers it halfway through an experiment.
if want qemu; then
	missing=""
	have qemu-aarch64-static || missing="$missing qemu-user-static"
	have qemu-system-x86_64 || missing="$missing qemu-system-x86"
	if [ -z "$missing" ]; then
		ok "qemu: user-static and system-x86 both present"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "qemu: missing$missing (--check). experiments/260- and 290- exit 2 without them"
	elif apt_install $missing; then
		still=""
		have qemu-aarch64-static || still="$still qemu-user-static"
		have qemu-system-x86_64 || still="$still qemu-system-x86"
		[ -z "$still" ] && did "qemu:$missing" || bad "qemu: still missing$still"
	else
		bad "qemu: apt-get install failed for$missing"
	fi
fi

# ------------------------------------------------------------------ vmtools
if want vmtools; then
	missing=""
	# ⚠ busybox-static and not busybox: the initramfs has no libc in it.
	[ -x /bin/busybox ] || [ -x /usr/bin/busybox ] || missing="$missing busybox-static"
	have cpio || missing="$missing cpio"
	if [ -z "$missing" ]; then
		ok "vmtools: busybox and cpio present"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "vmtools: missing$missing (--check). experiments/290- exits 2 without them"
	elif apt_install $missing; then
		still=""
		[ -x /bin/busybox ] || [ -x /usr/bin/busybox ] || still="$still busybox-static"
		have cpio || still="$still cpio"
		[ -z "$still" ] && did "vmtools:$missing" || bad "vmtools: still missing$still"
	else
		bad "vmtools: apt-get install failed for$missing"
	fi
fi

# ------------------------------------------------------------------ go
if want go; then
	if have go; then
		ok "go: $(go version | cut -d' ' -f3)"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "go: absent (--check). experiments/20- exits 2 without it"
	elif apt_install golang-go; then
		have go && did "go: $(go version | cut -d' ' -f3)" || bad "go: did not appear after install"
	else
		bad "go: apt-get install golang-go failed"
	fi
fi

# ------------------------------------------------------------------ bloat
if want bloat; then
	if cargo bloat --version >/dev/null 2>&1; then
		ok "bloat: cargo-bloat $(cargo bloat --version 2>/dev/null)"
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "bloat: cargo-bloat absent (--check). experiments/110- exits 2 without it"
	elif ! have cargo; then
		bad "bloat: cargo is not on PATH"
	elif timeout 900 cargo install cargo-bloat --locked >/dev/null 2>&1; then
		did "bloat: cargo-bloat"
	else
		bad "bloat: cargo install cargo-bloat failed"
	fi
fi

# ------------------------------------------------------------------ zig
if want zig; then
	if have zig && [ "$(zig version 2>/dev/null)" = "$ZIG_VERSION" ]; then
		ok "zig: $ZIG_VERSION"
	elif have zig; then
		note "  ⚠ zig: $(zig version) is on PATH and this script pins $ZIG_VERSION.
           Left alone: replacing a toolchain somebody chose is not this
           script's decision. Measurements taken with it say which version."
		already=$((already + 1))
	elif [ "$CHECK_ONLY" -eq 1 ]; then
		skipped "zig: absent (--check)"
	elif [ "$disk_short" -eq 1 ]; then
		bad "zig: not attempted, the disk allowance is short and the tarball is ~53 MB"
	else
		tmp="$(mktemp -d)"
		url="https://ziglang.org/download/$ZIG_VERSION/zig-x86_64-linux-$ZIG_VERSION.tar.xz"
		# ⛔ Verified before it is unpacked. A download this script cannot check
		# is a failure: `docs/conventions/code.md` calls an unverified restore
		# the class that corrupts silently.
		if ! timeout 900 curl -fsS -o "$tmp/zig.tar.xz" "$url"; then
			bad "zig: download failed from $url"
		else
			got="$(sha256sum "$tmp/zig.tar.xz" | cut -d' ' -f1)"
			if [ "$got" != "$ZIG_SHA256" ]; then
				bad "zig: sha256 mismatch. Expected $ZIG_SHA256, got $got.
           ⛔ NOT unpacked. Re-pin the checksum in this script from
           https://ziglang.org/download/index.json if the version moved."
			elif timeout 300 tar -xf "$tmp/zig.tar.xz" -C "$tmp" &&
				$SUDO rm -rf "${ZIG_PREFIX:?}" &&
				$SUDO mv "$tmp/zig-x86_64-linux-$ZIG_VERSION" "$ZIG_PREFIX" &&
				$SUDO ln -sf "$ZIG_PREFIX/zig" /usr/local/bin/zig &&
				[ "$(zig version 2>/dev/null)" = "$ZIG_VERSION" ]; then
				did "zig: $ZIG_VERSION at $ZIG_PREFIX, linked as /usr/local/bin/zig"
			else
				bad "zig: unpack or link failed"
			fi
		fi
		rm -rf "$tmp"
	fi
fi

# ------------------------------------------------------------------ docker
#
# ⛔ INSTALLED IS NOT RUNNING. dockerd does not survive between turns here, and
# every experiment that needs it exits 2 rather than failing loudly, so a
# session can spend a long time reading SKIP lines that mean "start the daemon".
if want docker; then
	if ! have docker; then
		if [ "$CHECK_ONLY" -eq 1 ]; then
			skipped "docker: not on PATH (--check)"
		elif apt_install docker.io; then
			have docker && did "docker: client and daemon installed" ||
				bad "docker: did not appear after install"
		else
			bad "docker: apt-get install docker.io failed"
		fi
	fi
	if have docker; then
		if timeout 15 docker info >/dev/null 2>&1; then
			ok "docker: daemon reachable, $(timeout 15 docker version --format '{{.Server.Version}}' 2>/dev/null)"
		elif [ "$CHECK_ONLY" -eq 1 ]; then
			skipped "docker: installed and NOT running (--check)"
		else
			# ⚠ Bounded wait on a condition, never a bare sleep and never
			# unbounded: TODO/RULES.md section 8.
			nohup $SUDO dockerd >/tmp/dockerd.log 2>&1 &
			for _ in $(seq 1 20); do
				timeout 5 docker info >/dev/null 2>&1 && break
				sleep 2
			done
			if timeout 15 docker info >/dev/null 2>&1; then
				did "docker: daemon started, log at /tmp/dockerd.log"
			else
				bad "docker: daemon did not come up in 40 s. See /tmp/dockerd.log"
			fi
		fi
	fi
fi

# ------------------------------------------------------------------ report
echo
printf '%s\n' "${REPORT[@]}"
echo
printf '== %d already present, %d installed, %d failed\n' "$already" "$installed" "$failed"

if [ "$failed" -gt 0 ]; then
	echo
	echo "⛔ Something a session needs is not here, and the experiments that need it" >&2
	echo "   exit 2 rather than failing, so read the FAILED lines above before" >&2
	echo "   treating a SKIP as a result." >&2
	exit 1
fi
echo "everything requested is present."
exit 0
