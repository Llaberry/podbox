#!/usr/bin/env bash
# Question: which interposer object may be loaded into a given payload, and can
# that be decided from ELF metadata alone, before anything is run?
#
# This is the experiment that retires `60-interposer-libc.sh`'s question B.
# 60- asks whether a musl-linked preload object loads into a glibc payload and
# exits 2 on a host with no musl. That question has an answer (check B below,
# measured), and the answer makes it the wrong question: the object that may be
# loaded is decided by two ELF facts a reader can take without running
# anything, so podbox selects rather than tries.
#
# Four checks:
#
#   A. the SONAME discriminator. musl's libc has no SONAME and an object linked
#      against it records `libc.so` in DT_NEEDED; glibc's SONAME is `libc.so.6`.
#      On a glibc host `libc.so` is a GNU ld script, so the musl object's own
#      DT_NEEDED names a text file.
#   B. the four cross-libc arms, both controls included.
#   C. the version predicate. The highest `GLIBC_x.y` an object imports, against
#      the highest a payload libc declares. Predicted here, then confirmed by
#      running it, so the check is measured against the loader rather than
#      believed.
#   D. the `.dynsym` trap. A shipped libc.so.6 is stripped: a reader that asks
#      `.symtab` finds no symbols at all and concludes the payload's libc
#      defines nothing, which refuses every artefact and reads exactly like a
#      check that works.
#
# Inputs pinned: the two images by manifest digest below, this host's own
# toolchain, and `references/VHSgunzo__pathmap/tree/path-mapping.c` at the
# commit `TODO/reference-map.md` records. pathmap is the subject because it is
# the mechanism podbox adopts and because it builds against either libc with no
# unwinder, which podbox's own Rust cdylib does not (check A2 of 60-).
#
# Exit: 0 every check this host can run ran and matched, 1 a check ran and did
#       not match, 2 a check could not run here.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
OUT="${OUT:-$HERE/.abi}"
rm -rf "$OUT"; mkdir -p "$OUT"

SRC="$ROOT/references/VHSgunzo__pathmap/tree/path-mapping.c"
GLIBC_PAYLOAD='ubuntu@sha256:c664f8f86ed5a386b0a340d981b8f81714e21a8b9c73f658c4bea56aa179d54a'
MUSL_PAYLOAD='alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
_ldd="$(ldd --version 2>&1)"
printf 'host libc         %s\n' "${_ldd%%$'\n'*}"
printf 'cc                %s\n' "$(cc --version 2>/dev/null | head -1)"
printf 'musl-gcc          %s\n' "$(command -v musl-gcc || echo absent)"
_dv="$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
printf 'docker            %s\n' "${_dv:-MISSING}"
printf 'glibc payload     %s\n' "$GLIBC_PAYLOAD"
printf 'musl payload      %s\n' "$MUSL_PAYLOAD"
printf 'subject           %s\n' "references/VHSgunzo__pathmap/tree/path-mapping.c"
echo

[ -r "$SRC" ] || { echo "SKIP: $SRC is absent" >&2; exit 2; }
command -v readelf >/dev/null 2>&1 || { echo "SKIP: no readelf" >&2; exit 2; }
command -v cc      >/dev/null 2>&1 || { echo "SKIP: no cc" >&2; exit 2; }

rc=0
fail() { printf 'FAIL %s\n' "$1"; rc=1; }
pass() { printf 'ok   %s\n' "$1"; }

needed() { readelf -dW "$1" 2>/dev/null | awk -F'[][]' '/NEEDED/{print $2}' | tr '\n' ' '; }
maxver() { readelf -sW --dyn-syms "$1" 2>/dev/null | grep -oE 'GLIBC_[0-9.]+' | sort -uV | tail -1; }

echo "== build the subject against each libc"
GNU_SO="$OUT/pathmap-glibc.so"
MUSL_SO="$OUT/pathmap-musl.so"
if cc -shared -fPIC -O1 -o "$GNU_SO" "$SRC" 2>"$OUT/build-glibc.log"; then
  printf '  glibc object  %8s bytes  NEEDED: %s\n' "$(stat -c%s "$GNU_SO")" "$(needed "$GNU_SO")"
else
  echo "SKIP: the subject does not build against this host's libc; see $OUT/build-glibc.log" >&2
  exit 2
fi
HAVE_MUSL=0
if command -v musl-gcc >/dev/null 2>&1 \
   && musl-gcc -shared -fPIC -O1 -o "$MUSL_SO" "$SRC" 2>"$OUT/build-musl.log"; then
  HAVE_MUSL=1
  printf '  musl object   %8s bytes  NEEDED: %s\n' "$(stat -c%s "$MUSL_SO")" "$(needed "$MUSL_SO")"
else
  printf '  musl object   NOT BUILT (musl-gcc absent, or the link failed)\n'
fi
echo

echo "== A. the SONAME discriminator, taken with no run"
# ⭐ THIS IS THE MECHANISM podbox selects on. Both facts are one readelf away.
printf '  glibc libc SONAME   %s\n' \
  "$(readelf -dW /lib/x86_64-linux-gnu/libc.so.6 2>/dev/null | awk -F'[][]' '/SONAME/{print $2}')"
if [ -e /lib/x86_64-linux-musl/libc.so ]; then
  _ms="$(readelf -dW /lib/x86_64-linux-musl/libc.so 2>/dev/null | awk -F'[][]' '/SONAME/{print $2}')"
  printf '  musl libc SONAME    %s\n' "${_ms:-(none declared)}"
fi
# ⚠ On a glibc host `libc.so` is not an object. This is why B's failure is an
# ELF-header error and not a symbol error, and why no symbol table is consulted.
if [ -e /lib/x86_64-linux-gnu/libc.so ]; then
  printf '  /lib/x86_64-linux-gnu/libc.so is: %s\n' "$(file -b /lib/x86_64-linux-gnu/libc.so)"
fi
g_needed="$(needed "$GNU_SO")"
case "$g_needed" in
  *libc.so.6*) pass "A: the glibc object records libc.so.6, so DT_NEEDED names the libc it wants" ;;
  *)           fail "A: the glibc object records '$g_needed', not libc.so.6" ;;
esac
if [ "$HAVE_MUSL" = 1 ]; then
  m_needed="$(needed "$MUSL_SO")"
  case "$m_needed" in
    *libc.so.6*) fail "A: the musl object records libc.so.6, so it is not musl-linked" ;;
    *libc.so*)   pass "A: the musl object records libc.so, which no glibc host serves as an object" ;;
    *)           fail "A: the musl object records '$m_needed', which is neither shape" ;;
  esac
fi
echo

echo "== B. the four cross-libc arms"
command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 || {
  echo "SKIP: no docker daemon, so three of the four arms cannot run" >&2; exit 2; }
arm() { # arm LABEL IMAGE SO EXPECT
  local label="$1" img="$2" so="$3" expect="$4" out r
  if [ "$img" = "HOST" ]; then
    out="$(LD_PRELOAD="$so" /usr/bin/env true 2>&1)"; r=$?
  else
    out="$(timeout 180 docker run --rm -v "$so":/i.so:ro "$img" \
             sh -c 'LD_PRELOAD=/i.so /bin/true' 2>&1)"; r=$?
  fi
  printf '  %-34s rc=%-4s %s\n' "$label" "$r" "$(printf '%s' "$out" | head -1)"
  case "$expect" in
    loads)   [ "$r" -eq 0 ] || fail "B: $label was expected to load and did not" ;;
    refused) [ "$r" -ne 0 ] || fail "B: $label was expected to be refused and loaded" ;;
  esac
}
# ⛔ THE CONTROLS COME FIRST. A cross-libc object that does not load proves
# nothing about the libc unless a matching-libc object does load.
arm "control glibc object -> glibc"  HOST            "$GNU_SO"  loads
arm "glibc object -> musl payload"   "$MUSL_PAYLOAD" "$GNU_SO"  refused
if [ "$HAVE_MUSL" = 1 ]; then
  arm "control musl object -> musl"    "$MUSL_PAYLOAD"  "$MUSL_SO" loads
  arm "QUESTION B musl object -> glibc" HOST            "$MUSL_SO" refused
else
  echo "  QUESTION B                       COULD NOT RUN: no musl-gcc on this host"
  echo "  Install musl-tools, or run this script in an alpine container."
fi
echo

echo "== C. the version predicate: predict from ELF, then confirm with the loader"
obj_max="$(maxver "$GNU_SO")"
payload_max="$(timeout 180 docker run --rm "$GLIBC_PAYLOAD" \
  sh -c 'readelf -V /lib/x86_64-linux-gnu/libc.so.6 2>/dev/null | grep -oE "GLIBC_[0-9.]+" | sort -uV | tail -1' 2>/dev/null)"
if [ -z "$payload_max" ]; then
  # ⚠ A minimal image has no readelf. Fall back to the payload's own ldd banner,
  # which names the release rather than the highest declared version symbol.
  payload_max="GLIBC_$(timeout 180 docker run --rm "$GLIBC_PAYLOAD" \
    sh -c 'ldd --version 2>&1 | head -1 | grep -oE "[0-9]+\.[0-9]+$"' 2>/dev/null)"
fi
printf '  the object imports up to   %s\n' "${obj_max:-none}"
printf '  the payload libc declares  %s\n' "${payload_max:-unknown}"
predict=loads
if [ -n "$obj_max" ] && [ -n "$payload_max" ]; then
  hi="$(printf '%s\n%s\n' "$obj_max" "$payload_max" | sort -uV | tail -1)"
  [ "$hi" = "$obj_max" ] && [ "$obj_max" != "$payload_max" ] && predict=refused
fi
printf '  PREDICTION from ELF alone: %s\n' "$predict"
c_out="$(timeout 180 docker run --rm -v "$GNU_SO":/i.so:ro "$GLIBC_PAYLOAD" \
          sh -c 'LD_PRELOAD=/i.so /bin/true' 2>&1)"; c_rc=$?
printf '  OBSERVED                 : rc=%s %s\n' "$c_rc" "$(printf '%s' "$c_out" | head -1)"
observed=loads; [ "$c_rc" -ne 0 ] && observed=refused
if [ "$predict" = "$observed" ]; then
  if [ "$observed" = refused ]; then
    # The prediction is only worth having if the loader names the same version.
    case "$c_out" in
      *"$obj_max"*) pass "C: the prediction matched and the loader named $obj_max" ;;
      *) fail "C: the prediction matched but the loader named a different version than $obj_max" ;;
    esac
  else
    pass "C: the prediction matched: nothing the object imports is newer than the payload declares"
  fi
else
  fail "C: predicted $predict and observed $observed"
fi
echo

echo "== D. the .dynsym trap"
# ⭐ A reader that asks .symtab about a stripped libc concludes it defines
# nothing, refuses every artefact, and reads exactly like a check that works.
LIBC=/lib/x86_64-linux-gnu/libc.so.6
sym=$(nm --defined-only "$LIBC" 2>/dev/null | grep -c . )
dyn=$(nm -D --defined-only "$LIBC" 2>/dev/null | grep -c . )
printf '  .symtab defined symbols  %s\n' "$sym"
printf '  .dynsym defined symbols  %s\n' "$dyn"
if [ "$sym" -eq 0 ] && [ "$dyn" -gt 0 ]; then
  pass "D: this libc is stripped, so an ABI check must read .dynsym"
elif [ "$sym" -gt 0 ]; then
  echo "  NOTE: this host's libc is not stripped, so the trap is not reproducible here."
  echo "  It stays a requirement: a payload image's libc usually is stripped."
else
  fail "D: neither table has symbols, so this reading is not interpretable"
fi
echo

echo "== verdict"
if [ "$rc" -eq 0 ]; then
  echo "  every check that ran matched."
  echo "  podbox selects the interposer by DT_NEEDED and refuses on the version"
  echo "  predicate, both read from ELF. TODO/interpose.md T-0702 carries this."
else
  echo "  see the FAIL lines above"
fi
[ "$HAVE_MUSL" = 1 ] || { echo "  question B could not run here; exiting 2."; exit 2; }
exit "$rc"
