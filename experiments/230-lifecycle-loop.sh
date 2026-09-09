#!/usr/bin/env bash
# Question: does the container lifecycle hold twenty times in a row, or only on
# the runs somebody happened to look at?
#
# TODO/milestones.md T-1105, TODO/supervise.md T-0607, T-0601 to T-0605.
#
# ⛔ TWENTY CONSECUTIVE PASSES, AND A SINGLE FAILURE FAILS THE MILESTONE. The
# prior art's own capture of this lifecycle contains a failed run beside a
# passing one, because it decided "running" by sleeping three seconds and
# looking. Retrying one failure is how a race becomes a published pass, so this
# script never retries an iteration and never continues past one.
#
# ⛔ NO SLEEP ANYWHERE, in this loop or in the implementation under it. Every
# wait here is on a condition: `start` returns when the payload has reached its
# execve, `stop` returns when the launcher has reaped it, and both are bounded.
# A `sleep` added to this file would remove the only thing it measures.
#
#   ./230-lifecycle-loop.sh [iterations]
#
# Exit: 0 every iteration passed, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/lifecycle-loop.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

IMAGE="${PODBOX_RUN_IMAGE:-ghcr.io/pkgforge-dev/archlinux:latest}"
N="${1:-20}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
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
	printf 'iterations asked  %s\n' "$N"
	echo
} >"$WORK/report"

# ⚠ Pulled and extracted ONCE, outside the loop. The loop measures the
# lifecycle, and a registry round trip inside it would measure the network.
if ! timeout 1800 "$BIN" pull "$IMAGE" >"$WORK/pull.log" 2>&1; then
	say "SKIP: could not pull $IMAGE"
	sed -n '1,3p' "$WORK/pull.log" | sed 's/^/  /' >>"$WORK/report"
	cat "$WORK/report"
	exit 2
fi
timeout 1800 "$BIN" extract "$IMAGE" >/dev/null 2>&1

say "== the loop: create, start, ps, exec, stop, rm"
say "  ⛔ no sleep anywhere; every wait is on a condition with a bound"
passed=0
i=1
while [ "$i" -le "$N" ]; do
	name="loop$i"
	step=""
	# create: writes a record and starts nothing.
	if ! timeout 300 "$BIN" create --name "$name" "$IMAGE" /bin/sleep 3600 >/dev/null 2>"$WORK/e"; then
		step="create"
	# start: returns when the payload has reached its execve.
	elif ! timeout 300 "$BIN" start "$name" >/dev/null 2>"$WORK/e"; then
		step="start"
	fi
	if [ -z "$step" ]; then
		# ⭐ ps shows it RUNNING, read out of podbox's own table and never
		# from /proc. This is the assertion the prior art made with a sleep.
		status="$(timeout 60 "$BIN" ps --format '{{.Names}} {{.Status}}' 2>/dev/null | grep "^$name ")"
		case "$status" in
		"$name Up"*) ;;
		*) step="ps" ;;
		esac
	fi
	if [ -z "$step" ]; then
		marker="$(timeout 300 "$BIN" exec "$name" /bin/sh -c 'echo marker' 2>/dev/null)"
		[ "$marker" = "marker" ] || step="exec"
	fi
	if [ -z "$step" ]; then
		timeout 120 "$BIN" stop "$name" >/dev/null 2>"$WORK/e" || step="stop"
	fi
	if [ -z "$step" ]; then
		# ⛔ 143 and not 0: a SIGTERMed payload is 128+15, and a runtime that
		# rounds that to a clean exit lies in the field a caller reads first.
		after="$(timeout 60 "$BIN" ps -a --format '{{.Names}} {{.Status}}' 2>/dev/null | grep "^$name ")"
		case "$after" in
		"$name Exited (143)") ;;
		*) step="status after stop [$after]" ;;
		esac
	fi
	if [ -z "$step" ]; then
		timeout 60 "$BIN" rm "$name" >/dev/null 2>"$WORK/e" || step="rm"
	fi
	if [ -n "$step" ]; then
		# ⛔ STOP HERE. Not a retry, and not a continue: the milestone is twenty
		# CONSECUTIVE passes, so the number that matters is how many happened
		# before the first failure.
		say "  iteration $i FAILED at $step"
		sed -n '1,3p' "$WORK/e" 2>/dev/null | sed 's/^/    /' >>"$WORK/report"
		fail=1
		break
	fi
	passed=$((passed + 1))
	i=$((i + 1))
done
say "  consecutive passes: $passed of $N"

# --------------------------------------------------------------------- 2
say ""
say '== T-0604: a launcher that is killed leaves `dead`, not a made-up code'
# ⛔ The half nothing else drives. A launcher killed with SIGKILL cannot write
# an exit code, and the only honest answer afterwards is that podbox was not
# watching. A runtime that reported 0 here would be lying in the field a caller
# reads first.
if [ "$fail" -ne 0 ]; then
	say "  SKIP: the loop above did not finish"
	skipped=1
else
	timeout 300 "$BIN" run -d --name deadprobe "$IMAGE" /bin/sleep 3600 >/dev/null 2>&1
	rc=$?
	launcher="$(timeout 60 "$BIN" inspect --format '{{.LauncherPid}}' deadprobe 2>/dev/null)"
	# ⚠ Whether there IS one, not which. A tracked reading that carries a pid
	# differs on every run and can therefore never reproduce.
	say "  run -d rc=$rc, a launcher pid was recorded: $([ -n "$launcher" ] && [ "$launcher" != 0 ] && echo yes || echo no)"
	if [ "$rc" -ne 0 ] || [ -z "$launcher" ] || [ "$launcher" = "0" ]; then
		say "  SKIP: no detached container to kill"
		skipped=1
	else
		kill -9 "$launcher" 2>/dev/null
		state="$(timeout 60 "$BIN" ps -a --format '{{.Names}} {{.State}}' 2>/dev/null | grep '^deadprobe ')"
		code="$(timeout 60 "$BIN" inspect --format '{{.ExitCode}}' deadprobe 2>/dev/null)"
		noticed="$(timeout 60 "$BIN" inspect --format '{{.Noticed}}' deadprobe 2>/dev/null)"
		say "  after killing the launcher: [$state]"
		# ⚠ The same rule for the timestamp: that one was RECORDED is the
		# assertion, and its value is this run's.
		say "  exit code reported:         [$code]   a time was noticed: $([ -n "$noticed" ] && [ "$noticed" != "-" ] && echo yes || echo no)"
		case "$state" in
		"deadprobe dead") ;;
		*) say "  FAIL: a killed launcher did not leave the container dead"; fail=1 ;;
		esac
		[ "$code" = "-" ] || {
			say "  FAIL: podbox reported an exit code it never measured"
			fail=1
		}
		[ -n "$noticed" ] && [ "$noticed" != "-" ] || {
			say "  FAIL: nothing recorded when the exit was noticed"
			fail=1
		}
		# ⛔ And `wait` refuses rather than printing a number.
		wrc=0
		timeout 60 "$BIN" wait deadprobe >/dev/null 2>&1 || wrc=$?
		say "  podbox wait on it:          rc=$wrc (want non-zero)"
		[ "$wrc" -ne 0 ] || { say "  FAIL: wait invented an exit code"; fail=1; }
		# ⚠ The payload itself is still running: the launcher was killed, not it.
		# Cleaned up here so the loop leaves nothing behind.
		payload="$(timeout 60 "$BIN" inspect --format '{{.Pid}}' deadprobe 2>/dev/null)"
		[ -n "$payload" ] && [ "$payload" != "0" ] && kill -9 "$payload" 2>/dev/null
		timeout 60 "$BIN" rm -f deadprobe >/dev/null 2>&1
	fi
fi

# --------------------------------------------------------------------- 3
say ""
say "== T-0603: the spawn path never grew a thread"
# ⛔ `PR_SET_PDEATHSIG` fires on the CREATING THREAD's exit, so the path that
# clones, chroots and execs has to be the process. Read off a real detached
# launcher rather than asserted in a comment.
if [ "$fail" -ne 0 ]; then
	say "  SKIP: an earlier clause failed"
	skipped=1
else
	timeout 300 "$BIN" run -d --name threadprobe "$IMAGE" /bin/sleep 3600 >/dev/null 2>&1
	lp="$(timeout 60 "$BIN" inspect --format '{{.LauncherPid}}' threadprobe 2>/dev/null)"
	if [ -z "$lp" ] || [ "$lp" = "0" ] || [ ! -d "/proc/$lp/task" ]; then
		say "  SKIP: could not read the launcher's task list"
		skipped=1
	else
		tasks="$(ls /proc/"$lp"/task 2>/dev/null | wc -l)"
		say "  the launcher's threads: $tasks (want 1)"
		[ "$tasks" -eq 1 ] || { say "  FAIL: the launcher is threaded"; fail=1; }
	fi
	timeout 60 "$BIN" rm -f threadprobe >/dev/null 2>&1
fi

# --------------------------------------------------------------------- 4
say ""
say "== T-0605: the log was captured, from descriptors opened before the chroot"
if [ "$fail" -ne 0 ]; then
	say "  SKIP: an earlier clause failed"
	skipped=1
else
	timeout 300 "$BIN" run -d --name logprobe "$IMAGE" \
		/bin/sh -c 'echo out; echo err >&2' >/dev/null 2>&1
	# ⚠ Waited on, not slept on: the payload is short and `wait` returns when
	# the launcher reaped it.
	timeout 60 "$BIN" wait logprobe >/dev/null 2>&1
	got="$(timeout 60 "$BIN" logs logprobe 2>/dev/null | tr '\n' ' ')"
	say "  podbox logs logprobe: [$got]"
	case "$got" in
	*out*err*) ;;
	*) say "  FAIL: the log did not carry both streams"; fail=1 ;;
	esac
	timeout 60 "$BIN" rm -f logprobe >/dev/null 2>&1
fi

say ""
say "  containers left behind: $(timeout 60 "$BIN" ps -aq 2>/dev/null | wc -l) (want 0)"

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

# ⛔ A single failure fails the milestone.
[ "$fail" -eq 0 ] || exit 1
[ "$passed" -eq "$N" ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
