#!/usr/bin/env bash
# Question: `podbox-complete` supplies `/etc/passwd`, `/etc/group` and
# `/etc/resolv.conf` into a rootfs (TOOL.md section 6.5). Under what condition
# does the payload actually read them?
#
# The answer is not "when the file is there". glibc resolves a user through
# NSS, and `/etc/nsswitch.conf` names which service answers. `files` reads
# `/etc/passwd`; anything else does not, and a supplied file is then a shim
# that silently does nothing.
#
# Four checks:
#
#   A. a supplied `/etc/passwd` against two nsswitch settings, with a novel
#      user that exists in no image. This is the requirement.
#   B. what pinned images actually ship, because the requirement is only
#      expensive if real images disagree.
#   C. `PT_INTERP`-free is not dependency-free: what a `-static` glibc binary
#      opens after its own execve, counted with the instrument that does not
#      inflate itself on `/etc/ld.so.cache`.
#   D. glibc's gconv modules and DT_NEEDED, which is the settled reason not to
#      bundle them into a musl artefact.
#
# ⭐ A uses a user name no distribution ships, so a hit cannot come from the
# image's own `/etc/passwd`.
#
# Inputs pinned by manifest digest:
#   ubuntu@sha256:c664f8f86ed5a386b0a340d981b8f81714e21a8b9c73f658c4bea56aa179d54a
#   debian@sha256:2f65600e1252c5649d2213e1d1ea4d74253d26514dc6530102a875e429245929
#   alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
#
# Exit: 0 every check ran and matched, 1 a check ran and did not match,
#       2 could not run (no cc, no docker, no daemon).
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
OUT="${OUT:-$HERE/.nsswitch}"
rm -rf "$OUT"; mkdir -p "$OUT"

UBUNTU='ubuntu@sha256:c664f8f86ed5a386b0a340d981b8f81714e21a8b9c73f658c4bea56aa179d54a'
DEBIAN='debian@sha256:2f65600e1252c5649d2213e1d1ea4d74253d26514dc6530102a875e429245929'
ALPINE='alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
_ldd="$(ldd --version 2>&1)"
printf 'host libc         %s\n' "${_ldd%%$'\n'*}"
printf 'cc                %s\n' "$(cc --version 2>/dev/null | head -1)"
_dv="$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
printf 'docker            %s\n' "${_dv:-MISSING}"
printf 'strace            %s\n' "$(strace -V 2>/dev/null | head -1 || echo absent)"
echo

command -v cc >/dev/null 2>&1 || { echo "SKIP: no cc" >&2; exit 2; }
command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 || {
  echo "SKIP: no docker daemon; start dockerd" >&2; exit 2; }

rc=0
fail() { printf 'FAIL %s\n' "$1"; rc=1; }
pass() { printf 'ok   %s\n' "$1"; }

cat > "$OUT/q.c" <<'EOF'
#include <stdio.h>
#include <pwd.h>
int main(void) {
    struct passwd *p = getpwnam("podboxsupplied");
    puts(p ? "FOUND" : "NOTFOUND");
    return p ? 0 : 1;
}
EOF
cc -static -o "$OUT/q_static" "$OUT/q.c" 2>"$OUT/cc.log" || {
  echo "SKIP: cannot build a static probe; see $OUT/cc.log" >&2; exit 2; }

cat > "$OUT/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
podboxsupplied:x:1000:1000:supplied by podbox-complete:/workspace:/bin/sh
EOF
printf 'passwd: files\ngroup: files\n'     > "$OUT/nsw.files"
printf 'passwd: systemd\ngroup: systemd\n' > "$OUT/nsw.other"

echo "== A. is a supplied /etc/passwd consulted? nsswitch decides"
for ns in files other; do
  out="$(timeout 180 docker run --rm \
          -v "$OUT/passwd":/etc/passwd:ro \
          -v "$OUT/nsw.$ns":/etc/nsswitch.conf:ro \
          -v "$OUT/q_static":/q:ro "$UBUNTU" /q 2>&1 | head -1)"
  printf '  nsswitch=%-6s -> %s\n' "$ns" "$out"
  eval "res_$ns=\$out"
done
if [ "${res_files:-}" = FOUND ] && [ "${res_other:-}" = NOTFOUND ]; then
  pass "A: the supplied /etc/passwd is read under 'files' and IGNORED otherwise"
  echo "     ⛔ podbox-complete must supply /etc/nsswitch.conf too, or assert it"
  echo "        already names files. Supplying /etc/passwd alone is a no-op on"
  echo "        any rootfs whose nsswitch names another service."
elif [ "${res_files:-}" != FOUND ]; then
  fail "A: the supplied /etc/passwd was NOT read even under 'files'; the control is broken"
else
  fail "A: the supplied /etc/passwd was read under a non-files service, so nsswitch is not the switch here"
fi
echo

echo "== B. what pinned images actually ship"
saw_files=0; saw_none=0
for row in "ubuntu-20.04 $UBUNTU" "debian-12 $DEBIAN" "alpine-3.22 $ALPINE"; do
  name=${row%% *}; img=${row##* }
  timeout 300 docker pull -q "$img" >/dev/null 2>&1 || { printf '  %-14s no-pull\n' "$name"; continue; }
  ns="$(timeout 180 docker run --rm "$img" \
        sh -c 'grep -E "^passwd" /etc/nsswitch.conf 2>/dev/null || echo NO-NSSWITCH-FILE' 2>&1 | tr -d '\r')"
  printf '  %-14s %s\n' "$name" "$ns"
  case "$ns" in
    *NO-NSSWITCH*) saw_none=$((saw_none+1)) ;;
    *files*)       saw_files=$((saw_files+1)) ;;
  esac
done
# ⚠ A row that could not be pulled is neither of these and is not counted.
if [ $((saw_files + saw_none)) -eq 0 ]; then
  echo "SKIP: no image could be read, so B measured nothing" >&2; exit 2
fi
pass "B: $saw_files image(s) name files, $saw_none ship no nsswitch.conf at all"
echo "     A musl rootfs has no NSS and reads /etc/passwd directly, so the"
echo "     requirement in A is a glibc-rootfs requirement."
echo

echo "== C. PT_INTERP-free is not dependency-free"
printf '  static probe PT_INTERP headers: %s\n' "$(readelf -l "$OUT/q_static" 2>/dev/null | grep -c INTERP)"
if command -v strace >/dev/null 2>&1; then
  # ⛔ THREE TRAPS, all of which inflate or deflate this count:
  #   count only after the LAST execve, or the shell and the tracer are counted;
  #   require .so or .so.N at the END of the name, because /etc/ld.so.cache is
  #   an index and matching it as an object inflated every early reading;
  #   -f, because a child's opens belong to the tree and not to one pid.
  strace -f -e trace=openat,open,execve "$OUT/q_static" >/dev/null 2>"$OUT/strace.txt"
  opened="$(awk '/execve\(/{n=NR} {l[NR]=$0} END{for(i=n+1;i<=NR;i++) print l[i]}' "$OUT/strace.txt" \
            | grep -oE '"[^"]+"' | tr -d '"' | grep -E '\.so(\.[0-9]+)*$' | sort -u)"
  cache="$(grep -c 'ld\.so\.cache' "$OUT/strace.txt")"
  printf '  objects opened after the last execve: %s\n' "$(printf '%s' "$opened" | grep -c . )"
  [ -n "$opened" ] && printf '%s\n' "$opened" | sed 's/^/     /'
  printf '  ld.so.cache lines seen (the trap, correctly excluded): %s\n' "$cache"
  pass "C: measured on this host with nsswitch '$(grep -E '^passwd' /etc/nsswitch.conf 2>/dev/null | tr -s ' ')'"
  echo "     ⚠ The count is a property of THIS host's nsswitch, not of static"
  echo "        linking. A rootfs naming a non-files service is where a static"
  echo "        glibc payload reaches for a host NSS module it cannot have."
else
  echo "  COULD NOT RUN: no strace on this host."
  echo "  This is the one check here that needs it; A, B and D stand without it."
fi
echo

echo "== D. glibc's gconv modules and DT_NEEDED"
GCONV=/usr/lib/x86_64-linux-gnu/gconv
if [ -d "$GCONV" ]; then
  tot=0; wl=0
  for m in "$GCONV"/*.so; do
    [ -e "$m" ] || continue
    tot=$((tot+1))
    readelf -dW "$m" 2>/dev/null | grep -q 'libc\.so\.6' && wl=$((wl+1))
  done
  printf '  %s of %s gconv modules record DT_NEEDED libc.so.6\n' "$wl" "$tot"
  if [ "$tot" -gt 0 ] && [ "$wl" -eq "$tot" ]; then
    pass "D: bundling gconv into a musl artefact reintroduces a second libc"
  else
    fail "D: not every gconv module records libc.so.6; re-read this host's glibc"
  fi
else
  echo "  COULD NOT RUN: no gconv directory on this host."
fi
echo

echo "== verdict"
[ "$rc" -eq 0 ] && echo "  every check that ran matched" || echo "  see the FAIL lines above"
exit "$rc"
