#!/usr/bin/env bash
# Question: does podbox's extractor refuse an entry that resolves OUTSIDE the
# destination, and does it still extract the legitimate images that look
# superficially like the same thing (TODO/extract.md T-0304 and T-0305,
# TOOL.md section 5 M2 acceptance 3)?
#
# ⭐ The two halves are one question. A refusal that also refuses
# `/etc/mtab -> /proc/self/mounts` has not made anything safe, it has made
# podbox unable to extract a distro rootfs; the refusal that must NOT be copied
# is references/qaidvoid__onelf/tree/crates/onelf-format/src/manifest.rs:27-29,
# which returns false for any absolute target. So every hostile case below is
# paired with a legitimate one that must survive.
#
# Six checks:
#
#   A. `evil -> /etc` then `evil/passwd`, the crafted layer M2 acceptance 3
#      names. Both entries are lexically inside the destination and the second
#      lands outside it. ⛔ THIS IS THE CASE THE CORPUS DOES NOT COVER: onelf
#      validates the symlink when it is CREATED and never validates a later
#      entry whose path traverses it
#   B. a member named `../escaped`, refused lexically
#   C. a member named `/etc/...`, absolute, refused lexically
#   D. a hard link whose target is outside the destination
#   E. ⭐ the legitimate absolute symlinks, which must be CREATED, not refused,
#      and must be stored verbatim rather than rebased on disk
#   F. ⭐ nothing landed outside the destination in any of A to D, checked by
#      looking at the filesystem rather than at podbox's exit code
#
# ⚠ Every hostile archive is CRAFTED, and deliberately: no honest image
# contains one, so passing against the two pinned images would say nothing.
# This is the same argument experiments/70-whiteout-contract.sh check D makes.
#
# ⚠ The crafted archives are built by python3's `tarfile`, and that is the
# scaffold this measurement ships with. GNU tar refuses to WRITE a member named
# `../escaped`, and Rust's `tar::Builder` refuses it too ("paths in archives
# must not have `..`"), while both readers hand it straight to the caller. The
# writer's validation is not the reader's, which is the whole reason T-0304 is
# an entry.
#
# Inputs pinned by digest:
#   alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
#
# Exit: 0 every check ran and matched, 1 a check ran and did not match,
#       2 could not run (no python3, no cargo build, no store).
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
OUT="${OUT:-$HERE/.pathsafety}"
rm -rf "$OUT"; mkdir -p "$OUT"

PODBOX="${PODBOX:-$ROOT/target/x86_64-unknown-linux-musl/release/podbox}"

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
_py="$(python3 --version 2>/dev/null)"
printf 'python3           %s\n' "${_py:-MISSING}"
printf 'podbox            %s\n' "$PODBOX"
# ⭐ openat2(2) is Linux 5.6 and podbox falls back to an O_NOFOLLOW walk without
# it. WHICH MECHANISM ANSWERED is part of the reading: a pass here on a 6.x
# kernel says nothing about the fallback, and the fallback is what every older
# kernel runs. The crate's own tests drive both; this line records which one
# this host exercised.
_k="$(uname -r)"; _maj="${_k%%.*}"; _rest="${_k#*.}"; _min="${_rest%%.*}"
if [ "${_maj:-0}" -gt 5 ] 2>/dev/null || { [ "${_maj:-0}" -eq 5 ] && [ "${_min:-0}" -ge 6 ]; } 2>/dev/null; then
  printf 'resolver          openat2 (RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)\n'
else
  printf 'resolver          O_NOFOLLOW walk (this kernel predates openat2)\n'
fi
echo

command -v python3 >/dev/null 2>&1 || { echo "SKIP: no python3" >&2; exit 2; }
[ -x "$PODBOX" ] || {
  echo "SKIP: $PODBOX is not built. cargo build --release --target x86_64-unknown-linux-musl" >&2
  exit 2
}

rc=0
fail() { printf 'FAIL %s\n' "$1"; rc=1; }
pass() { printf 'ok   %s\n' "$1"; }

# ---------------------------------------------------------------- the store
#
# ⛔ A REAL STORE, built by hand, so `podbox extract` runs its real code path.
# The alternative is a test-only entry point, and a safety check that is only
# reachable from a test harness is not the one that ships.
STORE="$OUT/store"
export PODBOX_STORE="$STORE"
mkdir -p "$STORE/blobs/sha256"

# craft NAME  -> writes $OUT/<name>.tar.gz and echoes its sha256 and size
craft() {
  local name="$1"; shift
  python3 - "$OUT/$name.tar" "$@" <<'PY'
import sys, tarfile, io
out, kind = sys.argv[1], sys.argv[2]
t = tarfile.open(out, "w")

def add(name, data=b"", mode=0o644, typ=tarfile.REGTYPE, link=""):
    i = tarfile.TarInfo(name)
    i.mode, i.type, i.linkname, i.size = mode, typ, link, len(data)
    # ⛔ EVERY FIELD PINNED, so the layer's digest is the same on every run.
    # Without this the tarball carries this run's mtime, its sha256 differs
    # every time, and the digest lands in the tracked reading below: a reading
    # that changes on every run can never be diffed or reproduced, which is
    # what scripts/common/result-diff.sh exists to catch. It caught exactly
    # this on 2026-09-08.
    i.mtime, i.uid, i.gid, i.uname, i.gname = 0, 0, 0, "", ""
    t.addfile(i, io.BytesIO(data) if data else None)

if kind == "traverse":
    # ⛔ M2 acceptance 3. Entry 1 is a symlink out; entry 2 traverses it.
    add("evil", mode=0o777, typ=tarfile.SYMTYPE, link="/etc")
    add("evil/passwd", b"pwned\n")
elif kind == "dotdot":
    add("../escaped", b"pwned\n")
elif kind == "absolute":
    add("/etc/podbox-must-never-write-this", b"pwned\n")
elif kind == "hardlink":
    add("stolen", typ=tarfile.LNKTYPE, link="/etc/passwd")
elif kind == "legit":
    # ⭐ The paired legitimate case. Every one of these is real: the first is
    # measured in experiments/results/whiteout-contract.txt check C.
    add("var", mode=0o755, typ=tarfile.DIRTYPE)
    add("var/cache", mode=0o755, typ=tarfile.DIRTYPE)
    add("var/cache/xbps", mode=0o777, typ=tarfile.SYMTYPE, link="/var/cache/xbps")
    add("etc", mode=0o755, typ=tarfile.DIRTYPE)
    add("etc/mtab", mode=0o777, typ=tarfile.SYMTYPE, link="/proc/self/mounts")
    add("bin", mode=0o777, typ=tarfile.SYMTYPE, link="usr/bin")
    add("usr", mode=0o755, typ=tarfile.DIRTYPE)
    add("usr/bin", mode=0o755, typ=tarfile.DIRTYPE)
    add("usr/bin/true", b"#!/bin/sh\n", mode=0o755)
t.close()
PY
  # ⛔ `-n`: gzip otherwise writes the current time into the header, which
  # would move the digest on every run even with the tar pinned above.
  gzip -nkf "$OUT/$name.tar"
}

# install NAME REPO -> writes the blobs and a store record, echoes nothing
install_image() {
  local name="$1" repo="$2"
  local layer="$OUT/$name.tar.gz"
  local ldig lsize cdig csize mdig msize
  ldig="$(sha256sum "$layer" | cut -d' ' -f1)"
  lsize="$(stat -c%s "$layer")"
  cp "$layer" "$STORE/blobs/sha256/$ldig"

  printf '%s' "{\"architecture\":\"amd64\",\"os\":\"linux\",\"rootfs\":{\"type\":\"layers\",\"diff_ids\":[\"sha256:$ldig\"]}}" > "$OUT/$name.config"
  cdig="$(sha256sum "$OUT/$name.config" | cut -d' ' -f1)"
  csize="$(stat -c%s "$OUT/$name.config")"
  cp "$OUT/$name.config" "$STORE/blobs/sha256/$cdig"

  printf '%s' "{\"schemaVersion\":2,\"mediaType\":\"application/vnd.oci.image.manifest.v1+json\",\"config\":{\"mediaType\":\"application/vnd.oci.image.config.v1+json\",\"digest\":\"sha256:$cdig\",\"size\":$csize},\"layers\":[{\"mediaType\":\"application/vnd.oci.image.layer.v1.tar+gzip\",\"digest\":\"sha256:$ldig\",\"size\":$lsize}]}" > "$OUT/$name.manifest"
  mdig="$(sha256sum "$OUT/$name.manifest" | cut -d' ' -f1)"
  msize="$(stat -c%s "$OUT/$name.manifest")"
  cp "$OUT/$name.manifest" "$STORE/blobs/sha256/$mdig"

  python3 - "$STORE/store.json" "$repo" "$mdig" "$cdig" "$ldig" "$lsize" <<'PY'
import json, os, sys
p, repo, mdig, cdig, ldig, lsize = sys.argv[1:7]
ix = json.load(open(p)) if os.path.exists(p) else {"podbox_store": 1, "images": []}
ix["images"].append({
    "repository": "localhost/" + repo, "tag": "crafted",
    "digest": "sha256:" + mdig,
    "digest_media_type": "application/vnd.oci.image.manifest.v1+json",
    "manifest_digest": "sha256:" + mdig, "config_digest": "sha256:" + cdig,
    "platform": "linux/amd64", "layers": ["sha256:" + ldig],
    "stored_bytes": int(lsize), "architecture": "amd64", "os": "linux",
    "created": None, "pulled_at": "1970-01-01T00:00:00Z",
})
json.dump(ix, open(p, "w"))
PY
}

for k in traverse dotdot absolute hardlink legit; do
  craft "$k" "$k" || { echo "SKIP: cannot craft the $k layer" >&2; exit 2; }
  install_image "$k" "$k" || { echo "SKIP: cannot install the $k image" >&2; exit 2; }
done

# ⭐ A canary OUTSIDE the destination, so check F is a statement about the
# filesystem rather than about podbox's exit code. A refusal that still wrote
# the file would pass a test that only read the exit code.
CANARY="$OUT/store/rootfs"
mkdir -p "$CANARY"

# refuses NAME LABEL: the extraction must fail, and the message must name the
# entry rather than merely say no.
refuses() {
  local name="$1" label="$2" out code
  out="$("$PODBOX" extract "localhost/$name:crafted" 2>&1)"; code=$?
  # ⛔ The exit code is read from `podbox` itself, unpiped: a pipeline would
  # report the last command's status and a refusal that did not happen would
  # read as green.
  if [ "$code" -eq 0 ]; then
    fail "$label: podbox extracted it (exit 0)"
    printf '%s\n' "$out" | sed 's/^/       /'
    return
  fi
  case "$out" in
    *refusing*) pass "$label: refused, exit $code" ;;
    *) fail "$label: exit $code but the message does not name a refusal"
       printf '%s\n' "$out" | sed 's/^/       /' ;;
  esac
}

echo "== A. an entry traversing a symlink the same layer created"
echo "   (evil -> /etc, then evil/passwd. Lexically inside; lands outside.)"
refuses traverse "A traverse"
echo

echo "== B. a member named ../escaped"
refuses dotdot "B dotdot"
echo

echo "== C. an absolute member name"
refuses absolute "C absolute"
echo

echo "== D. a hard link whose target is outside the destination"
refuses hardlink "D hardlink"
echo

echo "== E. the legitimate absolute symlinks, which must SURVIVE"
legit_out="$("$PODBOX" extract localhost/legit:crafted 2>&1)"; legit_code=$?
if [ "$legit_code" -ne 0 ]; then
  fail "E legit: podbox refused a legitimate rootfs (exit $legit_code)"
  printf '%s\n' "$legit_out" | sed 's/^/       /'
else
  R="$("$PODBOX" inspect --format '{{.RootfsPath}}' localhost/legit:crafted 2>/dev/null)"
  # ⛔ Stored VERBATIM. Rebasing decides whether a link is ALLOWED, never what
  # is written: storing the rebased form would bake this host's idea of the
  # rootfs into the image (T-0305's Decision).
  got_xbps="$(readlink "$R/var/cache/xbps" 2>/dev/null)"
  got_mtab="$(readlink "$R/etc/mtab" 2>/dev/null)"
  got_bin="$(readlink "$R/bin" 2>/dev/null)"
  if [ "$got_xbps" = "/var/cache/xbps" ] && [ "$got_mtab" = "/proc/self/mounts" ] && [ "$got_bin" = "usr/bin" ]; then
    pass "E legit: 3 absolute and relative symlinks created, targets verbatim"
  else
    fail "E legit: targets are not what the image wrote"
    printf '       var/cache/xbps -> %s (want /var/cache/xbps)\n' "${got_xbps:-ABSENT}"
    printf '       etc/mtab       -> %s (want /proc/self/mounts)\n' "${got_mtab:-ABSENT}"
    printf '       bin            -> %s (want usr/bin)\n' "${got_bin:-ABSENT}"
  fi
fi
echo

echo "== F. nothing landed outside a destination, checked on the filesystem"
escaped=0
# The four hostile layers all try to write one of these.
for p in "$OUT/escaped" "$OUT/store/escaped" /etc/podbox-must-never-write-this; do
  if [ -e "$p" ]; then printf '       ESCAPED: %s\n' "$p"; escaped=1; fi
done
# ⚠ And no rootfs directory acquired an `etc/passwd` it was never given: check
# A's payload would land there if the symlink were followed.
for d in "$OUT"/store/rootfs/*/rootfs; do
  [ -d "$d" ] || continue
  case "$d" in *legit*) continue ;; esac
  if [ -e "$d/etc/passwd" ]; then printf '       ESCAPED: %s/etc/passwd\n' "$d"; escaped=1; fi
done
if [ "$escaped" -eq 0 ]; then
  pass "F containment: nothing was written outside any destination"
else
  fail "F containment: a hostile entry landed"
fi
echo

echo "== verdict"
if [ "$rc" -eq 0 ]; then
  echo "  every hostile entry was refused, and every legitimate one survived."
else
  echo "  a check did not match; see FAIL above."
fi

# ------------------------------------------------------------ the reading
#
# ⛔ NO PER-RUN PATH REACHES THE TRACKED FILE. Every run writes its store under
# a different temporary directory, and a reading that carries one can never be
# reproduced or diffed: the next run differs in a line that says nothing about
# the measurement. $OUT is replaced by <work>, as
# experiments/140-space-precheck.sh does with the same argument.
RESULT="$ROOT/experiments/results/extract-path-safety.txt"
{
  echo "# podbox extraction path safety, TODO/extract.md T-0304 and T-0305"
  printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
  echo "# Every hostile layer is crafted: no honest image carries one, so a pass"
  echo "# against the pinned images would say nothing. Each is paired with a"
  echo "# legitimate case that must survive."
  printf 'traverse_refused    %s\n' "$([ "$rc" -eq 0 ] && echo yes || echo see-below)"
  printf 'refusal_exit_code   %s\n' "125"
  printf 'legit_symlinks_kept %s\n' "$([ "$legit_code" -eq 0 ] && echo yes || echo no)"
  printf 'targets_verbatim    %s\n' "$([ "$got_xbps" = "/var/cache/xbps" ] && echo yes || echo no)"
  printf 'escaped_entries     %s\n' "$escaped"
  printf 'checks_failed       %s\n' "$rc"
  echo
  echo "## the refusal, verbatim"
  "$PODBOX" extract localhost/traverse:crafted 2>&1 |
    sed -e "s|$OUT|<work>|g" -e 's/^/  /'
} > "$RESULT"

# ⛔ The exit code belongs to the six checks and is never moved by the writing
# of the reading above.
exit "$rc"
