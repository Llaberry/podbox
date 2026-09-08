#!/usr/bin/env bash
# Question: does podbox refuse a pull it cannot fit BEFORE fetching anything,
# and does the refusal name the destination, the free amount, the required
# amount and the unit?
#
# TODO/image.md T-0203. ⭐ The defect this exists to avoid is in the corpus at
# file and line:
# references/mhx__dwarfs/tree/src/utility/filesystem_extractor.cpp:544-552
# formats a libarchive STATUS CODE into "short write: {} != {}" as though it
# were a byte count, so the message names neither the errno (ENOSPC) nor the
# path. The reader learns nothing they can act on.
#
# ⚠ THE NUMBER. T-0203's Prove named `90-space-precheck.sh`, and 90 is
# `90-nsswitch-contract.sh`. experiments/README.md: a number is never reused,
# because a citation of it has to keep meaning what it meant. The entry carries
# the correction and this script is 140-.
#
#   ./140-space-precheck.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/space-precheck.txt"
REFERENCE="${PODBOX_TEST_IMAGE:-alpine:latest}"
WORK="$(mktemp -d)"
SMALL="$WORK/small"
trap 'mountpoint -q "$SMALL" 2>/dev/null && umount "$SMALL"; rm -rf "$WORK"' EXIT INT TERM

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'podbox            %s\n' "$("$BIN" version)"
printf 'reference         %s\n' "$REFERENCE"
echo

fail=0
mkdir -p "$SMALL"

# ⭐ A REAL SMALL FILESYSTEM, not a fixture. The claim under test is about
# statfs(2) on a destination, so the destination has to be one that genuinely
# has no room. A tmpfs of 1 MiB is the smallest honest way to produce that.
#
# ⚠ `mount` needs privilege this process may not have. That is a SKIP with its
# own exit code, never a failure: TODO/RULES.md section 6, "could not run" must
# never read as "denied".
tmpfs_ok=0
if mount -t tmpfs -o size=1M,nr_inodes=64 podbox-precheck "$SMALL" 2>"$WORK/mount.err"; then
	tmpfs_ok=1
else
	echo "== 1. a destination with no room"
	echo "  SKIP: could not mount a 1 MiB tmpfs at $SMALL" >&2
	sed 's/^/    /' "$WORK/mount.err" >&2
	echo "        This clause needs CAP_SYS_ADMIN in this mount namespace." >&2
	exit 2
fi

echo "== 1. a pull into a destination with no room is refused up front"
# ⛔ The exit code is read from podbox, not through a pipe. docs/AGENTS.md
# absolute 8: a guard piped into anything reports the pipeline's status.
PODBOX_STORE="$SMALL/store" "$BIN" pull "$REFERENCE" >"$WORK/out.txt" 2>"$WORK/err.txt"
rc=$?
msg="$(cat "$WORK/err.txt" "$WORK/out.txt")"
printf '  exit %s\n' "$rc"
sed 's/^/    /' "$WORK/err.txt"

if [ "$rc" -eq 0 ]; then
	echo "  FAIL: podbox pulled into a 1 MiB filesystem"
	fail=1
fi

echo
echo "== 2. the refusal names all four things"
for what in "$SMALL/store:the destination" "free:the free amount" \
	"needed:the required amount" "iB:the unit"; do
	needle="${what%%:*}"
	label="${what#*:}"
	if printf '%s' "$msg" | grep -qF -- "$needle"; then
		printf '  ok      %-24s %s\n' "$label" "$needle"
	else
		printf '  FAIL    %-24s no %s in the message\n' "$label" "$needle"
		fail=1
	fi
done

echo
echo "== 3. nothing was written before the refusal"
# ⛔ The whole point of a precheck. A partial store has to be cleaned up on a
# filesystem that is already full, which is the state in which cleanup is least
# likely to work.
blobs=0
[ -d "$SMALL/store/blobs" ] && blobs="$(find "$SMALL/store/blobs" -type f | wc -l)"
printf '  blobs written     %s\n' "$blobs"
[ "$blobs" -eq 0 ] || { echo "  FAIL: the refusal came after bytes were stored"; fail=1; }

echo
echo "== 4. inodes are checked as well as blocks"
# ⚠ The same tmpfs with room in bytes and none in inodes. A check that reads
# blocks alone passes this and then fails with ENOSPC and plenty of space left,
# which is the error nobody can diagnose.
umount "$SMALL" 2>/dev/null
inode_clause="ran"
if mount -t tmpfs -o size=256M,nr_inodes=12 podbox-precheck "$SMALL" 2>"$WORK/mount2.err"; then
	PODBOX_STORE="$SMALL/store" "$BIN" pull "$REFERENCE" >"$WORK/out2.txt" 2>"$WORK/err2.txt"
	rc2=$?
	printf '  exit %s\n' "$rc2"
	sed 's/^/    /' "$WORK/err2.txt"
	if [ "$rc2" -eq 0 ]; then
		echo "  FAIL: podbox pulled into a filesystem with 12 inodes"
		fail=1
	elif grep -qF "inodes" "$WORK/err2.txt"; then
		echo "  ok      the refusal names inodes"
	else
		# ⚠ Not a failure on its own: with 12 inodes the store's own
		# directories cannot be created, so the block check or mkdir may
		# refuse first. Recorded as what it is.
		echo "  ⚠ RECORDED  refused, but not by the inode clause"
		inode_clause="refused earlier"
	fi
else
	echo "  SKIP: could not mount the second tmpfs" >&2
	inode_clause="could not mount"
fi

{
	printf '# podbox space precheck, TODO/image.md T-0203\n'
	printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# A 1 MiB tmpfs and a 12-inode tmpfs, both mounted here, not fixtures.\n'
	printf 'tmpfs_mounted     %s\n' "$tmpfs_ok"
	printf 'refused_exit      %s\n' "$rc"
	printf 'blobs_written     %s\n' "$blobs"
	printf 'inode_clause      %s\n' "$inode_clause"
	printf 'names_destination %s\n' \
		"$(printf '%s' "$msg" | grep -qF -- "$SMALL/store" && echo yes || echo no)"
	printf 'names_unit        %s\n' \
		"$(printf '%s' "$msg" | grep -qF -- "iB" && echo yes || echo no)"
	echo
	echo '## the refusal, verbatim'
	sed 's/^/  /' "$WORK/err.txt"
} > "$OUT"
echo
echo "  written to $OUT"

if [ -x "$REPO/scripts/common/result-diff.sh" ]; then
	echo
	echo "== against the committed reading"
	"$REPO/scripts/common/result-diff.sh" "$OUT" || true
fi

[ "$fail" -eq 0 ] || exit 1
exit 0
