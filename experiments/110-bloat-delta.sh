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
if ! cargo build --release --target "$TARGET" --manifest-path "$REPO/Cargo.toml" 2>&1 | tail -3; then
	echo "FAIL: the release build did not succeed" >&2
	exit 1
fi
[ -f "$BIN" ] || { echo "SKIP: $BIN was not produced" >&2; exit 2; }
total="$(stat -c%s "$BIN")"
printf '  %s\n' "$BIN"
printf '  total_bytes %s\n' "$total"
printf '  headroom    %s\n' "$((CEILING_BYTES - total))"

interp="$(readelf -l "$BIN" 2>/dev/null | grep -c INTERP)"
printf '  PT_INTERP   %s\n' "$interp"

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
	printf 'breakdown         %s\n' "$bloat_status"
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

if [ "$total" -ge "$CEILING_BYTES" ]; then
	echo
	echo "FAIL: $total bytes is at or over the ceiling declared in this script." >&2
	echo "      Raise it deliberately, here, with the delta that justifies it" >&2
	echo "      committed beside the change. TODO/deps.md T-0910." >&2
	exit 1
fi
case "$bloat_status" in
"not taken:"*) exit 2 ;;
esac
exit 0
