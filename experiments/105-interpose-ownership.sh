#!/usr/bin/env bash
# Question: does podbox's own interposer clear the ownership wall, and does it
# stay quiet on a machine that has no wall?
#
# TODO/interpose.md T-0701 (the cdylib's build constraints) and T-0704
# (ownership virtualization). ⭐ This is the half a path interposer does not
# have: `references/VHSgunzo__pathmap/tree/path-mapping.c:1240-1241` routes
# `chown` through the same path-rewriting macro as everything else and passes
# the ids through untouched, so a payload's `chown 0:42` fails identically with
# and without it loaded.
#
# Six checks:
#
#   A. the EXPORTED SET. T-0701 constraint 1: exactly the entry points, and no
#      `rust_eh_personality`. Asserted against `interpose.map` itself, so the
#      version script and the `#[no_mangle]` list cannot drift apart.
#   B. `struct stat` AND `struct statx` OFFSETS, under both libcs, by `offsetof`
#      rather than by reading a header. `src/lib.rs` carries all of them, and a
#      check that asserted only the first struct would have left unmeasured the
#      one that actually answers a modern glibc payload.
#   C. THE CONTROL, and it comes first: on a machine that CAN chown, podbox must
#      change nothing and write no memo. ⛔ An interposer that reported the
#      caller's intent where the kernel would have done the real thing is weaker
#      than the bare chroot it replaces.
#   D. THE WALL, glibc. `--cap-drop=CHOWN` is root without `CAP_CHOWN`, which is
#      the same refusal this runtime gives for an unmapped id. Without the
#      object the chown fails; with it the chown succeeds and `stat` reports
#      what the payload meant.
#   E. THE WALL, musl, with the musl-linked object, because T-0702 is that one
#      object per libc is required and an arm that only ran under glibc would
#      not have tested that.
#   F. THE PAIR podbox MUST REFUSE, and it is podbox's own object rather than
#      the C reference interposer 80-interposer-abi.sh uses: this object is
#      built on glibc 2.39 and imports `dlsym@GLIBC_2.34`, so a 2.31 payload
#      cannot load it. T-0709's reader says so from ELF, with nothing loaded,
#      and the loader is asked afterwards to agree.
#
# ⚠ WHY NOT THIS HOST DIRECTLY. It grants real ownership -- `podbox probe` reads
# `ownership: real` -- so `chown 0:42` SUCCEEDS here and the memo path never
# runs. A check that passed for that reason would be measuring the kernel.
# `--cap-drop=CHOWN` removes exactly the capability and nothing else.
#
# Inputs pinned: the two images by manifest digest, and the objects
# `scripts/build-interpose.sh` produces from this tree.
#
# Exit: 0 every check that ran matched, 1 one did not, 2 a check could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
CRATE="$ROOT/crates/podbox-interpose"
OUT="${OUT:-$HERE/.ownership}"
rm -rf "$OUT"
mkdir -p "$OUT"

GNU_SO="$CRATE/target/x86_64-unknown-linux-gnu/release/libpodbox_interpose.so"
MUSL_SO="$CRATE/target/x86_64-unknown-linux-musl/release/libpodbox_interpose.so"
# ⚠ THE GLIBC PAYLOAD IS NEWER THAN THE BUILD HOST'S, and that is not a
# convenience. This object is linked against the host's glibc 2.39 and imports
# `dlsym@GLIBC_2.34`, so it CANNOT be loaded into a 2.31 payload: check F puts
# the old one to podbox's own reader and requires it to be refused before
# anything is loaded, which is TODO/interpose.md T-0709.
GLIBC_PAYLOAD='quay.io/fedora/fedora@sha256:e78cd1a688cd079c23864f289a89a49a3f4ad66d817864e325e1d058310ee95c'
GLIBC_TOO_OLD='ubuntu@sha256:c664f8f86ed5a386b0a340d981b8f81714e21a8b9c73f658c4bea56aa179d54a'
MUSL_PAYLOAD='alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
_ldd="$(ldd --version 2>&1)"
printf 'host libc         %s\n' "${_ldd%%$'\n'*}"
printf 'zig               %s\n' "$(zig version 2>/dev/null || echo absent)"
_dv="$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
printf 'docker            %s\n' "${_dv:-MISSING}"
printf 'glibc payload     %s\n' "$GLIBC_PAYLOAD"
printf 'glibc too old     %s\n' "$GLIBC_TOO_OLD"
printf 'musl payload      %s\n' "$MUSL_PAYLOAD"
echo

rc=0
could_not=0
fail() { printf 'FAIL %s\n' "$1"; rc=1; }
pass() { printf 'ok   %s\n' "$1"; }
cannot() { printf '  COULD NOT RUN: %s\n' "$1"; could_not=1; }

if [ ! -r "$GNU_SO" ] || [ ! -r "$MUSL_SO" ]; then
	echo "SKIP: the objects are not built. ./scripts/build-interpose.sh" >&2
	exit 2
fi

echo "== A. the exported set, against the version script"
# ⚠ The version script is the SOURCE of this list, and the object is what is
# compared with it: a name in `interpose.map` that `src/lib.rs` does not define
# is a silent non-interposition, and one the object exports that the script does
# not list is a symbol some other library in the payload's process would resolve
# to podbox.
awk '/global:/{g=1;next} /local:/{g=0} g && /;/{gsub(/[ \t;]/,"");if($0!="")print}' \
	"$CRATE/interpose.map" | sort >"$OUT/declared"
nm -D --defined-only "$GNU_SO" | awk '$2=="T"{print $3}' | sort >"$OUT/exported.gnu"
nm -D --defined-only "$MUSL_SO" | awk '$2=="T"{print $3}' | sort >"$OUT/exported.musl"
printf '  declared in interpose.map  %s\n' "$(wc -l <"$OUT/declared")"
printf '  exported by the gnu object %s\n' "$(wc -l <"$OUT/exported.gnu")"
printf '  exported by the musl object %s\n' "$(wc -l <"$OUT/exported.musl")"
for arm in gnu musl; do
	if diff -q "$OUT/declared" "$OUT/exported.$arm" >/dev/null; then
		pass "A: the $arm object exports exactly what interpose.map declares"
	else
		fail "A: the $arm object's exports differ from interpose.map"
		diff "$OUT/declared" "$OUT/exported.$arm" | sed 's/^/      /'
	fi
done
# ⛔ T-0701's own Prove. A default Rust cdylib exports this into every process.
for arm in gnu musl; do
	so="$GNU_SO"
	[ "$arm" = musl ] && so="$MUSL_SO"
	n="$(nm -D --defined-only "$so" | grep -c 'rust_eh_personality')"
	printf '  rust_eh_personality in the %s object: %s\n' "$arm" "$n"
	[ "$n" -eq 0 ] || fail "A: the $arm object exports rust_eh_personality"
done
echo

echo "== B. struct stat offsets, by offsetof and under both libcs"
cat >"$OUT/off.c" <<'EOF'
#define _GNU_SOURCE
#include <stdio.h>
#include <sys/stat.h>
#include <stddef.h>
/* ⚠ NOT <linux/stat.h>. musl's own <sys/stat.h> defines `struct statx` and the
 * kernel header then redefines it; glibc since 2.28 defines it there too under
 * _GNU_SOURCE. ⭐ Two libcs declaring one kernel struct in two different headers
 * is exactly why podbox's object carries OFFSETS rather than a struct. */
int main(void){
  printf("sizeof=%zu dev=%zu ino=%zu mode=%zu uid=%zu gid=%zu\n",
    sizeof(struct stat), offsetof(struct stat, st_dev), offsetof(struct stat, st_ino),
    offsetof(struct stat, st_mode), offsetof(struct stat, st_uid),
    offsetof(struct stat, st_gid));
  /* ⭐ statx too, and it is not a duplicate: coreutils' `stat` on a modern
   * glibc asks statx(2) and never reaches `stat`. src/lib.rs carries these
   * six numbers as well, and a check that asserted only the first struct
   * would have left the ones that actually answer a glibc payload unmeasured. */
  printf("statx sizeof=%zu mask=%zu uid=%zu gid=%zu ino=%zu devmaj=%zu devmin=%zu\n",
    sizeof(struct statx), offsetof(struct statx, stx_mask),
    offsetof(struct statx, stx_uid), offsetof(struct statx, stx_gid),
    offsetof(struct statx, stx_ino), offsetof(struct statx, stx_dev_major),
    offsetof(struct statx, stx_dev_minor));
  return 0;
}
EOF
g_off=""
m_off=""
if cc -o "$OUT/off_g" "$OUT/off.c" 2>/dev/null; then g_off="$("$OUT/off_g" | tr '\n' '|')"; fi
if command -v zig >/dev/null 2>&1 &&
	ZIG_TARGET=x86_64-linux-musl "$ROOT/scripts/zig-cc.sh" -o "$OUT/off_m" "$OUT/off.c" 2>/dev/null; then
	m_off="$("$OUT/off_m" | tr '\n' '|')"
fi
printf '  glibc  %s\n' "${g_off:-COULD NOT BUILD}"
printf '  musl   %s\n' "${m_off:-COULD NOT BUILD}"
# ⚠ The numbers `crates/podbox-interpose/src/lib.rs` carries, asserted here
# rather than believed. ⛔ They agree on x86_64 and that is this architecture's
# property, not a general one: T-0702's premise is that struct layout is what a
# preload cannot bridge.
WANT='sizeof=144 dev=0 ino=8 mode=24 uid=28 gid=32|statx sizeof=256 mask=0 uid=20 gid=24 ino=32 devmaj=136 devmin=140|'
if [ -z "$g_off" ] || [ -z "$m_off" ]; then
	cannot "one of the two offsetof programs did not build"
elif [ "$g_off" = "$WANT" ] && [ "$m_off" = "$WANT" ]; then
	pass "B: both libcs agree on BOTH structs, and with the offsets src/lib.rs carries"
else
	fail "B: the offsets differ from the ones src/lib.rs carries [$WANT]"
fi
echo

command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 || {
	echo "SKIP: no docker daemon, so C, D and E cannot run" >&2
	exit 2
}

# subject SO IMAGE CAPS -> prints "<rc-of-chown> <what stat reports> <memo bytes>"
subject() {
	local so="$1" img="$2" caps="$3" preload="$4"
	# ⛔ ONE `sh -c`, so the chown and the stat are two EXECVEs of one shell:
	# T-0704's own Prove shape, and the reason the memo cannot live in memory.
	# shellcheck disable=SC2086
	timeout 180 docker run --rm $caps -v "$so":/i.so:ro "$img" /bin/sh -c "
    ${preload}
    : >/tmp/f
    chown 0:42 /tmp/f 2>/tmp/e; echo \"CHOWN_RC=\$?\"
    echo \"CHOWN_ERR=\$(head -1 /tmp/e 2>/dev/null | cut -c1-80)\"
    echo \"STAT=\$(stat -c %u:%g /tmp/f 2>/dev/null)\"
    echo \"MEMO=\$( [ -f /.podbox/ownership.memo ] && wc -c </.podbox/ownership.memo || echo none )\"
  " 2>&1
}

echo "== C. the control: a machine that CAN chown, with the object loaded"
# ⛔ FIRST. An interposer that reported the caller's intent where the kernel
# would have done the real thing is weaker than the bare chroot it replaces, and
# every arm below would pass for that reason.
c_out="$(subject "$GNU_SO" "$GLIBC_PAYLOAD" "" 'export LD_PRELOAD=/i.so')"
printf '%s\n' "$c_out" | sed 's/^/  /'
c_stat="$(printf '%s' "$c_out" | sed -n 's/^STAT=//p')"
c_memo="$(printf '%s' "$c_out" | sed -n 's/^MEMO=//p')"
if [ "$c_stat" = "0:42" ] && [ "$c_memo" = "none" ]; then
	pass "C: the real chown worked and podbox wrote no memo"
else
	fail "C: stat says [$c_stat] and the memo is [$c_memo]; it should be 0:42 and none"
fi
echo

echo "== D. the wall, glibc: root WITHOUT CAP_CHOWN"
# ⚠ `--cap-drop=CHOWN` refuses the call with EPERM, which is one of the two
# errnos T-0704 names. The runtime podbox targets answers EINVAL for an unmapped
# id, and the object swallows both; fakeroot tests EPERM alone at eight sites
# and would return the error on every one of them here.
d_bare="$(subject "$GNU_SO" "$GLIBC_PAYLOAD" "--cap-drop=CHOWN" 'true')"
printf '  without the object:\n'
printf '%s\n' "$d_bare" | sed 's/^/    /'
d_pre="$(subject "$GNU_SO" "$GLIBC_PAYLOAD" "--cap-drop=CHOWN" 'export LD_PRELOAD=/i.so')"
printf '  with the object:\n'
printf '%s\n' "$d_pre" | sed 's/^/    /'
bare_rc="$(printf '%s' "$d_bare" | sed -n 's/^CHOWN_RC=//p')"
pre_rc="$(printf '%s' "$d_pre" | sed -n 's/^CHOWN_RC=//p')"
pre_stat="$(printf '%s' "$d_pre" | sed -n 's/^STAT=//p')"
if [ "$bare_rc" = "0" ]; then
	cannot "the control chown SUCCEEDED without CAP_CHOWN, so there is no wall here"
elif [ "$pre_rc" = "0" ] && [ "$pre_stat" = "0:42" ]; then
	pass "D: the bare chown failed (rc=$bare_rc) and podbox's answered 0:42"
else
	fail "D: with the object the chown exited $pre_rc and stat says [$pre_stat]"
fi
echo

echo "== E. the wall, musl, with the musl-linked object"
e_bare="$(subject "$MUSL_SO" "$MUSL_PAYLOAD" "--cap-drop=CHOWN" 'true')"
printf '  without the object:\n'
printf '%s\n' "$e_bare" | sed 's/^/    /'
e_pre="$(subject "$MUSL_SO" "$MUSL_PAYLOAD" "--cap-drop=CHOWN" 'export LD_PRELOAD=/i.so')"
printf '  with the object:\n'
printf '%s\n' "$e_pre" | sed 's/^/    /'
e_bare_rc="$(printf '%s' "$e_bare" | sed -n 's/^CHOWN_RC=//p')"
e_pre_rc="$(printf '%s' "$e_pre" | sed -n 's/^CHOWN_RC=//p')"
e_pre_stat="$(printf '%s' "$e_pre" | sed -n 's/^STAT=//p')"
if [ "$e_bare_rc" = "0" ]; then
	cannot "the musl control chown SUCCEEDED without CAP_CHOWN"
elif [ "$e_pre_rc" = "0" ] && [ "$e_pre_stat" = "0:42" ]; then
	pass "E: the musl object answered 0:42 where the bare chown failed (rc=$e_bare_rc)"
else
	fail "E: with the musl object the chown exited $e_pre_rc and stat says [$e_pre_stat]"
fi
echo

echo "== F. the pair podbox must refuse BEFORE loading anything"
# ⭐ TODO/interpose.md T-0709, against podbox's OWN object rather than the C
# reference interposer 80-interposer-abi.sh uses. The object is built on glibc
# 2.39 and imports `dlsym@GLIBC_2.34`; the older payload declares up to
# GLIBC_2.31. ⛔ The reader must say so from ELF, with nothing loaded, and the
# loader must agree when the pair is forced.
BIN="${PODBOX_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podbox}"
if [ ! -x "$BIN" ]; then
	cannot "$BIN is not an executable, so the reader cannot be asked"
else
	cid="$(timeout 180 docker create "$GLIBC_TOO_OLD" /bin/true 2>/dev/null)"
	if [ -n "$cid" ] && timeout 180 docker cp -L "$cid:/lib/x86_64-linux-gnu/libc.so.6" \
		"$OUT/old-libc.so.6" >/dev/null 2>&1 && [ -s "$OUT/old-libc.so.6" ]; then
		docker rm -f "$cid" >/dev/null 2>&1
		f_out="$("$BIN" system abi "$GNU_SO" "$OUT/old-libc.so.6" 2>&1)"
		f_rc=$?
		printf '  podbox system abi: rc=%s %s\n' "$f_rc" \
			"$(printf '%s' "$f_out" | head -1 | sed "s#$ROOT/##g" | cut -c1-100)"
		f_loader="$(subject "$GNU_SO" "$GLIBC_TOO_OLD" "" 'export LD_PRELOAD=/i.so' |
			grep -oE "GLIBC_[0-9.]+.? not found" | head -1)"
		printf '  and the loader:    %s\n' "${f_loader:-it loaded}"
		if [ "$f_rc" = 1 ] && [ -n "$f_loader" ]; then
			pass "F: the reader refused the pair from ELF and the loader agreed"
		elif [ "$f_rc" = 1 ]; then
			fail "F: the reader refused it and the loader did not"
		else
			fail "F: the reader answered rc=$f_rc for a pair the loader refuses"
		fi
	else
		docker rm -f "$cid" >/dev/null 2>&1
		cannot "the old payload's libc could not be copied out"
	fi
fi
echo

echo "== verdict"
if [ "$rc" -eq 0 ] && [ "$could_not" -eq 0 ]; then
	echo "  every check ran and matched. podbox's own object clears the ownership"
	echo "  wall and stays quiet where there is none."
elif [ "$rc" -eq 0 ]; then
	echo "  every check that ran matched; one or more could not be taken here."
else
	echo "  see the FAIL lines above"
fi
[ "$rc" -ne 0 ] || [ "$could_not" -eq 0 ] || exit 2
exit "$rc"
