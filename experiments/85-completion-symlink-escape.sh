#!/usr/bin/env bash
# Question: can a symlink inside an image make podbox's own completion layer
# write OUTSIDE the rootfs, onto the host?
#
# TODO/complete.md T-0405. `/etc/mtab` is a symlink in most images, and
# `printf ... > "$R/etc/mtab"` follows it. If the target is absolute the write
# lands on the host. The same shape is `/etc/resolv.conf ->
# ../run/systemd/resolve/stub-resolv.conf`, and the next one nobody has met.
#
# ⭐ WHY THIS IS A MEASUREMENT AND NOT A UNIT TEST. The unit tests in
# `crates/podbox-complete/src/write.rs` drive one function. This drives the
# SHIPPED BINARY through the path a real container start takes: a real image,
# extracted into a real store, tampered with the way an image could ship, and
# then run. A containment check that is right in the library and not reached by
# `podbox run` is a check nobody has.
#
# ⛔ THE ASSERTION IS ON THE FILESYSTEM, NOT ON AN EXIT CODE. A canary file
# outside the rootfs is written before the run and its bytes are compared after
# it. `podbox run` exiting 0 says nothing about where its writes landed, which
# is the mistake `experiments/220-extract-path-safety.sh` was built to avoid on
# the extraction side.
#
# Six doors. Five must not be walked through, and the sixth MUST be:
#
#   A  /etc/mtab        -> an absolute path outside the rootfs        (T-0405)
#   B  /etc/resolv.conf -> an absolute path outside the rootfs        (T-0402)
#   C  /etc/hosts       -> an absolute path outside the rootfs        (T-0403)
#   E  /etc/passwd      -> an absolute path outside the rootfs        (T-0404)
#   D  /etc/ssl/certs   -> an absolute DIRECTORY outside the rootfs   (T-0407)
#   F  /etc/apt         -> ../var/aptreal, INSIDE the rootfs          (T-0407)
#
# ⭐ F IS THE ONE THAT MAKES THE OTHER FIVE MEAN SOMETHING. A completion layer
# that refuses every symlink escapes nothing and also cannot write the file the
# payload reads: `/etc/ssl/certs -> /var/lib/ca-certificates/pem` on openSUSE and
# `/lib -> usr/lib` on void are the rootfs's own links, and refusing them made
# the CA fixup fail and the libc probe read `unknown` on 2026-09-09. So the
# directory components resolve under `RESOLVE_IN_ROOT`: a link is followed, and
# the rootfs is `/` while it is, so D lands INSIDE at the rebased path and F
# lands where the image put it.
#
# ⚠ ghcr.io and public.ecr.aws only: Docker Hub's rate limit is attributed to a
# shared address here and tripping it reads as a broken registry.
#
#   ./85-completion-symlink-escape.sh
#
# Exit: 0 nothing escaped, 1 something did, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/completion-symlink-escape.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

IMAGE="${PODBOX_COMPLETE_IMAGE:-public.ecr.aws/docker/library/alpine:3.20}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. ./scripts/dev.sh build" >&2
	exit 2
}

export PODBOX_STORE="$WORK/store"
fail=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'image             %s\n' "$IMAGE"
	echo
} >"$WORK/report"

# ------------------------------------------------------------------ the setup
timeout 600 "$BIN" pull "$IMAGE" >"$WORK/pull.log" 2>&1 || {
	echo "SKIP: could not pull $IMAGE" >&2
	sed 's/^/  /' "$WORK/pull.log" >&2
	exit 2
}
ROOTFS="$(timeout 300 "$BIN" extract "$IMAGE" 2>>"$WORK/pull.log" | tail -1)"
[ -d "$ROOTFS" ] || {
	echo "SKIP: podbox extract did not print a rootfs directory" >&2
	sed 's/^/  /' "$WORK/pull.log" >&2
	exit 2
}
# ⛔ A TRACKED READING CARRIES NO PER-RUN VALUE. `$WORK` is a fresh mktemp
# directory every run, so the rootfs path and every canary path in it would make
# this file differ from itself on every run and `result-diff.sh` useless. They
# are redacted to a fixed token; what is being measured is which side of the
# rootfs a write landed on, not where the rootfs was.
redact() { sed "s#$WORK#<WORK>#g"; }
say "rootfs            $(printf '%s' "$ROOTFS" | redact)"

# ⛔ The canaries live OUTSIDE the store, so a run that removed the store would
# not also remove the evidence.
CANARY="$WORK/canary"
mkdir -p "$CANARY/dir"
for f in mtab resolv hosts passwd; do
	printf 'CANARY-%s-UNTOUCHED\n' "$f" >"$CANARY/$f"
done
printf 'CANARY-DIRFILE-UNTOUCHED\n' >"$CANARY/dir/ca-certificates.crt"

# The six doors, planted the way an image could ship them.
rm -f "$ROOTFS/etc/mtab" "$ROOTFS/etc/resolv.conf" "$ROOTFS/etc/hosts" "$ROOTFS/etc/passwd"
ln -s "$CANARY/mtab" "$ROOTFS/etc/mtab"
ln -s "$CANARY/resolv" "$ROOTFS/etc/resolv.conf"
ln -s "$CANARY/hosts" "$ROOTFS/etc/hosts"
ln -s "$CANARY/passwd" "$ROOTFS/etc/passwd"
# D: the DIRECTORY the CA bundle goes into, pointed outside.
rm -rf "$ROOTFS/etc/ssl"
mkdir -p "$ROOTFS/etc/ssl"
ln -s "$CANARY/dir" "$ROOTFS/etc/ssl/certs"
# F: openSUSE's own shape -- a directory link that stays inside, which podbox
# MUST follow or it writes the fixup somewhere the payload does not read.
rm -rf "$ROOTFS/etc/apt" "$ROOTFS/var/aptreal"
mkdir -p "$ROOTFS/var/aptreal/apt.conf.d"
ln -s /var/aptreal "$ROOTFS/etc/apt"
say "planted           5 symlinks out of the rootfs, and 1 legitimate one inside"
say ""

# ------------------------------------------------------------------ the run
# ⚠ NOT --rm: the tree has to survive so it can be looked at afterwards.
timeout 300 "$BIN" run "$IMAGE" /bin/true >"$WORK/run.out" 2>"$WORK/run.err"
run_rc=$?
say "== the run"
say "  exit              $run_rc"
say "  fixups reported   $(grep -c 'podbox: complete:' "$WORK/run.err")"
grep 'podbox: complete:' "$WORK/run.err" | redact | sed 's/^/    /' | cut -c1-150 >>"$WORK/report"
say ""

# ------------------------------------------------------------------ the check
say "== the canaries, read off the filesystem"
check() { # check <name> <path> <expected-first-line>
	local name="$1" path="$2" want="$3" got
	got="$(head -1 "$path" 2>/dev/null)"
	if [ "$got" = "$want" ]; then
		say "  $name: intact"
	else
		say "  $name: ⛔ ESCAPED. it now reads: $(printf '%s' "$got" | redact | cut -c1-60)"
		fail=1
	fi
}
check "A /etc/mtab           " "$CANARY/mtab" "CANARY-mtab-UNTOUCHED"
check "B /etc/resolv.conf    " "$CANARY/resolv" "CANARY-resolv-UNTOUCHED"
check "C /etc/hosts          " "$CANARY/hosts" "CANARY-hosts-UNTOUCHED"
check "E /etc/passwd         " "$CANARY/passwd" "CANARY-passwd-UNTOUCHED"
check "D a directory link out" "$CANARY/dir/ca-certificates.crt" "CANARY-DIRFILE-UNTOUCHED"

say ""
say "== and the writes landed INSIDE"
# ⛔ The other half. A completion layer that escapes nothing because it writes
# nothing is not the property being measured.
for pair in "etc/mtab: /" "etc/resolv.conf:nameserver" "etc/hosts:localhost"; do
	rel="${pair%%:*}"
	needle="${pair#*:}"
	if [ -L "$ROOTFS/$rel" ]; then
		say "  $rel is STILL a symlink: podbox did not replace it"
		fail=1
	elif grep -qF -- "$needle" "$ROOTFS/$rel" 2>/dev/null; then
		say "  $rel is a regular file inside the rootfs and carries what podbox wrote"
	else
		say "  $rel: ⛔ podbox wrote nothing readable here"
		fail=1
	fi
done
# ⚠ `/etc/passwd` is REPLACED rather than written through, and the image's own
# users are gone with the link, so what is checked is that a real file is there
# and root is in it.
if [ -L "$ROOTFS/etc/passwd" ]; then
	say "  etc/passwd is STILL a symlink"
	fail=1
elif grep -q '^root:' "$ROOTFS/etc/passwd" 2>/dev/null; then
	say "  etc/passwd is a regular file inside the rootfs and names root"
else
	say "  etc/passwd: ⛔ nothing usable was written"
	fail=1
fi

say ""
say "== F: a legitimate INTERNAL directory link is followed"
# ⭐ The other half, and without it every assertion above is satisfied by a
# completion layer that writes nothing at all.
if [ -f "$ROOTFS/var/aptreal/apt.conf.d/99podbox" ]; then
	say "  etc/apt -> /var/aptreal was followed, and the drop-in is at its target"
else
	say "  ⛔ etc/apt -> /var/aptreal was NOT followed: the fixup did not reach"
	say "     the directory the payload reads. That is openSUSE's own shape."
	fail=1
fi
# ⚠ D is a DANGLING link once it is rebased: `/tmp/.../canary/dir` does not exist
# inside the rootfs. Two outcomes are correct and a third is not. Either podbox
# creates the rebased directory and writes there, or it refuses -- and a refusal
# has to be REPORTED, because a fixup that quietly did nothing is the sandlock
# failure TODO/cli.md T-0804 names. What is never correct is the write landing
# outside, which the canary above already covers.
if [ -s "$ROOTFS$CANARY/dir/ca-certificates.crt" ]; then
	say "  D: the write landed INSIDE the rootfs, at the rebased path"
elif grep -q 'complete: failed' "$WORK/run.err"; then
	say "  D: refused, and the refusal is on the banner:"
	grep 'complete: failed' "$WORK/run.err" | redact | sed 's/^/       /' | cut -c1-140 >>"$WORK/report"
else
	say "  D: ⛔ podbox neither wrote it nor said it could not. A fixup that"
	say "     quietly did nothing is what T-0804 exists to refuse."
	fail=1
fi

say ""
say "== verdict"
if [ "$fail" -eq 0 ]; then
	say "  6 doors, 0 escapes; the five that leave were confined and the one"
	say "  that stays inside was followed."
else
	say "  ⛔ SOMETHING LEFT THE ROOTFS. The canary above says which door."
fi

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

exit "$fail"
