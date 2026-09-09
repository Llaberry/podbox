#!/usr/bin/env bash
# Question: does podbox acquire the platform it was ASKED for, and can one
# store hold two architectures of one tag at the same time?
#
# TODO/image.md T-0212. Until 2026-09-09 `oci::ARCH` was the constant `"amd64"`,
# so podbox built for aarch64 would still have pulled an amd64 rootfs, and the
# store's key was (repository, tag) so a second platform DELETED the first.
#
# ⭐ THE REGISTRY IS ghcr.io/pkgforge-dev/archlinux AND THAT IS DELIBERATE.
# It publishes one tag across eight platforms, which is what this question
# needs, and ghcr has no anonymous pull quota, so this script cannot be turned
# red by somebody else's rate limit the way TODO/image.md T-0206 describes.
#
#   ./270-multiarch-image.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/multiarch-image.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

# ⚠ Pinned by tag rather than by digest, because the question is about an INDEX
# and a digest names one manifest. The digest of what each clause resolved to is
# printed, so a re-run against a moved tag is visible rather than silent.
IMAGE="${PODBOX_MULTIARCH_IMAGE:-ghcr.io/pkgforge-dev/archlinux:latest}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
command -v file >/dev/null 2>&1 || { echo "SKIP: file(1) is not on PATH" >&2; exit 2; }

export PODBOX_STORE="$WORK/store"
# ⛔ Unset, so a caller's default cannot decide what this measures.
unset PODBOX_DEFAULT_PLATFORM DOCKER_DEFAULT_PLATFORM

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'host arch         %s\n' "$(uname -m)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'image             %s\n' "$IMAGE"
	printf 'store             a fresh directory under %s\n' "$(dirname "$WORK")"
	echo
} >"$WORK/report"

fail=0
skipped=0

# --------------------------------------------------------------------- 1
echo "== 1. no --platform pulls the platform podbox was BUILT for" >>"$WORK/report"
if ! timeout 900 "$BIN" pull "$IMAGE" >"$WORK/p1.out" 2>"$WORK/p1.err"; then
	printf '  SKIP: the pull did not complete: %s\n' "$(tail -1 "$WORK/p1.err")" >>"$WORK/report"
	cat "$WORK/report"
	exit 2
fi
host_plat="$("$BIN" images --format '{{.Platform}}' "$IMAGE" 2>/dev/null | head -1)"
printf '  asked for         nothing\n' >>"$WORK/report"
printf '  got               %s\n' "$host_plat" >>"$WORK/report"
case "$(uname -m)" in
x86_64) expect=linux/amd64 ;;
aarch64) expect=linux/arm64 ;;
*) expect="" ;;
esac
if [ -n "$expect" ] && [ "$host_plat" != "$expect" ]; then
	printf '  FAIL: this host is %s, so the default should be %s\n' "$(uname -m)" "$expect" >>"$WORK/report"
	fail=1
fi

# --------------------------------------------------------------------- 2
echo >>"$WORK/report"
echo "== 2. --platform takes a DIFFERENT manifest out of the same index" >>"$WORK/report"
# ⚠ The foreign platform is chosen to be the one this host is not.
foreign=linux/arm64
[ "$host_plat" = "linux/arm64" ] && foreign=linux/amd64
if ! timeout 900 "$BIN" pull --platform "$foreign" "$IMAGE" >"$WORK/p2.out" 2>"$WORK/p2.err"; then
	printf '  SKIP: the %s pull did not complete: %s\n' "$foreign" "$(tail -1 "$WORK/p2.err")" >>"$WORK/report"
	skipped=1
else
	# ⭐ The tag's digest is the INDEX digest and is the same for both, which is
	# the parity T-0202 is accepted on. What differs is the image ID, which is
	# the config digest of the platform-specific manifest.
	printf '  index digest      %s\n' "$(sed -n 's/^Digest: //p' "$WORK/p2.out")" >>"$WORK/report"
	mapfile -t rows < <("$BIN" images --format '{{.ID}} {{.Platform}}' "$IMAGE" 2>/dev/null | sort -k2)
	printf '  the store now holds %d record(s) for one tag:\n' "${#rows[@]}" >>"$WORK/report"
	printf '    %s\n' "${rows[@]}" >>"$WORK/report"
	if [ "${#rows[@]}" -ne 2 ]; then
		printf '  FAIL: two platforms of one tag did not both survive. A store\n' >>"$WORK/report"
		printf '        keyed without the platform loses the first pull silently.\n' >>"$WORK/report"
		fail=1
	fi
	ids="$(printf '%s\n' "${rows[@]}" | awk '{print $1}' | sort -u | wc -l)"
	if [ "$ids" -ne 2 ]; then
		printf '  FAIL: both records carry the same image ID, so they are one image\n' >>"$WORK/report"
		fail=1
	fi
fi

# --------------------------------------------------------------------- 3
echo >>"$WORK/report"
echo "== 3. the extracted rootfs really is that architecture" >>"$WORK/report"
# ⛔ The clause that makes the two above worth anything. A record can say
# `linux/arm64` and hold amd64 bytes; this reads the ELF header of a binary
# inside the extracted tree and asks the machine, not the metadata.
for want in "$host_plat" "$foreign"; do
	id="$("$BIN" images --format '{{.ID}} {{.Platform}}' "$IMAGE" 2>/dev/null | awk -v p="$want" '$2==p{print $1; exit}')"
	if [ -z "$id" ]; then
		printf '  SKIP: no record for %s to extract\n' "$want" >>"$WORK/report"
		skipped=1
		continue
	fi
	timeout 1800 "$BIN" extract "$id" >"$WORK/e.out" 2>"$WORK/e.err"
	rc=$?
	if [ "$rc" -ne 0 ]; then
		printf '  SKIP: extract of %s exited %d: %s\n' "$want" "$rc" "$(tail -1 "$WORK/e.err")" >>"$WORK/report"
		skipped=1
		continue
	fi
	root="$("$BIN" inspect --format '{{.RootfsPath}}' "$id" 2>/dev/null)"
	probe=""
	for cand in usr/bin/bash bin/sh usr/bin/busybox bin/busybox; do
		[ -f "$root/$cand" ] && { probe="$root/$cand"; break; }
	done
	if [ -z "$probe" ]; then
		printf '  SKIP: %s has no binary this clause knows how to read\n' "$want" >>"$WORK/report"
		skipped=1
		continue
	fi
	# ⚠ The ELF machine word is the SECOND comma-field, not the first: the
	# first is "ELF 64-bit LSB pie executable" for every architecture alike,
	# which is evidence of nothing. The reader has to be able to check this.
	machine="$(file -b "$probe" | cut -d, -f2 | sed 's/^ *//')"
	printf '  %-14s %-12s (%s)\n' "$want" "$machine" "${probe#"$root"/}" >>"$WORK/report"
	# The ELF machine word `file` prints, for the platform we asked for.
	case "$want" in
	linux/arm64) need="aarch64" ;;
	linux/amd64) need="x86-64" ;;
	*) need="" ;;
	esac
	if [ -n "$need" ] && ! file -b "$probe" | grep -q "$need"; then
		printf '  FAIL: %s extracted a tree whose binaries are not %s\n' "$want" "$need" >>"$WORK/report"
		fail=1
	fi
done

# --------------------------------------------------------------------- 4
echo >>"$WORK/report"
echo "== 4. an index that offers nothing for the ask says what it does offer" >>"$WORK/report"
# ⛔ Never a bare 404. The refusal has to name the platforms available, or the
# caller cannot tell a typo from an image that was never built for them.
out="$(timeout 300 "$BIN" pull --platform linux/nosucharch "$IMAGE" 2>&1)"
rc=$?
printf '  exit              %d\n' "$rc" >>"$WORK/report"
printf '  says              %s\n' "$(printf '%s' "$out" | tr -d '\n' | cut -c1-96)" >>"$WORK/report"
if [ "$rc" -eq 0 ]; then
	printf '  FAIL: a platform the index does not carry was accepted\n' >>"$WORK/report"
	fail=1
elif ! printf '%s' "$out" | grep -q 'offers'; then
	printf '  FAIL: the refusal does not name what the index does offer\n' >>"$WORK/report"
	fail=1
fi

# --------------------------------------------------------------------- 5
echo >>"$WORK/report"
echo "== 5. a malformed --platform is a USAGE error, before any network" >>"$WORK/report"
out="$(timeout 120 "$BIN" pull --platform 'a/b/c/d' "$IMAGE" 2>&1)"
rc=$?
printf '  exit              %d (2 is invalid input, TODO/probe.md T-0110)\n' "$rc" >>"$WORK/report"
printf '  says              %s\n' "$(printf '%s' "$out" | tr -d '\n' | cut -c1-96)" >>"$WORK/report"
[ "$rc" -eq 2 ] || { printf '  FAIL: expected exit 2\n' >>"$WORK/report"; fail=1; }

out="$(timeout 120 "$BIN" pull --platform "$IMAGE" 2>&1)"
rc=$?
printf '  --platform with no value: exit %d\n' "$rc" >>"$WORK/report"
[ "$rc" -eq 2 ] || { printf '  FAIL: a flag swallowing its image is not exit 2\n' >>"$WORK/report"; fail=1; }

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
