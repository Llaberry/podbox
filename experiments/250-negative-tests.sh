#!/usr/bin/env bash
# Question: does every refusal this project SHIPPED actually happen?
#
# TODO/milestones.md T-1109. `TOOL.md` section 9 names three; this corpus adds
# five more, one per honesty rule that has landed since.
#
# ⭐ WHY THIS EXISTS AT ALL. Every honesty rule in `TOOL.md` sections 4.1 and 6.8
# is unenforced until something asserts the refusal happens. A rule that is only
# prose regresses silently, which is the exact failure mode the whole design is
# built against: a runtime that quietly stops refusing looks identical to one
# that never had to.
#
# ⛔ DRIVEN FROM THE OUTSIDE, THROUGH THE SHIPPED BINARY. A unit test asserting
# that a function returns an error is a test of that function. What a caller
# gets is an exit code and a line on stderr, and that is what every clause below
# reads. Several of these refusals are produced by code no unit test reaches.
#
# ⛔ EVERY CLAUSE ASSERTS TWO THINGS, and the second is the one that rots:
#
#   1. the exit code, read from the process that produced it, unpiped;
#   2. that the message NAMES THE REASON. A refusal a caller cannot act on is a
#      refusal that costs a session, and "unknown option" where the table has a
#      reason is a regression even though the code is the same.
#
# ⚠ The exit codes are DATA, read out of the binary rather than written here:
# `scripts/common/exit-codes.sh` and TODO/cli.md T-0802. Six clauses across four
# experiments had a `2` written into them and all went red at once the day
# docker's codes were measured.
#
#   ./250-negative-tests.sh
#
# Exit: 0 every refusal happened and named its reason, 1 one did not, 2 could
#       not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/negative-tests.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

IMAGE="${PODBOX_NEG_IMAGE:-public.ecr.aws/docker/library/alpine:3.20}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. ./scripts/dev.sh build" >&2
	exit 2
}
command -v jq >/dev/null 2>&1 || {
	echo "SKIP: jq is not on PATH. ./scripts/common/bootstrap-env.sh tools" >&2
	exit 2
}
# shellcheck source=../scripts/common/exit-codes.sh
. "$REPO/scripts/common/exit-codes.sh"
podbox_exit_codes "$BIN" || {
	echo "SKIP: cannot read podbox's exit-code table" >&2
	exit 2
}

export PODBOX_STORE="$WORK/store"
fail=0
skipped=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'image             %s\n' "$IMAGE"
	printf 'flag-error code   %s\n' "$PODBOX_EXIT_FLAG_ERROR"
	printf 'cli-error code    %s\n' "$PODBOX_EXIT_CLI_ERROR"
	echo
} >"$WORK/report"

timeout 600 "$BIN" pull "$IMAGE" >"$WORK/pull.log" 2>&1 || {
	echo "SKIP: could not pull $IMAGE" >&2
	tail -3 "$WORK/pull.log" >&2
	exit 2
}

# ⛔ ONE PLACE runs a refusal, so every clause asserts the same two things and
# the report cannot show an assertion that was not made.
#   refuses <name> <want-code> <needle> -- <podbox args...>
refuses() {
	local name="$1" want="$2" needle="$3"
	shift 3
	[ "${1:-}" = "--" ] && shift
	local out rc
	out="$(timeout 300 "$BIN" "$@" 2>&1 >/dev/null)"
	rc=$?
	local verdict="ok"
	if [ "$rc" != "$want" ]; then
		verdict="WRONG CODE, wanted $want"
		fail=1
	elif ! printf '%s' "$out" | grep -qF -- "$needle"; then
		verdict="the code is right and the REASON is not named"
		fail=1
	fi
	printf '  %-34s rc=%-4s %s\n' "$name" "$rc" "$verdict" >>"$WORK/report"
	printf '      %s\n' "$(printf '%s' "$out" | grep -F -- "$needle" | head -1 | cut -c1-118)" \
		>>"$WORK/report"
	[ "$verdict" = "ok" ] || printf '      GOT: %s\n' \
		"$(printf '%s' "$out" | head -2 | tr '\n' ' ' | cut -c1-118)" >>"$WORK/report"
}

# --------------------------------------------------------------------- 1
say "== 1. TOOL.md section 9's three"
# ⭐ `--network=none` must FAIL with a named reason. ⚠ The reason comes out of
# the parity table rather than being written here: T-0801 made the table
# binding, so the row's own note is what a caller reads.
refuses "run --network=none" "$PODBOX_EXIT_FLAG_ERROR" \
	"no network namespace to select" -- run --network=none "$IMAGE" true
# ⭐ `-v ...:ro` must be REJECTED, not honoured as a copy.
refuses "run -v host:/mapped:ro" "$PODBOX_EXIT_FLAG_ERROR" \
	"a copy pretending to be a mount" -- run -v "$WORK:/mapped:ro" "$IMAGE" true
# ⚠ The third, a Go payload under `interpose` declined rather than silently
# unvirtualized, is M6's and cannot be asserted before the interposer exists.
# ⛔ Recorded as a clause that DID NOT RUN rather than left out: a missing
# assertion and a passing one look the same in a report that omits it.
say "  a Go payload under interpose        SKIPPED: M6 has no interposer yet"
say "      TODO/interpose.md T-0709 is the entry, and this clause is its Prove"
skipped=1

# --------------------------------------------------------------------- 2
say ""
say "== 2. the honesty switch: --strict refuses a degraded run (T-0804)"
# ⛔ Two halves, and the second is what makes the first mean something.
out="$(timeout 300 "$BIN" run --strict --rm "$IMAGE" true 2>&1 >/dev/null)"
rc=$?
say "  run --strict                       rc=$rc"
printf '%s' "$out" | grep -F -- "--strict, and this run is degraded" | head -1 \
	| sed 's/^/      /' | cut -c1-120 >>"$WORK/report"
if [ "$rc" = "$PODBOX_EXIT_RUNTIME_ERROR" ]; then
	printf '%s' "$out" | grep -q -- "--strict, and this run is degraded" || {
		say "  FAIL: it refused without naming --strict as the reason"
		fail=1
	}
	# ⛔ AND IT NAMES EVERY REASON, not the first one it found.
	n="$(printf '%s' "$out" | grep -c '^  - ')"
	say "  reasons named                      $n"
	[ "$n" -ge 1 ] || { say "  FAIL: it refused and listed nothing"; fail=1; }
elif [ "$rc" = 0 ]; then
	# ⚠ A machine where nothing about the run is degraded. Legitimate, and it
	# means this clause measured nothing rather than that it passed.
	say "  ⚠ nothing about this run is degraded here, so --strict had nothing"
	say "    to refuse. That is a reading about this machine, not a pass."
	skipped=1
else
	say "  FAIL: --strict exited $rc, which is neither 0 nor a refusal"
	fail=1
fi
# ⚠ And WITHOUT it the same run proceeds, or `--strict` is the default in
# disguise.
timeout 300 "$BIN" run --rm "$IMAGE" true >/dev/null 2>&1
rc=$?
say "  the same run without --strict      rc=$rc (must be 0)"
[ "$rc" -eq 0 ] || { say "  FAIL: the run does not work without --strict"; fail=1; }

# --------------------------------------------------------------------- 3
say ""
say "== 3. -t is a NAMED refusal where /dev/ptmx is unusable (T-0503)"
# ⚠ Conditional on this machine, and which arm ran is recorded. A pty podbox
# could not measure is not one it may promise, and a pty it CAN allocate is not
# a refusal to assert.
usable="$(timeout 120 "$BIN" probe --json 2>/dev/null | jq -r '.ptmx.usable // false')"
say "  /dev/ptmx usable here              $usable"
if [ "$usable" = "true" ]; then
	timeout 300 "$BIN" run -t --rm "$IMAGE" true >/dev/null 2>&1
	rc=$?
	say "  run -t                             rc=$rc (a pty is available, so 0)"
	[ "$rc" -eq 0 ] || { say "  FAIL: -t was refused on a machine with a pty"; fail=1; }
	say "  ⚠ the refusal arm could not be measured here: this machine has a pty"
	skipped=1
else
	refuses "run -t" "$PODBOX_EXIT_RUNTIME_ERROR" \
		"cannot allocate a pty" -- run -t --rm "$IMAGE" true
fi

# --------------------------------------------------------------------- 4
say ""
say "== 4. the table decides, and an unlisted flag cannot be quietly accepted"
refuses "an unlisted flag" "$PODBOX_EXIT_FLAG_ERROR" \
	"no row in the parity table" -- run --no-such-flag "$IMAGE" true
refuses "a None flag names its status" "$PODBOX_EXIT_FLAG_ERROR" \
	"status None" -- run --privileged "$IMAGE" true
refuses "a None VERB names its reason" "$PODBOX_EXIT_RUNTIME_ERROR" \
	"cgroup this runtime does not grant" -- stats

# --------------------------------------------------------------------- 5
say ""
say "== 5. podbox speaks HTTPS only, and never downgrades (T-0201)"
# ⛔ Under a timeout, because the failure this refusal prevents is a HANG: on
# the runtimes podbox targets tcp/80 is black-holed, so a fallback does not fail,
# it waits. A clause with no bound would pass by hanging.
out="$(timeout 30 "$BIN" pull "http://registry.invalid/library/x:latest" 2>&1 >/dev/null)"
rc=$?
say "  pull http://...                      rc=$rc (124 would be the hang)"
[ "$rc" != 124 ] || { say "  FAIL: it hung, which is what this refusal exists to prevent"; fail=1; }
[ "$rc" = "$PODBOX_EXIT_CLI_ERROR" ] || { say "  FAIL: wanted $PODBOX_EXIT_CLI_ERROR"; fail=1; }
printf '%s' "$out" | grep -q 'HTTPS only' || {
	say "  FAIL: it refused without saying podbox is HTTPS only"
	fail=1
}
printf '      %s\n' "$(printf '%s' "$out" | grep -o 'HTTPS only[^.]*' | head -1 | cut -c1-110)" \
	>>"$WORK/report"

# --------------------------------------------------------------------- 6
say ""
say "== 6. a container podbox did not see end has NO exit code (T-0604)"
# ⛔ `wait` refuses rather than printing a guess. A runtime that invents an exit
# code is lying in the one field an automated caller reads first.
cid="$(timeout 300 "$BIN" run -d --name "neg-$$" "$IMAGE" sleep 30 2>/dev/null)"
if [ -z "$cid" ]; then
	say "  SKIP: could not start a detached container"
	skipped=1
else
	lp="$(timeout 60 "$BIN" inspect --format '{{.LauncherPid}}' "neg-$$" 2>/dev/null)"
	if [ -n "$lp" ] && [ "$lp" != 0 ]; then
		kill -9 "$lp" 2>/dev/null
		sleep 1
		out="$(timeout 60 "$BIN" wait "neg-$$" 2>&1 >/dev/null)"
		rc=$?
		state="$(timeout 60 "$BIN" inspect --format '{{.State}}' "neg-$$" 2>/dev/null)"
		code="$(timeout 60 "$BIN" inspect --format '{{.ExitCode}}' "neg-$$" 2>/dev/null)"
		say "  after SIGKILL of the launcher      state=$state exitcode=$code wait rc=$rc"
		[ "$state" = dead ] || { say "  FAIL: it does not read dead"; fail=1; }
		[ "$code" = "-" ] || { say "  FAIL: it invented an exit code: $code"; fail=1; }
		[ "$rc" = "$PODBOX_EXIT_RUNTIME_ERROR" ] || {
			say "  FAIL: wait exited $rc rather than refusing"
			fail=1
		}
	else
		say "  SKIP: no launcher pid to kill"
		skipped=1
	fi
	timeout 60 "$BIN" rm -f "neg-$$" >/dev/null 2>&1
fi

# --------------------------------------------------------------------- 7
say ""
say "== 7. the attribution census is 0 or 2, never 1 (T-1109's own rule)"
# ⛔ A 1 means the runtime moved or a probe stopped discriminating, and either is
# a finding rather than a flake. ⚠ Its exit code is read from the process that
# produced it, so it is run unpiped into a file.
if [ -x "$REPO/experiments/30-attribution-census.sh" ]; then
	timeout 900 "$REPO/experiments/30-attribution-census.sh" >"$WORK/census.log" 2>&1
	crc=$?
	say "  30-attribution-census.sh           rc=$crc"
	case "$crc" in
	0) say "      it ran and matched" ;;
	2) say "      it could not run here, which is the third state" ;;
	*)
		say "      ⛔ a 1 is a FINDING: the runtime moved or a probe stopped"
		say "         discriminating. The log tail:"
		tail -3 "$WORK/census.log" | sed 's/^/         /' >>"$WORK/report"
		fail=1
		;;
	esac
else
	say "  SKIP: 30-attribution-census.sh is not executable here"
	skipped=1
fi

say ""
say "== verdict"
if [ "$fail" -eq 0 ]; then
	say "  every refusal that could be driven here happened AND named its reason."
else
	say "  ⛔ a refusal did not happen, or happened without saying why."
fi

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
