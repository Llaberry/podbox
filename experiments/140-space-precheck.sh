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
REFERENCE="${PODBOX_TEST_IMAGE:-public.ecr.aws/docker/library/alpine:latest}"
WORK="$(mktemp -d)"
SMALL="$WORK/small"
trap 'mountpoint -q "$SMALL" 2>/dev/null && umount "$SMALL"; rm -rf "$WORK"' EXIT INT TERM

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}

# ⚠ NOT DOCKER HUB BY DEFAULT, AND THE REASON IS MEASURED. This script does not
# compare anything against docker, so it needs a registry rather than THE
# registry. Docker Hub answered
#   HTTP 429: TOOMANYREQUESTS: You have reached your unauthenticated pull rate limit
# on 2026-09-08 after this session's own runs, which turns a re-run of a check
# about disk space into a check about somebody else's quota.
# `experiments/150-image-acquisition.sh` stays on `alpine:latest` because it
# must ask docker about the same tag. `PODBOX_TEST_IMAGE` overrides this.
echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'podbox            %s\n' "$("$BIN" version)"
printf 'reference         %s\n' "$REFERENCE"
echo

fail=0
mkdir -p "$SMALL"

# ⛔ A REGISTRY THAT REFUSED IS NOT A CHECK THAT FAILED. The space precheck runs
# after the MANIFEST is fetched, because the manifest is what says how large the
# layers are, so every clause here needs the network to reach the registry once.
# Measured on 2026-09-08: Docker Hub answered
#   HTTP 429: TOOMANYREQUESTS: You have reached your unauthenticated pull rate limit
# and this script reported FAIL for a clause that had never run. TODO/RULES.md
# section 6: "could not run" must never read as "denied".
registry_refused() {
	grep -qE 'HTTP [0-9]{3}|transport:' "$1"
}

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

if registry_refused "$WORK/err.txt"; then
	echo "  SKIP: the registry refused before the space check could run" >&2
	sed 's/^/    /' "$WORK/err.txt" >&2
	exit 2
fi
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
free_inodes="-"
# ⛔ ROOMY FIRST, THEN FILLED, and that is not a detail. Mounting straight into
# a 12-inode tmpfs is NOT DETERMINISTIC: the store's own five directories may or
# may not fit, so the refusal comes sometimes from the inode clause and
# sometimes from `mkdir`. Both are honest refusals and the record then flips
# between two values on re-runs, which `result-diff.sh` reports as a finding
# every time and which is noise rather than one. Measured here on 2026-09-08:
# the same script reported `ran` and then `refused earlier` on two runs of one
# machine. Building the store while inodes are plentiful and consuming the rest
# afterwards puts the check in exactly one place.
if mount -t tmpfs -o size=256M,nr_inodes=64 podbox-precheck "$SMALL" 2>"$WORK/mount2.err"; then
	# `images` opens the store, which is what creates its directories.
	PODBOX_STORE="$SMALL/store" "$BIN" images >/dev/null 2>&1
	# Then eat inodes until fewer remain than a pull needs: one per layer plus
	# eight, and alpine has one layer.
	i=0
	while [ "$(stat -f -c%d "$SMALL")" -gt 6 ] && [ "$i" -lt 200 ]; do
		: > "$SMALL/filler.$i" || break
		i=$((i + 1))
	done
	free_inodes="$(stat -f -c%d "$SMALL")"
	printf '  free inodes       %s\n' "$free_inodes"
	PODBOX_STORE="$SMALL/store" "$BIN" pull "$REFERENCE" >"$WORK/out2.txt" 2>"$WORK/err2.txt"
	rc2=$?
	printf '  exit %s\n' "$rc2"
	sed 's/^/    /' "$WORK/err2.txt"
	if registry_refused "$WORK/err2.txt"; then
		echo "  SKIP: the registry refused before the inode check could run" >&2
		inode_clause="could not run: the registry refused"
		exit 2
	elif [ "$rc2" -eq 0 ]; then
		echo "  FAIL: podbox pulled into a filesystem with $free_inodes free inodes"
		fail=1
	elif grep -qF "inodes" "$WORK/err2.txt"; then
		echo "  ok      the refusal names inodes"
	else
		# ⛔ Now a FAILURE rather than a recorded reading. The store exists and
		# has room in bytes, so the only thing left to refuse on is inodes.
		echo "  FAIL: refused, but not by the inode clause"
		inode_clause="refused earlier"
		fail=1
	fi
else
	echo "  SKIP: could not mount the second tmpfs" >&2
	inode_clause="could not mount"
fi

{
	printf '# podbox space precheck, TODO/image.md T-0203\n'
	printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# A 1 MiB tmpfs, and a roomy one filled until few inodes remain. Both\n'
	printf '# are mounted by this script rather than being fixtures.\n'
	printf 'tmpfs_mounted     %s\n' "$tmpfs_ok"
	printf 'refused_exit      %s\n' "$rc"
	printf 'blobs_written     %s\n' "$blobs"
	printf 'inode_clause      %s\n' "$inode_clause"
	printf 'inode_free_at_pull %s\n' "$free_inodes"
	printf 'names_destination %s\n' \
		"$(printf '%s' "$msg" | grep -qF -- "$SMALL/store" && echo yes || echo no)"
	printf 'names_unit        %s\n' \
		"$(printf '%s' "$msg" | grep -qF -- "iB" && echo yes || echo no)"
	echo
	echo '## the refusal, verbatim'
	# ⛔ THE WORKING DIRECTORY IS REDACTED, and that is what makes this file
	# evidence rather than noise. `mktemp -d` gives a different name every run,
	# so a transcript carrying it CHANGES on every re-run and
	# scripts/common/result-diff.sh reports a finding every time. 130- carries
	# the same rule for the same reason: an absolute path in tracked evidence is
	# one machine's directory layout recorded as though it were a fact about the
	# measurement. `names_destination` above is what asserts the path was named.
	sed -e "s|$WORK|<work>|g" -e 's/^/  /' "$WORK/err.txt"
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
