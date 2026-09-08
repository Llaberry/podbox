#!/usr/bin/env bash
# result-diff.sh - did a re-run reproduce the committed reading?
#
#   scripts/common/result-diff.sh <result-file>
#
# ⭐ WHY THIS EXISTS. Every result file carries the date it was taken, so
# re-running an experiment rewrites that line and dirties the tree by one
# character of information. An unattended session then has to decide whether
# two changed lines are a finding or noise, and guessing wrong in either
# direction is expensive: committing a regression as an "update", or discarding
# a real change as "just the timestamp".
#
# This compares the working copy against what git has, IGNORING the lines that
# are a clock and nothing else, and says which it is. It reads git's copy rather
# than a second file, so there is one source of truth.
#
# ⛔ It never edits anything. The caller decides what to do.
#
# Exit: 0 every measured value is unchanged, 1 something measured changed,
#       2 could not compare (not tracked, no git, no working copy).
set -uo pipefail

f="${1:?usage: result-diff.sh <result-file>}"
command -v git >/dev/null 2>&1 || { echo "SKIP: no git" >&2; exit 2; }
[ -f "$f" ] || { echo "SKIP: $f does not exist" >&2; exit 2; }
root="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "SKIP: not a git tree" >&2; exit 2; }
rel="$(realpath --relative-to="$root" "$f")"
git -C "$root" ls-files --error-unmatch -- "$rel" >/dev/null 2>&1 || {
	echo "  new: $rel is not tracked yet, so there is nothing to compare against"
	exit 0
}

# The clock lines, and only those. Anything else that varies between two runs of
# the same measurement is a finding by definition.
strip() { grep -vE '^(# taken |date +)' -- "$1"; }

was="$(mktemp)"; now="$(mktemp)"
trap 'rm -f "$was" "$now"' EXIT INT TERM
git -C "$root" show "HEAD:$rel" 2>/dev/null | grep -vE '^(# taken |date +)' > "$was"
strip "$f" > "$now"

if diff -q "$was" "$now" >/dev/null 2>&1; then
	echo "  reproduced: every measured value in $rel is the committed one."
	echo "              Only the date line moved, so the diff is noise."
	exit 0
fi
echo "  ⚠ CHANGED: $rel differs from the committed reading in more than its date."
diff -u "$was" "$now" | sed -n '3,25p' | sed 's/^/    /'
echo "  That is a finding. Read it before committing it."
exit 1
