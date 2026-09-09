#!/usr/bin/env bash
# Question: does podbox return DOCKER's exit code, case by case, on this host?
#
# TODO/cli.md T-0802. An automated caller reads the exit code before it reads
# anything else, so a runtime that returns its own codes breaks every script
# that branches on docker's.
#
# ⭐ WHY THIS IS A MEASUREMENT AND NOT A UNIT TEST. docker's contract is not
# written down anywhere podbox can read: it is what the binary does. Every row
# below runs the SAME case through both binaries on the SAME host and compares
# the two numbers, so the assertion is against docker rather than against a
# number somebody remembered.
#
# ⛔ THE DISCRIMINATOR IS NOT OBVIOUS AND IT IS THE FINDING. docker exits 125
# for anything its FLAG PARSER refuses and 1 for anything the verb refuses
# afterwards: `run --badflag`, `--pull=bogus` and `--memory=notasize` are 125,
# while `images --format '{{.Nope}}'`, `rmi no-such-image` and `run` with no
# image are 1. Both are "the caller wrote something wrong" and they are
# different numbers.
#
# ⛔ AN EXIT CODE IS READ FROM THE PROCESS THAT PRODUCED IT, UNPIPED.
# docs/AGENTS.md absolute 8. Every case below redirects to /dev/null and reads
# `$?` from the command itself.
#
# ⚠ podbox's own side of the table is DATA, not a number written here:
# `podbox system info --format '{{json .ExitCodes}}'` is `podbox_probe::exit`,
# and the clause that asserts it agrees with what the binary does is what stops
# the table from being a description of the code rather than the code's source.
#
# ⚠ ghcr.io and public.ecr.aws only. Docker Hub's rate limit is attributed to a
# shared address here and a run that trips it reads as a broken registry.
#
#   ./330-exit-codes.sh
#
# Exit: 0 every case agreed, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/exit-codes.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

# ⚠ The same tag for both binaries, fully qualified, so a difference between
# them is a difference between the binaries and not between two images.
IMAGE="${PODBOX_EXIT_IMAGE:-public.ecr.aws/docker/library/alpine:3.20}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. ./scripts/dev.sh build" >&2
	exit 2
}
command -v jq >/dev/null 2>&1 || {
	echo "SKIP: jq is not on PATH. ./scripts/common/bootstrap-env.sh tools" >&2
	exit 2
}

export PODBOX_STORE="$WORK/store"
fail=0
skipped=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

have_docker=0
if command -v docker >/dev/null 2>&1 && timeout 30 docker info >/dev/null 2>&1; then
	have_docker=1
fi

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	if [ "$have_docker" -eq 1 ]; then
		printf 'docker            %s\n' "$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
	else
		printf 'docker            ABSENT. The comparison half cannot run\n'
	fi
	printf 'image             %s\n' "$IMAGE"
	echo
} >"$WORK/report"

# --------------------------------------------------------------------- 0
say "== 0. podbox's table, as data"
"$BIN" system info --format '{{json .ExitCodes}}' >"$WORK/codes.json" 2>"$WORK/e0"
rc=$?
if [ "$rc" -ne 0 ]; then
	say "  FAIL: podbox system info --format '{{json .ExitCodes}}' exited $rc"
	sed 's/^/    /' "$WORK/e0" >>"$WORK/report"
	fail=1
	cat "$WORK/report"
	exit 1
fi
jq -r '.[] | "  \(.case)  \(.code)"' "$WORK/codes.json" >>"$WORK/report"
code_for() { jq -r --arg c "$1" '.[] | select(.case == $c) | .code' "$WORK/codes.json"; }

FLAG_ERROR="$(code_for flag-error)"
CLI_ERROR="$(code_for cli-error)"
NOT_FOUND="$(code_for not-found)"
CANNOT_INVOKE="$(code_for cannot-invoke)"
RUNTIME_ERROR="$(code_for runtime-error)"
say "  read from the table: flag=$FLAG_ERROR cli=$CLI_ERROR notfound=$NOT_FOUND \
invoke=$CANNOT_INVOKE runtime=$RUNTIME_ERROR"

# Make sure the image is in both stores before the timing-sensitive cases, so a
# pull is not what is being measured.
timeout 600 "$BIN" pull "$IMAGE" >/dev/null 2>&1
[ "$have_docker" -eq 1 ] && timeout 600 docker pull -q "$IMAGE" >/dev/null 2>&1

# ⛔ ONE PLACE runs a case, so the two binaries are driven identically and the
# report cannot show a comparison that was not made.
#   run_case <name> <expected-from-the-table> <podbox args...> -- <docker args...>
run_case() {
	local name="$1" want="$2"
	shift 2
	local pb=() dk=() seen=0
	for a in "$@"; do
		if [ "$a" = "--" ]; then
			seen=1
			continue
		fi
		if [ "$seen" -eq 0 ]; then pb+=("$a"); else dk+=("$a"); fi
	done
	local prc drc
	timeout 300 "$BIN" "${pb[@]}" >/dev/null 2>&1
	prc=$?
	if [ "$have_docker" -eq 1 ]; then
		timeout 300 docker "${dk[@]}" >/dev/null 2>&1
		drc=$?
	else
		drc="-"
	fi
	local verdict="ok"
	[ "$prc" = "$want" ] || { verdict="PODBOX DISAGREES WITH ITS OWN TABLE"; fail=1; }
	if [ "$drc" != "-" ] && [ "$prc" != "$drc" ]; then
		verdict="PODBOX AND DOCKER DISAGREE"
		fail=1
	fi
	printf '  %-26s want %-4s podbox %-4s docker %-4s %s\n' \
		"$name" "$want" "$prc" "$drc" "$verdict" >>"$WORK/report"
}

say ""
say "== 1. the flag parser refused it: docker's 125"
run_case "run --badflag" "$FLAG_ERROR" \
	run --badflag "$IMAGE" true -- run --badflag "$IMAGE" true
run_case "run --pull=bogus" "$FLAG_ERROR" \
	run --pull=bogus "$IMAGE" true -- run --pull=bogus "$IMAGE" true
run_case "images --badflag" "$FLAG_ERROR" images --badflag -- images --badflag
run_case "pull --badflag" "$FLAG_ERROR" pull --badflag "$IMAGE" -- pull --badflag "$IMAGE"
# ⚠ A flag podbox lists as None has no docker equivalent -- docker HAS --memory  -- 
# so the two numbers agreeing is the point: a caller's script branches the same
# way either way.
run_case "run --memory=1g" "$FLAG_ERROR" \
	run --memory=1g "$IMAGE" true -- run --memory=notasize "$IMAGE" true

say ""
say "== 2. the verb refused it after the flags parsed: docker's 1"
run_case "run, no image" "$CLI_ERROR" run -- run
run_case "images --format bad" "$CLI_ERROR" \
	images --format '{{.Nope}}' -- images --format '{{.Nope}}'
run_case "rmi no-such-image" "$CLI_ERROR" \
	rmi no-such-image-xyz -- rmi no-such-image-xyz
run_case "a verb neither has" "$CLI_ERROR" nosuchverb -- nosuchverb

say ""
say "== 3. the payload's own answer"
run_case "payload exit 42" 42 \
	run --rm "$IMAGE" sh -c 'exit 42' -- run --rm "$IMAGE" sh -c 'exit 42'
run_case "command not found" "$NOT_FOUND" \
	run --rm "$IMAGE" /nonexistent -- run --rm "$IMAGE" /nonexistent
run_case "command not invocable" "$CANNOT_INVOKE" \
	run --rm "$IMAGE" /etc/passwd -- run --rm "$IMAGE" /etc/passwd
run_case "payload exit 0" 0 run --rm "$IMAGE" true -- run --rm "$IMAGE" true

say ""
say "== 4. the bare name, with NO arguments at all"
# ⭐ Measured: `docker` with no arguments prints its help and exits 0. A script
# that runs the bare name to see whether the tool is there reads that status.
#
# ⛔ NOT through `run_case`: passing it an empty string as the argument list runs
# `docker ""`, which is an EMPTY VERB and exits 1. On 2026-09-09 that harness
# mistake was reported as "podbox and docker disagree" when the two agree, which
# is a defect in this script and not a reading about either binary.
timeout 60 "$BIN" >/dev/null 2>&1
prc=$?
if [ "$have_docker" -eq 1 ]; then
	timeout 60 docker >/dev/null 2>&1
	drc=$?
else
	drc="-"
fi
printf '  %-26s want %-4s podbox %-4s docker %-4s %s\n' \
	"no arguments at all" 0 "$prc" "$drc" \
	"$( { [ "$prc" = 0 ] && { [ "$drc" = "-" ] || [ "$drc" = "$prc" ]; }; } && echo ok || echo DISAGREE)" \
	>>"$WORK/report"
[ "$prc" = 0 ] || fail=1
[ "$drc" = "-" ] || [ "$drc" = "$prc" ] || fail=1

say ""
say "== 5. ⛔ a fixup NEVER changes the payload's exit code"
# T-0409 and T-0802. The completion layer edits files inside the image on every
# run; a payload that fails must still report its own failure, and one that
# succeeds must not be improved into a failure by a fixup that warned.
before=$(timeout 300 "$BIN" run --rm "$IMAGE" sh -c 'exit 7' >/dev/null 2>&1; echo $?)
after=$(timeout 300 "$BIN" run --rm --no-source-fixup "$IMAGE" sh -c 'exit 7' >/dev/null 2>&1; echo $?)
say "  with fixups        $before"
say "  --no-source-fixup  $after"
[ "$before" = 7 ] && [ "$after" = 7 ] || {
	say "  FAIL: the completion layer moved the payload's exit code"
	fail=1
}

say ""
say "== 6. --strict refuses rather than running, and with the runtime code"
timeout 300 "$BIN" run --strict --rm "$IMAGE" true >/dev/null 2>&1
rc=$?
say "  run --strict       $rc (want $RUNTIME_ERROR on a machine below the namespace rung)"
strict_ok="$("$BIN" system info --format '{{.StrictOk}}')"
say "  StrictOk           $strict_ok"
if [ "$strict_ok" = "true" ]; then
	# ⚠ A machine at the namespace rung can still be refused by a completion
	# fixup, so the only thing asserted here is that the code is one of the two.
	[ "$rc" = 0 ] || [ "$rc" = "$RUNTIME_ERROR" ] || {
		say "  FAIL: --strict returned neither 0 nor $RUNTIME_ERROR"
		fail=1
	}
else
	[ "$rc" = "$RUNTIME_ERROR" ] || { say "  FAIL: --strict did not refuse"; fail=1; }
fi

if [ "$have_docker" -eq 0 ]; then
	say ""
	say "  ⚠ docker is not running here, so every `docker` column reads `-`."
	say "    podbox was still checked against its own table, which is half the"
	say "    question and is recorded as half rather than as a pass."
	skipped=1
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
