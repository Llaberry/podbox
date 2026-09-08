#!/usr/bin/env bash
# Question: what does the OCI layer contract actually require of podbox's
# extractor (TOOL.md section 6.3), measured against real images rather than read off a
# specification?
#
# Four facts, each one a requirement on `podbox-extract`:
#
#   A. member names carry no `./` prefix, so a path glob anchored on a slash
#      cannot see a whiteout at the root of a layer
#   B. `alpine`'s `etc/shadow` is gid 42, which is the section 9.1 ownership wall
#   C. `voidlinux-musl` ships `var/cache/xbps` as an ABSOLUTE self-referential
#      symlink in one layer and whiteouts it in the next, which is M2's
#      acceptance case
#   D. udocker's whiteout selector, `tar t --wildcards -f LAYER '*/.wh.*'`
#      (references/indigo-dc__udocker/tree/udocker/container/structure.py:242
#      at commit 638bc42), is compared against a basename oracle on a crafted
#      archive that has one whiteout at the layer root
#
# ⭐ D is measured against a CRAFTED archive with a known answer, not against
# the two real images: neither of them happens to carry a root-level whiteout,
# so passing on them would say nothing.
#
# Inputs pinned by digest:
#   alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
#   voidlinux/voidlinux-musl@sha256:d5c970d0015c3aa2559a2b5a87b158839969fbb1941a9ef52cc484a5392554cd
#
# Exit: 0 every check ran and matched, 1 a check ran and did not match,
#       2 could not run (no docker, no tar, no network).
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
OUT="${OUT:-$HERE/.whiteout}"
rm -rf "$OUT"; mkdir -p "$OUT"

ALPINE='alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
VOID='voidlinux/voidlinux-musl@sha256:d5c970d0015c3aa2559a2b5a87b158839969fbb1941a9ef52cc484a5392554cd'

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'tar               %s\n' "$(tar --version 2>/dev/null | head -1)"
_dv="$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
printf 'docker            %s\n' "${_dv:-MISSING}"
printf 'alpine            %s\n' "$ALPINE"
printf 'void              %s\n' "$VOID"
echo

command -v tar >/dev/null 2>&1 || { echo "SKIP: no tar" >&2; exit 2; }
command -v docker >/dev/null 2>&1 || { echo "SKIP: no docker" >&2; exit 2; }
docker info >/dev/null 2>&1 || { echo "SKIP: no docker daemon; start dockerd" >&2; exit 2; }

rc=0
fail() { printf 'FAIL %s\n' "$1"; rc=1; }
pass() { printf 'ok   %s\n' "$1"; }

# `tar tzf` on an uncompressed layer fails, and `tar tf` on a gzipped one
# succeeds only with GNU tar's auto-detection. Try compressed, then plain.
list() { tar tzf "$1" 2>/dev/null || tar tf "$1" 2>/dev/null; }
listv() { tar tvzf "$1" 2>/dev/null || tar tvf "$1" 2>/dev/null; }

save() { # save IMAGE DIR
  local img="$1" dir="$2"
  mkdir -p "$dir"
  docker pull -q "$img" >/dev/null 2>&1 || { echo "SKIP: cannot pull $img" >&2; exit 2; }
  docker save "$img" -o "$dir/img.tar" || { echo "SKIP: cannot save $img" >&2; exit 2; }
  tar xf "$dir/img.tar" -C "$dir" || { echo "SKIP: cannot unpack $img" >&2; exit 2; }
}

layers() { # layers DIR -> one blob path per line, in order
  local dir="$1"
  if command -v jq >/dev/null 2>&1; then
    jq -r '.[0].Layers[]' "$dir/manifest.json"
  else
    grep -o 'blobs/sha256/[0-9a-f]\{64\}' "$dir/manifest.json" | tail -n +2
  fi
}

echo "== A. member-name prefix style, per layer"
save "$ALPINE" "$OUT/alpine"
save "$VOID"   "$OUT/void"
for d in alpine void; do
  for l in $(layers "$OUT/$d"); do
    first="$(list "$OUT/$d/$l" | head -1)"
    printf '  %-7s %s  first member: %s\n' "$d" "${l##*/}" "$first"
    case "$first" in
      ./*) fail "A: $d layer ${l##*/} DOES use a ./ prefix; A's premise is wrong here" ;;
    esac
  done
done
case "$rc" in 0) pass "A: no layer uses a ./ prefix, so a slash-anchored glob cannot see a root whiteout" ;; esac
echo

echo "== B. alpine etc/shadow ownership"
shadow=""
for l in $(layers "$OUT/alpine"); do
  s="$(listv "$OUT/alpine/$l" | grep -E ' etc/shadow$' | head -1)"
  [ -n "$s" ] && shadow="$s"
done
printf '  %s\n' "${shadow:-(not found)}"
case "$shadow" in
  *' 0/42 '*) pass "B: etc/shadow is uid 0 gid 42, the section 9.1 wall" ;;
  '')         fail "B: etc/shadow not found in any alpine layer" ;;
  *)          fail "B: etc/shadow is not 0/42; section 9.1's usual first casualty has moved" ;;
esac
echo

echo "== C. voidlinux var/cache/xbps"
sym=""; wh=""
n=0
for l in $(layers "$OUT/void"); do
  n=$((n+1))
  s="$(listv "$OUT/void/$l" | grep -E ' var/cache/xbps( ->|$)' | head -1)"
  [ -n "$s" ] && { sym="$s"; sym_layer=$n; }
  w="$(list "$OUT/void/$l" | grep -E '(^|/)\.wh\.xbps$' | head -1)"
  [ -n "$w" ] && { wh="$w"; wh_layer=$n; }
done
printf '  layer %s symlink:  %s\n' "${sym_layer:--}" "${sym:-(not found)}"
printf '  layer %s whiteout: %s\n' "${wh_layer:--}" "${wh:-(not found)}"
if [ -n "$sym" ] && [ -n "$wh" ] && [ "${wh_layer:-0}" -gt "${sym_layer:-0}" ]; then
  case "$sym" in
    *'-> /var/cache/xbps'*)
      pass "C: an ABSOLUTE self-referential symlink in layer $sym_layer, whiteouted in layer $wh_layer" ;;
    *)  fail "C: the symlink target is not the absolute self-reference M2 expects" ;;
  esac
else
  fail "C: the symlink and its later whiteout were not both found"
fi
echo

echo "== D. udocker's whiteout selector against a basename oracle"
# ⭐ The fixture is crafted, committed by this script on every run, and has a
# known answer: two whiteouts, one at the layer root, written WITHOUT a ./
# prefix so it matches the naming style A just measured.
F="$OUT/fixture"; mkdir -p "$F/dir"
: > "$F/.wh.rootfile"; : > "$F/dir/.wh.nested"; : > "$F/normal.txt"
( cd "$F" && tar cf "$OUT/fixture.tar" .wh.rootfile dir/.wh.nested normal.txt )
printf '  members: %s\n' "$(tar tf "$OUT/fixture.tar" | tr '\n' ' ')"

oracle="$(tar tf "$OUT/fixture.tar" | awk -F/ '$NF ~ /^\.wh\./' | sort)"
udocker="$(tar t --wildcards -f "$OUT/fixture.tar" '*/.wh.*' 2>/dev/null | sort)"
printf '  basename oracle: %s\n' "$(printf '%s' "$oracle" | tr '\n' ' ')"
printf '  udocker glob:    %s\n' "$(printf '%s' "$udocker" | tr '\n' ' ')"
missed="$(comm -23 <(printf '%s\n' "$oracle") <(printf '%s\n' "$udocker"))"
if [ -n "$missed" ]; then
  printf '  MISSED by the glob: %s\n' "$(printf '%s' "$missed" | tr '\n' ' ')"
  pass "D: a slash-anchored glob misses a layer-root whiteout. podbox matches on the basename."
else
  fail "D: the glob caught every whiteout. Re-read the fixture and the tar version."
fi
echo

echo "== verdict"
[ "$rc" -eq 0 ] && echo "  every check matched" || echo "  see the FAIL lines above"
exit "$rc"
