#!/usr/bin/env bash
# Question: can a store GC delete an image while something is using it, and
# does a shared blob survive the removal of one of the two images that reach it?
#
# TODO/image.md T-0204. ⭐ The mechanism is read out of the corpus at file and
# line: references/qaidvoid__onelf/tree/crates/onelf-rt/src/main.rs:275-278
# holds the cache lock guard THROUGH exec, with its fd left inheritable, "so a
# concurrent process's GC cannot delete the package while it is still in use".
# A pid file is stale the moment a process dies unexpectedly, and the check
# that clears a stale one is the race this closes.
#
# ⚠ WHAT THIS SCRIPT CANNOT YET DO. T-0204's Prove holds the lock with
# `podbox run -d`, and `run` is M3 (TODO/milestones.md T-1104). The holder here
# is `flock(1)`, which takes the same advisory lock on the same file, so the
# refusal path is driven for real; what is not yet driven is podbox itself
# holding it across its own exec. The entry stays `partial` for that half.
#
#   ./160-store-gc.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/store-gc.txt"
REFERENCE="${PODBOX_TEST_IMAGE:-alpine:latest}"
WORK="$(mktemp -d)"
STORE="$WORK/store"
HOLDER=""
trap '[ -n "$HOLDER" ] && kill "$HOLDER" 2>/dev/null; rm -rf "$WORK"' EXIT INT TERM
export PODBOX_STORE="$STORE"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
command -v flock >/dev/null 2>&1 || {
	echo "SKIP: flock(1) is not on PATH, and it is what stands in for the" >&2
	echo "      running container this clause needs (podbox run is M3)." >&2
	exit 2
}

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'podbox            %s\n' "$("$BIN" version)"
printf 'reference         %s\n' "$REFERENCE"
printf 'holder            flock(1), standing in for podbox run -d (M3)\n'
echo

fail=0

if ! timeout 600 "$BIN" pull "$REFERENCE" >"$WORK/pull.out" 2>"$WORK/pull.err"; then
	echo "SKIP: podbox pull failed; the registry may be unreachable" >&2
	sed 's/^/    /' "$WORK/pull.err" >&2
	exit 2
fi
digest="$("$BIN" images --format '{{.Digest}}' "$REFERENCE")"
hex="${digest#sha256:}"
LOCKFILE="$STORE/locks/$hex.lock"

echo "== 1. a second tag shares the first tag's blobs"
"$BIN" tag "$REFERENCE" podbox-gc-probe:v1 || { echo "  FAIL: tag"; fail=1; }
before="$(find "$STORE/blobs" -type f | wc -l)"
printf '  blobs after tagging   %s\n' "$before"

echo
echo "== 2. removing one of the two names frees nothing"
"$BIN" rmi podbox-gc-probe:v1 >"$WORK/rmi.out" 2>&1
rmi_rc=$?
sed 's/^/    /' "$WORK/rmi.out"
after_one="$(find "$STORE/blobs" -type f | wc -l)"
printf '  blobs afterwards      %s\n' "$after_one"
[ "$rmi_rc" -eq 0 ] || { echo "  FAIL: rmi exited $rmi_rc"; fail=1; }
if [ "$after_one" -ne "$before" ]; then
	echo "  FAIL: a blob the surviving tag still needs was deleted"
	fail=1
else
	echo "  ok      reachability was computed over what survives"
fi

echo
echo "== 3. an image something holds is refused by rmi and skipped by prune"
# ⛔ The holder takes the SAME advisory lock podbox's own `hold` takes, on the
# same path. `flock -x` blocks a shared holder too, and podbox asks for the
# exclusive lock to test, so either shape answers.
mkdir -p "$(dirname "$LOCKFILE")"
# ⛔ ONE PROCESS, and `exec` is why. `flock FILE -c 'sleep N'` spawns the sleep
# as a CHILD, which inherits the locked fd; killing flock then leaves the sleep
# holding the lock and clause 4 below fails with the image still "in use".
# Measured here on 2026-09-08, and it is the inheritance T-0204 depends on
# working exactly as intended, arriving as a defect in the harness rather than
# in podbox. `exec` replaces the subshell with the sleeper, so `$!` is the one
# process holding fd 9 and killing it releases the lock.
( exec 9>"$LOCKFILE"; flock -x 9 || exit 1; exec sleep 25 ) &
HOLDER=$!
# ⚠ Wait for the lock to actually be held, rather than sleeping a guessed
# interval. A race here would make this clause pass for the wrong reason.
held=0
for _ in $(seq 1 100); do
	if ! flock -n -x "$LOCKFILE" -c true 2>/dev/null; then
		held=1
		break
	fi
	sleep 0.1
done
if [ "$held" -eq 0 ]; then
	echo "  SKIP: the holder never took the lock at $LOCKFILE" >&2
	exit 2
fi
printf '  holder pid %s holds %s\n' "$HOLDER" "locks/$hex.lock"

"$BIN" rmi "$REFERENCE" >"$WORK/rmi2.out" 2>"$WORK/rmi2.err"
rmi2_rc=$?
printf '  rmi exit %s\n' "$rmi2_rc"
sed 's/^/    /' "$WORK/rmi2.err"
[ "$rmi2_rc" -ne 0 ] || { echo "  FAIL: rmi removed an image in use"; fail=1; }
if grep -qi 'in use' "$WORK/rmi2.err"; then
	echo "  ok      the refusal says it is in use"
else
	echo "  FAIL: the refusal does not say the image is in use"
	fail=1
fi

"$BIN" image prune -af >"$WORK/prune.out" 2>&1
prune_rc=$?
sed 's/^/    /' "$WORK/prune.out"
[ "$prune_rc" -eq 0 ] || { echo "  FAIL: prune exited $prune_rc"; fail=1; }
if grep -q 'skipped:' "$WORK/prune.out"; then
	echo "  ok      prune named what it skipped"
else
	echo "  FAIL: prune skipped silently, which is a step that reports success"
	echo "        having done nothing it was asked to do"
	fail=1
fi
still="$("$BIN" images -q | wc -l)"
printf '  images left           %s\n' "$still"
[ "$still" -ge 1 ] || { echo "  FAIL: prune deleted the image anyway"; fail=1; }

echo
echo "== 4. once the holder goes, the image can be removed"
kill "$HOLDER" 2>/dev/null
wait "$HOLDER" 2>/dev/null
HOLDER=""
"$BIN" rmi "$REFERENCE" >"$WORK/rmi3.out" 2>&1
rmi3_rc=$?
sed 's/^/    /' "$WORK/rmi3.out"
after_all="$(find "$STORE/blobs" -type f 2>/dev/null | wc -l)"
printf '  rmi exit %s, blobs left %s\n' "$rmi3_rc" "$after_all"
[ "$rmi3_rc" -eq 0 ] || { echo "  FAIL: rmi still refused with no holder"; fail=1; }
[ "$after_all" -eq 0 ] || { echo "  FAIL: the last reference did not free the blobs"; fail=1; }

echo
echo "== 5. a blob whose path resolves outside the store is refused"
# ⛔ The containment check T-0204 shares with T-0304. ⚠ The opposite failure is
# in the corpus: ruri tracker issue #59 records its `-U` unmount reaching
# OUTSIDE the container when the container directory was under a FUSE mount.
#
# ⚠ THE CANARY HAS TO BE REACHABLE FROM A RECORD, or this clause passes for a
# weaker reason than it claims. `prune` only ever unlinks blobs a record names,
# so a stray symlink in `blobs/` is never touched and proves nothing about the
# check. This replaces a REAL blob file with a symlink to the canary, which is
# a path `delete` genuinely walks.
STORE2="$WORK/store2"
outside="$WORK/outside"
mkdir -p "$outside"
echo "do not delete me" > "$outside/canary"
canary_clause="ran"
if PODBOX_STORE="$STORE2" timeout 600 "$BIN" pull "$REFERENCE" \
	>"$WORK/pull2.out" 2>"$WORK/pull2.err"; then
	victim="$(find "$STORE2/blobs/sha256" -type f | head -1)"
	rm -f "$victim"
	ln -s "$outside/canary" "$victim"
	PODBOX_STORE="$STORE2" "$BIN" image prune -af >"$WORK/prune2.out" 2>&1
	prune2_rc=$?
	printf '  prune exit %s\n' "$prune2_rc"
	sed 's/^/    /' "$WORK/prune2.out"
	if [ -f "$outside/canary" ]; then
		echo "  ok      the canary outside the store survived"
	else
		echo "  FAIL: prune followed a symlink out of the store and deleted it"
		fail=1
		canary_clause="deleted"
	fi
	if grep -qi 'outside the store' "$WORK/prune2.out"; then
		echo "  ok      and it said so by name"
	else
		echo "  ⚠ RECORDED: it did not delete the canary and did not name the reason"
		canary_clause="survived, unnamed"
	fi
else
	echo "  SKIP: the second pull failed" >&2
	canary_clause="could not pull"
fi

{
	printf '# podbox store GC under a holder, TODO/image.md T-0204\n'
	printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# ⚠ The holder is flock(1). podbox run -d is M3 and is the other half.\n'
	printf 'reference         %s\n' "$REFERENCE"
	printf 'blobs_after_tag   %s\n' "$before"
	printf 'blobs_after_one   %s\n' "$after_one"
	printf 'rmi_under_holder  %s   (non-zero is the pass)\n' "$rmi2_rc"
	printf 'prune_skipped     %s\n' \
		"$(grep -c 'skipped:' "$WORK/prune.out" 2>/dev/null || echo 0)"
	printf 'images_left       %s\n' "$still"
	printf 'blobs_after_all   %s\n' "$after_all"
	printf 'canary_survived   %s\n' \
		"$([ -f "$outside/canary" ] && echo yes || echo no)"
	printf 'canary_clause     %s\n' "$canary_clause"
	echo
	echo '## the refusal, verbatim'
	sed 's/^/  /' "$WORK/rmi2.err"
	echo
	echo '## prune, verbatim'
	sed 's/^/  /' "$WORK/prune.out"
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
