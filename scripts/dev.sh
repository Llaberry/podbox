#!/usr/bin/env bash
# dev.sh - get a fresh session to the point of writing code as fast as possible.
#
# ⭐ EVERY SESSION RUNS IN A NEW MACHINE AND PAYS THE SAME COLD COST TWICE OVER:
# the tools the last session installed are gone, and `target/` is empty, so the
# whole dependency graph, `ring` and its C included, is compiled again. Measured
# on 2026-09-09 and recorded in `experiments/results/session-startup.txt`.
#
# ⛔ THE POINT IS THAT NONE OF IT BLOCKS THE READING. `docs/AGENTS.md` opens by
# sending a session to `TODO/PROGRESS.md` and the routing table, which is minutes
# of reading that needs no toolchain. This starts the environment and the build
# BEHIND that reading and gets out of the way.
#
#   ./scripts/dev.sh              start it in the background and return at once
#   ./scripts/dev.sh status       is it running, done or failed
#   ./scripts/dev.sh wait         block until it finishes, then report
#   ./scripts/dev.sh build        one foreground build, for after a source change
#   ./scripts/dev.sh check        build, fmt, clippy, tests and the gate
#
# ⚠ IT IS SAFE TO RUN TWICE. A second `dev.sh` while one is running attaches to
# the first rather than starting a competing cargo: two `cargo build`s on one
# target directory block on the same lock and the second looks like a hang.
#
# Exit: 0 the requested thing succeeded, 1 it failed, 2 it could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
cd "$REPO" || exit 2

# ⚠ Under the repository, not /tmp: a session that loses /tmp between turns
# would lose the log that says why the build failed. `.gitignore` carries it.
STATE="$REPO/.dev"
LOG="$STATE/build.log"
PIDFILE="$STATE/pid"
STATUSFILE="$STATE/status"
STAMP="$STATE/last-build-inputs"

TARGET="${PODBOX_TARGET:-x86_64-unknown-linux-musl}"
BIN="$REPO/target/$TARGET/release/podbox"

mkdir -p "$STATE"

# What a build depends on. ⚠ Deliberately not `find .`: `target/` and
# `references/` are enormous and neither is an input to the build.
inputs_digest() {
	{
		find crates -name '*.rs' -newermt '1970-01-01' -printf '%p %T@ %s\n' 2>/dev/null
		cat Cargo.toml Cargo.lock rust-toolchain.toml .cargo/config.toml 2>/dev/null
	} | sha256sum | cut -d' ' -f1
}

running() {
	[ -f "$PIDFILE" ] || return 1
	local pid
	pid="$(cat "$PIDFILE" 2>/dev/null)"
	[ -n "$pid" ] || return 1
	kill -0 "$pid" 2>/dev/null
}

# ---------------------------------------------------------------- the worker
#
# ⛔ Bootstrap FIRST and the build second, because the build needs `zig` for
# `ring`'s C and would fail without it in a way that reads as a code error.
do_work() {
	{
		echo "== dev.sh started $(date -u +%Y-%m-%dT%H:%M:%SZ)"
		local t0 t1 t2
		t0=$(date +%s)
		echo "== bootstrap"
		"$REPO/scripts/common/bootstrap-env.sh" 2>&1
		local boot_rc=$?
		t1=$(date +%s)
		echo "== bootstrap finished in $((t1 - t0))s, rc=$boot_rc"
		if [ "$boot_rc" -ne 0 ]; then
			echo "FAILED: bootstrap"
			echo failed >"$STATUSFILE"
			return 1
		fi
		echo "== cargo build --release --target $TARGET"
		cargo build --release --target "$TARGET" 2>&1
		local build_rc=$?
		t2=$(date +%s)
		echo "== build finished in $((t2 - t1))s, rc=$build_rc"
		echo "== total $((t2 - t0))s"
		if [ "$build_rc" -ne 0 ]; then
			echo "FAILED: cargo build"
			echo failed >"$STATUSFILE"
			return 1
		fi
		inputs_digest >"$STAMP"
		echo ready >"$STATUSFILE"
		echo "== ok. $BIN"
	} >>"$LOG" 2>&1
}

start_bg() {
	if running; then
		echo "dev.sh: already running (pid $(cat "$PIDFILE")). Attaching rather than"
		echo "        starting a second cargo: two builds on one target directory"
		echo "        block on the same lock and the second looks like a hang."
		echo "        log: ${LOG#"$REPO"/}"
		return 0
	fi
	: >"$LOG"
	echo running >"$STATUSFILE"
	# ⛔ `setsid` and a full detach, so the build survives the shell that
	# started it. A session's turns are separate processes.
	setsid bash -c "$(declare -f do_work inputs_digest); \
		REPO='$REPO' STATE='$STATE' LOG='$LOG' STATUSFILE='$STATUSFILE' \
		STAMP='$STAMP' TARGET='$TARGET' BIN='$BIN' do_work" \
		</dev/null >/dev/null 2>&1 &
	echo $! >"$PIDFILE"
	cat <<EOF
dev.sh: the environment and the build are running in the background.

  log     ${LOG#"$REPO"/}
  status  ./scripts/dev.sh status
  wait    ./scripts/dev.sh wait

⭐ Do not wait for it. Read these now, in this order, which is what
   docs/AGENTS.md's routing table says and needs no toolchain:

     1. TODO/PROGRESS.md          the state, the work order, the open questions
     2. docs/AGENTS.md            the router, and the absolutes
     3. the TODO/ entry your task names, in full

   By the time that reading is done this will have finished.
EOF
}

case "${1:-start}" in
start)
	start_bg
	;;
status)
	if running; then
		echo "running (pid $(cat "$PIDFILE"))"
		tail -3 "$LOG" 2>/dev/null | sed 's/^/  /'
		exit 0
	fi
	st="$(cat "$STATUSFILE" 2>/dev/null || echo unknown)"
	echo "$st"
	case "$st" in
	ready)
		[ -x "$BIN" ] && echo "  $BIN"
		# ⚠ Says whether the build is STALE, rather than only whether it ran.
		# A `ready` that predates the last edit is the answer that misleads.
		if [ -f "$STAMP" ] && [ "$(cat "$STAMP")" != "$(inputs_digest)" ]; then
			echo "  ⚠ stale: a source file has changed since this build."
			echo "    ./scripts/dev.sh build"
			exit 1
		fi
		exit 0
		;;
	failed)
		echo "  the last lines of ${LOG#"$REPO"/}:"
		tail -12 "$LOG" 2>/dev/null | sed 's/^/  /'
		exit 1
		;;
	*) exit 2 ;;
	esac
	;;
wait)
	# ⚠ Bounded. RULES.md section 8: a runtime whose audience is automated may
	# not wait unbounded, and neither may its build script.
	limit="${PODBOX_DEV_WAIT:-1800}"
	waited=0
	while running && [ "$waited" -lt "$limit" ]; do
		sleep 2
		waited=$((waited + 2))
	done
	if running; then
		echo "dev.sh: still running after ${limit}s. It is not abandoned; raise" >&2
		echo "        \$PODBOX_DEV_WAIT or watch ${LOG#"$REPO"/}" >&2
		exit 2
	fi
	exec "$0" status
	;;
build)
	# ⛔ Foreground, for after a source change. It does NOT bootstrap: by the
	# time a session is editing code the environment is already up, and paying
	# for an apt check on every edit is the cost this script exists to remove.
	cargo build --release --target "$TARGET" || exit 1
	inputs_digest >"$STAMP"
	echo ready >"$STATUSFILE"
	echo "dev.sh: $BIN"
	;;
check)
	# What a change has to pass before it is committed, in the order that fails
	# cheapest first.
	rc=0
	for step in \
		"cargo fmt --all --check" \
		"cargo clippy --workspace --all-targets -- -D warnings" \
		"cargo build --release --target $TARGET" \
		"cargo test --workspace" \
		"./scripts/check-todo.py" \
		"./scripts/common/check-markers.sh"; do
		printf '== %s\n' "$step"
		# ⛔ The status is read from the step itself, unpiped. docs/AGENTS.md
		# absolute 8: piping a check into anything reports the pipeline's
		# status, so a guard that failed reads as green.
		if ! $step; then
			echo "   FAILED: $step"
			rc=1
		fi
	done
	[ "$rc" -eq 0 ] && inputs_digest >"$STAMP"
	exit "$rc"
	;;
-h | --help | help)
	sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
	;;
*)
	echo "dev.sh: unknown command ${1:?}" >&2
	sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//' >&2
	exit 2
	;;
esac
