#!/usr/bin/env bash
# Question: does the store's written contract hold against real concurrent
# processes, or only against the orderings somebody imagined?
#
# TODO/image.md T-0210. The invariants are I1 to I7 in
# `crates/podbox-image/src/store.rs`'s module header, and each clause here drives
# one of them. ⛔ THE CONTRACT LIVES IN THE CODE AND THIS SCRIPT DRIVES IT. A
# contract in an entry that nobody reads while changing the store is not one.
#
# ⭐ REAL PROCESSES, NOT A MOCK. T-0210's Decision: a race that only a mock can
# produce is a race the mock's author imagined. Every worker below is a separate
# `podbox` process against one store directory.
#
# ⛔ THE REGISTRY IS NOT UNDER TEST HERE AND MUST NOT BE THE BOTTLENECK. Three
# small references from `public.ecr.aws`, which is the acceptance's registry
# because it carries no Docker Hub quota (TODO/image.md T-0206).
#
#   ./210-store-concurrency.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/store-concurrency.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

# ⚠ Three DIFFERENT references, because two pulls of the same one share every
# blob and never exercise the interesting half: two writers adding different
# records to one index.
REFS=(
	"${PODBOX_TEST_IMAGE:-public.ecr.aws/docker/library/alpine:latest}"
	"${PODBOX_TEST_IMAGE_2:-public.ecr.aws/docker/library/alpine:3.20}"
	"${PODBOX_TEST_IMAGE_3:-public.ecr.aws/docker/library/alpine:3.19}"
)
WORKERS="${PODBOX_CONCURRENCY_WORKERS:-8}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
command -v sha256sum >/dev/null 2>&1 || { echo "SKIP: no sha256sum" >&2; exit 2; }

fail=0
skipped=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'workers           %s\n' "$WORKERS"
	printf 'references        %s\n' "${#REFS[@]}"
	echo
} >"$WORK/report"

# ⭐ ONE VERIFIER, USED AFTER EVERY CLAUSE. It answers the only question that
# matters about a store: is every byte in it what its own name says, and does
# every record still reach everything it needs?
#   $1 store   prints "blobs=<n> bad=<n> records=<n> missing=<n> partials=<n>"
verify() {
	local store="$1" blobs=0 bad=0 records=0 missing=0 partials=0
	local f name got
	for f in "$store"/blobs/sha256/*; do
		[ -e "$f" ] || continue
		blobs=$((blobs + 1))
		name="${f##*/}"
		got="$(sha256sum "$f")"
		got="${got%% *}"
		[ "$got" = "$name" ] || bad=$((bad + 1))
	done
	for f in "$store"/staging/*.partial; do
		[ -e "$f" ] || continue
		partials=$((partials + 1))
	done
	# ⛔ Read through podbox rather than by parsing the index by hand: a
	# verifier with its own reader checks its own reader.
	while IFS= read -r line; do
		[ -n "$line" ] || continue
		records=$((records + 1))
		for name in $line; do
			case "$name" in sha256:*) ;; *) continue ;; esac
			[ -e "$store/blobs/sha256/${name#sha256:}" ] || missing=$((missing + 1))
		done
	done <<-EOF
		$(PODBOX_STORE="$store" "$BIN" images --format '{{.Digest}}' 2>/dev/null)
	EOF
	printf 'blobs=%d bad=%d records=%d missing=%d partials=%d\n' \
		"$blobs" "$bad" "$records" "$missing" "$partials"
}

# --------------------------------------------------------------------- 1
say "== 1. I1 and I2: $WORKERS writers, ${#REFS[@]} references, one index"
# ⛔ The interesting case, and the one the previous session did NOT drive: two
# pulls of DIFFERENT images racing on one index. Same-reference pulls share
# every blob and never make two writers add two records.
S1="$WORK/s1"
mkdir -p "$S1"
i=0
while [ "$i" -lt "$WORKERS" ]; do
	ref="${REFS[$((i % ${#REFS[@]}))]}"
	(PODBOX_STORE="$S1" timeout 900 "$BIN" pull "$ref" >"$WORK/w$i.out" 2>&1; echo "$?" >"$WORK/w$i.rc") &
	i=$((i + 1))
done
wait
rcs=""
i=0
worst=0
while [ "$i" -lt "$WORKERS" ]; do
	rc="$(cat "$WORK/w$i.rc" 2>/dev/null || echo 99)"
	rcs="$rcs $rc"
	[ "$rc" -gt "$worst" ] && worst="$rc"
	i=$((i + 1))
done
say "  exit codes       $rcs"
say "  verify           $(verify "$S1")"
if [ "$worst" -ne 0 ]; then
	# ⚠ A network failure is not a contract failure, and the two must not be
	# collapsed. The transcripts say which.
	if grep -qiE 'connect|resolve|timed out|tls|certificate' "$WORK"/w*.out; then
		say "  SKIP: a worker could not reach the registry, so the contract was not tested"
		sed -n '1,3p' "$WORK/w0.out" | sed 's/^/    /' >>"$WORK/report"
		skipped=1
	else
		say "  FAIL: a worker exited non-zero for a reason that is not the network"
		fail=1
	fi
else
	v="$(verify "$S1")"
	case "$v" in
	*"bad=0"*) ;;
	*) say "  FAIL: a blob does not hash to its own name"; fail=1 ;;
	esac
	case "$v" in
	*"missing=0"*) ;;
	*) say "  FAIL: a record reaches a blob the store does not have"; fail=1 ;;
	esac
	case "$v" in
	*"partials=0"*) ;;
	*) say "  FAIL: a staging file survived a clean run"; fail=1 ;;
	esac
	# ⛔ One record per reference and no duplicates: the index survived
	# concurrent read-modify-writes.
	n="$(PODBOX_STORE="$S1" "$BIN" images --format '{{.Digest}}' 2>/dev/null | sort -u | wc -l)"
	say "  distinct images  $n (want ${#REFS[@]})"
	[ "$n" -eq "${#REFS[@]}" ] || {
		say "  FAIL: concurrent writers lost or duplicated a record"
		fail=1
	}
fi

# --------------------------------------------------------------------- 2
say ""
say "== 2. I4: a prune racing a hold never deletes what the hold needs"
# ⛔ THE CASE THAT WAS NEVER DRIVEN, and reading the code for it on 2026-09-09
# found the defect: `prune` asked `in_use` OUTSIDE the index lock, so a `run`
# taking its hold in between kept its image lock and lost its blobs.
if [ ! -d "$S1/blobs" ] || [ "$skipped" -eq 1 ]; then
	say "  SKIP: clause 1 did not populate a store"
	skipped=1
else
	ref="${REFS[0]}"
	# A holder that lives for a few seconds, taken through the real verb.
	PODBOX_STORE="$S1" timeout 300 "$BIN" run --pull never "$ref" /bin/sleep 6 \
		>/dev/null 2>&1 &
	holder=$!
	# ⚠ Give the holder time to reach its hold. It extracts first, so the
	# window is not instant; the loop below is what waits, bounded.
	waited=0
	while [ "$waited" -lt 60 ]; do
		[ -n "$(ls -A "$S1/locks" 2>/dev/null)" ] && break
		sleep 0.2
		waited=$((waited + 1))
	done
	held="$(ls -A "$S1/locks" 2>/dev/null | wc -l)"
	say "  image locks held while the payload runs: $held"
	out="$(PODBOX_STORE="$S1" timeout 300 "$BIN" image prune -af 2>&1)"
	rc=$?
	say "  prune -af        rc=$rc"
	say "    $(printf '%s' "$out" | grep -i 'skipped' | head -1 | cut -c1-88)"
	wait "$holder"
	hrc=$?
	say "  the holder exited $hrc"
	if [ "$held" -eq 0 ]; then
		say "  SKIP: the holder never took a lock, so nothing raced"
		skipped=1
	else
		[ "$hrc" -eq 0 ] || { say "  FAIL: the payload died while a prune ran"; fail=1; }
		printf '%s' "$out" | grep -qi 'skipped' || {
			say "  FAIL: prune did not name what it skipped"
			fail=1
		}
		v="$(verify "$S1")"
		say "  verify           $v"
		case "$v" in
		*"missing=0"*) ;;
		*) say "  FAIL: prune deleted a blob a surviving record still reaches"; fail=1 ;;
		esac
	fi
fi

# --------------------------------------------------------------------- 3
say ""
say "== 3. I5: what a SIGKILL leaves, and that opening the store sweeps it"
# ⭐ The gap this entry was filed for: a staging file named after a process that
# no longer exists was left forever, because nothing ever removed one.
S3="$WORK/s3"
mkdir -p "$S3"
# ⛔ NO `timeout` ON THE VICTIM, and that is the opposite of this repository's
# usual rule for a reason. Measured on 2026-09-09, and it cost this clause a
# false FAIL: `timeout 900 podbox pull &` makes `$!` the pid of TIMEOUT, so
# `kill -9 "$!"` kills the wrapper and leaves podbox running, still holding its
# staging file. The sweep then correctly refuses to take a file a live writer
# holds, and the clause reads that correct behaviour as a defect.
# ⚠ The upper bound is still here: the wait loop below is bounded to 10 s and
# the kill happens whether or not it saw anything.
PODBOX_STORE="$S3" "$BIN" pull "${REFS[0]}" >/dev/null 2>&1 &
victim=$!
# ⚠ Killed mid-pull rather than at a chosen point: the contract is about what a
# SIGKILL can leave, and choosing the moment would test the moment.
waited=0
while [ "$waited" -lt 100 ]; do
	[ -n "$(ls -A "$S3/staging" 2>/dev/null)" ] && break
	sleep 0.1
	waited=$((waited + 1))
done
left="$(ls -A "$S3/staging" 2>/dev/null | wc -l)"
kill -9 "$victim" 2>/dev/null
wait "$victim" 2>/dev/null
after_kill="$(ls -A "$S3/staging" 2>/dev/null | wc -l)"
say "  staging files while it ran:   $left"
say "  staging files after SIGKILL:  $after_kill"
if [ "$left" -eq 0 ]; then
	say "  SKIP: the pull finished or never staged, so nothing was left to sweep"
	skipped=1
else
	# ⛔ Opening the store is what sweeps, and every command opens it.
	PODBOX_STORE="$S3" "$BIN" images >/dev/null 2>&1
	swept="$(ls -A "$S3/staging" 2>/dev/null | wc -l)"
	say "  after any later command:      $swept"
	[ "$swept" -eq 0 ] || {
		say "  FAIL: an abandoned staging file survived a later command"
		fail=1
	}
	# ⛔ And the store is still sound: a sweep that took a live blob would show
	# up here rather than in a message.
	say "  verify           $(verify "$S3")"
fi

# --------------------------------------------------------------------- 4
say ""
say "== 4. I5 again, from the other side: the sweep leaves a live writer alone"
# ⚠ The dangerous half. A sweep that removes what it can see removes what
# another process is mid-write, which turns a fixed leak into a broken pull.
S4="$WORK/s4"
mkdir -p "$S4"
PODBOX_STORE="$S4" timeout 900 "$BIN" pull "${REFS[0]}" >"$WORK/s4.out" 2>&1 &
writer=$!
waited=0
while [ "$waited" -lt 100 ]; do
	[ -n "$(ls -A "$S4/staging" 2>/dev/null)" ] && break
	sleep 0.1
	waited=$((waited + 1))
done
during="$(ls -A "$S4/staging" 2>/dev/null | wc -l)"
# A second podbox opens the same store, which is what runs the sweep.
PODBOX_STORE="$S4" "$BIN" images >/dev/null 2>&1
wait "$writer"
wrc=$?
say "  staging files mid-pull:       $during"
say "  the pull exited               $wrc  (a sweep that took its file would not)"
if [ "$during" -eq 0 ]; then
	say "  SKIP: the pull never staged anything, so no sweep raced it"
	skipped=1
elif [ "$wrc" -ne 0 ] && grep -qiE 'connect|resolve|timed out|tls|certificate' "$WORK/s4.out"; then
	say "  SKIP: the pull could not reach the registry"
	skipped=1
else
	[ "$wrc" -eq 0 ] || { say "  FAIL: a concurrent sweep broke a pull in flight"; fail=1; }
	v="$(verify "$S4")"
	say "  verify           $v"
	case "$v" in
	*"bad=0"*) ;;
	*) say "  FAIL: a blob does not hash to its own name"; fail=1 ;;
	esac
fi

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

# ⛔ Three states, and "could not run" is never a failure.
[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
