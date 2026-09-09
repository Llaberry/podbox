#!/usr/bin/env bash
# Question: does `podbox run -d` RETURN before its payload ends, when the caller
# reads its stdout the way every script does?
#
# TODO/supervise.md T-0608. ⛔ REPRODUCE BEFORE CHANGING ANYTHING, which is what
# this script was written to do, and what it found is not what the entry's two
# written-down candidates said.
#
# ⭐ THE CALLER'S OWN STDOUT IS THE INSTRUMENT, and it is why `id="$(...)"` here
# is not incidental style. `podbox run -d` prints a container id, so every
# caller captures it, and command substitution reads that pipe TO EOF. A
# `clone`d launcher that never execs keeps every descriptor its parent had,
# including that pipe, for the container's whole life: the id is written, the
# process that wrote it exits, and the shell still waits. Measured on
# 2026-09-09 before the fix: `/proc/<launcher>/fd/1` was the caller's
# `pipe:[...]`, and a `sleep 25` payload took 25 s to hand back an id it had
# printed in the first second.
#
# ⛔ WHAT THIS MAKES OF THE ORIGINAL SIGHTING. "`run -d` returned an id and one
# second later `inspect` read `exited`, pid 0, launcher pid 0, code 0" was
# accurate and the timing was the other way round: the caller was released when
# the payload ENDED, and by then the launcher had written the terminal record.
# ⚠ Neither candidate in T-0608 was it: not the readiness bound reported as an
# exit, and not `reconcile` racing `start`.
#
# ⚠ THE LOAD IS KEPT AND IS NOT THE CAUSE. Both sightings were on a machine with
# all four CPUs busy and the entry named that as the one condition the passing
# runs did not share. It reproduces on an idle machine too, in one iteration;
# the load stays because a scheduler under pressure is where a handoff bug would
# hide, and a measurement that removed the condition it was told about would be
# answering a different question. `PODBOX_LOAD_CMD` replaces the burners.
#
#   ./340-detached-stdio.sh [iterations]
#
# Exit: 0 every iteration returned promptly and read `running` with a launcher
#       pid, 1 one did not and the transcript carries it, 2 could not be taken.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/detached-stdio.txt"
WORK="$(mktemp -d)"

# ⛔ The burners are killed on EVERY exit, including the one a `set -u` failure
# takes. A leaked `while :; do :; done` costs the rest of the session a CPU.
BURNERS=""
cleanup() {
	for p in $BURNERS; do kill -9 "$p" 2>/dev/null; done
	rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

IMAGE="${PODBOX_RUN_IMAGE:-ghcr.io/pkgforge-dev/archlinux:latest}"
N="${1:-40}"
# ⚠ Long enough that a blocking `run -d` is unmistakable and short enough that a
# reproduction does not cost the session: the failure is a WAIT, so the payload's
# own duration is the size of the evidence.
PAYLOAD_SECONDS=20
# ⛔ The bound `run -d` must return within. A detached start does a fork and two
# table writes; anything near the payload's own duration is the defect.
PROMPT_SECONDS=5

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. ./scripts/dev.sh build" >&2
	exit 2
}

export PODBOX_STORE="$WORK/store"
fail=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

cpus="$(nproc 2>/dev/null || echo 1)"
{
	echo "== conditions"
	printf 'date               %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel        %s\n' "$(uname -r)"
	printf 'podbox             %s\n' "$("$BIN" version)"
	printf 'rung podbox enters %s\n' "$("$BIN" system info --format '{{.EnteredRung}}')"
	printf 'image              %s\n' "$IMAGE"
	printf 'payload            /bin/sleep %s\n' "$PAYLOAD_SECONDS"
	printf 'prompt bound       %s s\n' "$PROMPT_SECONDS"
	printf 'iterations asked   %s\n' "$N"
	printf 'cpus               %s\n' "$cpus"
	printf 'load               %s\n' "${PODBOX_LOAD_CMD:-$cpus busy-loop shells}"
	echo
} >"$WORK/report"

# ⚠ Pulled and extracted ONCE, outside the loop and BEFORE the load starts. The
# loop measures the handoff, and a registry round trip inside it would measure
# the network instead.
if ! timeout 1800 "$BIN" pull "$IMAGE" >"$WORK/pull.log" 2>&1; then
	say "SKIP: could not pull $IMAGE"
	sed -n '1,3p' "$WORK/pull.log" | sed 's/^/  /' >>"$WORK/report"
	cat "$WORK/report"
	exit 2
fi
timeout 1800 "$BIN" extract "$IMAGE" >/dev/null 2>&1

# ------------------------------------------------------------------- the load
if [ -n "${PODBOX_LOAD_CMD:-}" ]; then
	sh -c "$PODBOX_LOAD_CMD" >"$WORK/load.log" 2>&1 &
	BURNERS="$!"
else
	i=1
	while [ "$i" -le "$cpus" ]; do
		# shellcheck disable=SC2016
		sh -c 'while :; do :; done' &
		BURNERS="$BURNERS $!"
		i=$((i + 1))
	done
fi

say "== the loop: capture the id the way a script does, then read the record"
say "  ⛔ id=\"\$(podbox run -d ...)\": command substitution reads to EOF, so a"
say "     launcher holding the caller's stdout blocks it for the whole life of"
say "     the container. That is the defect, and this is the instrument."

worst=0
passed=0
i=1
while [ "$i" -le "$N" ]; do
	name="det$i"
	before="$(date +%s)"
	# ⛔ COMMAND SUBSTITUTION, deliberately. Redirecting stdout to a file would
	# not reproduce it: a file has no reader waiting for EOF.
	id="$(timeout 300 "$BIN" run -d --name "$name" "$IMAGE" \
		/bin/sleep "$PAYLOAD_SECONDS" 2>"$WORK/e.$i")"
	rc=$?
	took=$(($(date +%s) - before))
	[ "$took" -le "$worst" ] || worst="$took"
	if [ "$rc" -ne 0 ] || [ -z "$id" ]; then
		say "  iteration $i: run -d exited $rc after ${took}s with id [$id]"
		sed -n '1,4p' "$WORK/e.$i" | sed 's/^/    /' >>"$WORK/report"
		fail=1
		break
	fi
	if [ "$took" -ge "$PROMPT_SECONDS" ]; then
		say "  ⛔ iteration $i REPRODUCED IT: run -d took ${took}s to hand back an"
		say "     id for a payload that runs ${PAYLOAD_SECONDS}s. It did not detach."
		say "     the launcher's own descriptors:"
		# ⚠ Best effort: by the time this runs the launcher may be gone, which
		# is itself the symptom. An empty listing is reported, not hidden.
		L="$(timeout 60 "$BIN" inspect --format '{{.LauncherPid}}' "$name" 2>/dev/null)"
		ls -l "/proc/$L/fd" 2>&1 | sed -n '1,8p' | sed 's/^/       /' >>"$WORK/report"
		fail=1
	fi
	# ⛔ ONE inspect, four fields, so all four describe the same read of the
	# table rather than four reads a scheduler could interleave.
	row="$(timeout 60 "$BIN" inspect \
		--format '{{.State}} {{.Pid}} {{.LauncherPid}} {{.ExitCode}}' "$name" 2>/dev/null)"
	state="${row%% *}"
	if [ "$state" != running ]; then
		say "  ⛔ iteration $i: [$row] for a payload that runs ${PAYLOAD_SECONDS}s"
		say "     podbox run -d said, on stderr:"
		sed -n '1,6p' "$WORK/e.$i" | cut -c1-110 | sed 's/^/       /' >>"$WORK/report"
		say "     podbox logs $name:"
		timeout 60 "$BIN" logs "$name" 2>&1 | sed -n '1,6p' | cut -c1-110 |
			sed 's/^/       /' >>"$WORK/report"
		fail=1
	fi
	[ "$fail" -eq 0 ] || break
	# ⚠ The launcher pid is asserted as PRESENT rather than by value: a tracked
	# reading that carries a pid differs from itself on every run.
	launcher="$(printf '%s' "$row" | awk '{print $3}')"
	if [ -z "$launcher" ] || [ "$launcher" = "0" ]; then
		say "  ⛔ iteration $i: state running and NO launcher pid: [$row]"
		fail=1
		break
	fi
	timeout 120 "$BIN" rm -f "$name" >/dev/null 2>&1
	passed=$((passed + 1))
	i=$((i + 1))
done
say "  detached starts that returned promptly and read running: $passed of $N"
say "  the longest \`run -d\` took: ${worst}s, against a bound of ${PROMPT_SECONDS}s"

# ⛔ Whatever happened, nothing is left running: a `sleep 20` per iteration
# outlives this script otherwise, and the next measurement inherits the load.
ids="$(timeout 60 "$BIN" ps -aq 2>/dev/null | tr '\n' ' ')"
# shellcheck disable=SC2086
[ -z "$ids" ] || timeout 120 "$BIN" rm -f $ids >/dev/null 2>&1

say ""
say "== verdict"
if [ "$fail" -eq 0 ]; then
	say "  T-0608 does not reproduce in $N detached starts under $cpus busy CPUs."
	say "  Every one handed its id back inside the bound and read \`running\`."
else
	say "  T-0608 REPRODUCED. The transcript above carries the timing, the"
	say "  record and the stderr of the iteration that did it."
fi

cat "$WORK/report"
{
	printf '# T-0608: does `podbox run -d` return before its payload ends?\n'
	printf '# TODO/supervise.md T-0608. Taken %s on kernel %s\n' \
		"$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# ⛔ the caller captures the id with $( ), which reads to EOF\n'
	printf 'iterations         %s\n' "$N"
	printf 'cpus               %s\n' "$cpus"
	printf 'load               %s\n' "${PODBOX_LOAD_CMD:-busy-loop shells, one per cpu}"
	printf 'payload_seconds    %s\n' "$PAYLOAD_SECONDS"
	printf 'prompt_bound_s     %s\n' "$PROMPT_SECONDS"
	printf 'prompt_and_running %s\n' "$passed"
	printf 'slowest_run_d_s    %s\n' "$worst"
	printf 'reproduced         %s\n' "$fail"
} >"$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"
exit "$fail"
