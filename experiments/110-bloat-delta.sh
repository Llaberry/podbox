#!/usr/bin/env bash
# Question: how large is the release artefact, what is in it, and how much did
# the dependency under test add?
#
# TODO/deps.md T-0910. ⛔ A dependency that lands without a before-and-after
# number has not landed, and without a committed baseline there is no "before".
#
#   ./110-bloat-delta.sh baseline       the "before": no dependencies at all
#   ./110-bloat-delta.sh <area>         the "after": one entry from TODO/deps.md
#
# ⛔ THE CANDIDATE MUST BE REACHABLE FROM `main`, OR THIS MEASURES NOTHING.
# `lto = true` deletes a dependency nothing calls, and the delta then reads
# zero for a crate that is very much in the tree. Measured on 2026-09-08: a
# scaffold in `podbox-image` that `main` never reached moved the binary by 0
# bytes with `ring` and `rustls` compiled and linked. Wire the scaffold into a
# path `main` can reach behind a condition the optimizer cannot fold, which in
# practice means an environment variable:
#
#     if std::env::var_os("PODBOX_SWEEP").is_some() { podbox_image::probe(...); }
#
# The check below refuses a run where third-party crates are present and the
# delta is zero, because that is this mistake and not a free dependency.
#
# Each run writes experiments/results/bloat-<area>.txt and prints its
# conditions. An <area> other than `baseline` is also differenced against the
# committed baseline, so the number an entry closes with is a delta and not a
# total somebody has to subtract by hand.
#
# ⭐ THE CEILING LIVES HERE AND NOWHERE ELSE. It is on the TOTAL, not on the
# delta: a delta ceiling permits an unbounded number of small dependencies,
# which is how a binary gets large without any single decision being wrong.
# The gate workflow calls this script rather than carrying its own copy of the
# number, because a value in two places drifts and the copy a reader trusts is
# the wrong one. Check 17 of scripts/check-todo.py holds that there is exactly
# one declaration of it in the tree.
#
# Exit: 0 measured and under the ceiling, 1 over the ceiling or the build
#       failed, 2 could not run.
set -uo pipefail

CEILING_BYTES=8000000

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
TARGET=x86_64-unknown-linux-musl
BIN="$REPO/target/$TARGET/release/podbox"
AREA="${1:-baseline}"
OUT="$REPO/experiments/results/bloat-$AREA.txt"
BASE="$REPO/experiments/results/bloat-baseline.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

case "$AREA" in
*[!a-z0-9-]*) echo "SKIP: <area> must be lowercase letters, digits and dashes" >&2; exit 2 ;;
esac

command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo is not on PATH" >&2; exit 2; }

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'rustc             %s\n' "$(rustc --version)"
printf 'cargo             %s\n' "$(cargo --version)"
printf 'target            %s\n' "$TARGET"
printf 'area              %s\n' "$AREA"
printf 'ceiling           %s bytes, declared in this script and nowhere else\n' "$CEILING_BYTES"
echo

# ------------------------------------------------------------------ the total
#
# ⛔ The shipping profile, unmodified: `strip = "symbols"` and `lto = true` are
# what the artefact is, so they are what is measured. The breakdown below needs
# symbols and is therefore taken from a SECOND build, in its own target
# directory, so it cannot replace the artefact this number came from.
echo "== the total, from the shipping profile"
# ⛔ The exit code is read from cargo, not from a pipeline. `cargo ... | tail -3`
# reports tail's status, so a failed build reads as green, and it also throws
# away the diagnostic: measured on 2026-09-08, a candidate whose scaffold named
# a type that had moved showed only "could not compile", with the line that
# said which type three lines above the cut.
if ! cargo build --release --target "$TARGET" --manifest-path "$REPO/Cargo.toml" \
	>"$WORK/build.log" 2>&1; then
	echo "FAIL: the release build did not succeed" >&2
	grep -E "^(error|warning: unused)" -A 6 "$WORK/build.log" | head -40 >&2
	exit 1
fi
tail -2 "$WORK/build.log"
[ -f "$BIN" ] || { echo "SKIP: $BIN was not produced" >&2; exit 2; }
total="$(stat -c%s "$BIN")"
printf '  %s\n' "$BIN"
printf '  total_bytes %s\n' "$total"
printf '  headroom    %s\n' "$((CEILING_BYTES - total))"

interp="$(readelf -l "$BIN" 2>/dev/null | grep -c INTERP)"
printf '  PT_INTERP   %s\n' "$interp"

# ⭐ THE CONTROL, and it is not optional. A zero delta has two causes that look
# identical in a size: a scaffold `main` cannot reach, which `lto` deleted and
# which measured nothing; and a candidate genuinely smaller than the change a
# linked, padded binary can show. Running the scaffold tells them apart, so the
# instrument asks rather than assuming. TODO/probe.md T-0102 is the same
# discipline applied to a different measurement.
scaffold_said="$(PODBOX_SWEEP=1 timeout 60 "$BIN" version 2>/dev/null | head -1)"
case "$scaffold_said" in
podbox\ *) scaffold_said="" ;;  # only the version line: the scaffold said nothing
esac
printf '  scaffold    %s\n' "${scaffold_said:-(said nothing)}"

# ------------------------------------------------------------- the breakdown
echo
echo "== what is in it"
bloat_status=""
if ! cargo bloat --version >/dev/null 2>&1; then
	# ⛔ Reported as a skip, never as a pass. A tool that is not installed
	# means nothing about its subject was verified.
	bloat_status="not taken: cargo-bloat is not installed (cargo install cargo-bloat)"
	echo "  SKIP $bloat_status"
else
	# ⚠ `strip = "symbols"` leaves cargo-bloat nothing to read, so the
	# breakdown comes from the same profile with stripping turned off, built
	# into its own directory. The SIZES BELOW ARE THEREFORE NOT THE ARTEFACT'S;
	# the proportions are what the breakdown is for, and the artefact's own
	# size is the total above.
	CARGO_TARGET_DIR="$WORK/bloat-target" \
	CARGO_PROFILE_RELEASE_STRIP=none \
		cargo bloat --release --target "$TARGET" --manifest-path "$REPO/Cargo.toml" \
		--bin podbox -n 20 > "$WORK/bloat.txt" 2>"$WORK/bloat.err"
	if [ -s "$WORK/bloat.txt" ]; then
		sed 's/^/  /' "$WORK/bloat.txt"
		bloat_status="taken from an unstripped build of the release profile"
	else
		bloat_status="not taken: cargo bloat produced nothing"
		echo "  SKIP $bloat_status"
		sed 's/^/    /' "$WORK/bloat.err" >&2
	fi
fi

echo
echo "== the dependency tree"
deps="$(cargo tree --edges normal --prefix none --manifest-path "$REPO/Cargo.toml" 2>/dev/null \
	| awk 'NF' | sort -u | grep -vc '^podbox-')"
printf '  third-party crates in the normal dependency graph: %s\n' "$deps"

# ------------------------------------------------------------------ the delta
delta_line="the baseline itself; there is nothing before it"
unmeasured=0
if [ "$AREA" != "baseline" ]; then
	if [ -f "$BASE" ]; then
		before="$(awk '/^total_bytes /{print $2}' "$BASE")"
		if [ -n "$before" ]; then
			delta_line="$((total - before)) bytes against $before in $(basename "$BASE")"
		else
			delta_line="- (the baseline file carries no total_bytes line)"
		fi
	else
		delta_line="- (no committed baseline; run ./110-bloat-delta.sh baseline first)"
	fi
fi
echo
echo "== the delta"
printf '  %s\n' "$delta_line"

# ⛔ A zero delta with a dependency present is this script failing to measure,
# not a crate that costs nothing. See the header.
unmeasured=0
if [ "$AREA" != "baseline" ] && [ "${deps:-0}" -gt 0 ] && [ -n "${before:-}" ] &&
	[ "$total" -eq "$before" ]; then
	if [ -z "$scaffold_said" ]; then
		unmeasured=1
		echo
		echo "FAIL: $deps third-party crate(s) are in the graph, the binary did not" >&2
		echo "      move by one byte, AND the scaffold said nothing when run. That is" >&2
		echo "      a scaffold \`main\` cannot reach, deleted by lto, not a dependency" >&2
		echo "      that is free. Wire it into a reachable path and re-run." >&2
	else
		# ⭐ The control answered, so the reading is real: the candidate is
		# smaller than this instrument can show. That is a result, and a
		# useful one, so it is recorded rather than turned into a failure.
		delta_line="0 bytes against $before: BELOW THIS INSTRUMENT'S RESOLUTION."
		delta_line="$delta_line The scaffold ran and printed \"$scaffold_said\", so the"
		delta_line="$delta_line candidate is linked and reachable and still moved nothing"
		delta_line="$delta_line a padded, stripped, lto'd binary can show."
	fi
fi

# ------------------------------------------------------------------ the record
{
	printf '# cargo bloat and the release total, area=%s\n' "$AREA"
	printf '# TODO/deps.md T-0910. One machine, one day.\n'
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'rustc             %s\n' "$(rustc --version)"
	printf 'cargo             %s\n' "$(cargo --version)"
	printf 'target            %s\n' "$TARGET"
	printf 'profile           release: lto=true opt-level=z codegen-units=1 strip=symbols panic=abort\n'
	printf 'total_bytes %s\n' "$total"
	printf 'ceiling_bytes %s\n' "$CEILING_BYTES"
	printf 'headroom_bytes %s\n' "$((CEILING_BYTES - total))"
	printf 'pt_interp %s\n' "$interp"
	printf 'third_party_crates %s\n' "$deps"
	printf 'delta             %s\n' "$delta_line"
	printf 'unmeasured        %s\n' "$unmeasured"
	printf 'scaffold_said     %s\n' "${scaffold_said:-(nothing)}"
	printf 'breakdown         %s\n' "$bloat_status"
	echo
	# ⭐ THE SCAFFOLD TRAVELS WITH THE NUMBER. `docs/AGENTS.md`'s fourth
	# absolute wants every measurement to ship with what took it, and for a
	# sweep that is the dependency lines plus the code that reaches them: the
	# same crates behind a scaffold that exercises less of the surface measure
	# smaller, and a reader cannot tell without seeing it.
	echo '## the dependency declaration this was measured with'
	sed -n '/^\[workspace.dependencies\]/,/^$/p' "$REPO/Cargo.toml" | grep -vE '^\s*#|^$' || true
	echo
	echo '## the scaffold `main` reached'
	if [ -f "$REPO/crates/podbox-image/src/lib.rs" ]; then
		sed -n '/sweep_scaffold/,/^}/p' "$REPO/crates/podbox-image/src/lib.rs"
	fi
	echo
	echo '## cargo bloat -n 20'
	if [ -s "$WORK/bloat.txt" ]; then
		cat "$WORK/bloat.txt"
	else
		printf '(not taken: %s)\n' "$bloat_status"
	fi
} > "$OUT"
echo
echo "written to $OUT"

# ⭐ Did this run reproduce the committed reading, or change it? A result file
# carries the date it was taken, so a re-run always dirties the tree by one
# line and a caller has to decide whether that is a finding. This says which.
# ⚠ It never moves the exit code: this script's codes are about the ceiling and
# the measurement, and overloading them with "the number moved" would make one
# number carry two statements.
if [ -x "$REPO/scripts/common/result-diff.sh" ]; then
	echo
	echo "== against the committed reading"
	"$REPO/scripts/common/result-diff.sh" "$OUT" || true
fi

if [ "$total" -ge "$CEILING_BYTES" ]; then
	echo
	echo "FAIL: $total bytes is at or over the ceiling declared in this script." >&2
	echo "      Raise it deliberately, here, with the delta that justifies it" >&2
	echo "      committed beside the change. TODO/deps.md T-0910." >&2
	exit 1
fi
[ "$unmeasured" -eq 1 ] && exit 1
case "$bloat_status" in
"not taken:"*) exit 2 ;;
esac
exit 0
