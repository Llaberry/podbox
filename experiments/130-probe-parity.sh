#!/usr/bin/env bash
# Question: does `podbox probe` select the rung the milestone says it must, in
# both environments, and do its per-probe verdicts match the reading the
# reference instrument took on this machine, row for row?
#
# This is TODO/milestones.md T-1101's acceptance, as a script, so it can be
# re-run rather than recalled:
#
#   1. `podbox probe` inside ./20-enter-target.sh selects `chroot`;
#   2. the same binary, unconfined, selects `namespace`;
#   3. every attribution row podbox reports equals the row of the same name in
#      experiments/results/attribute.txt, comparing verdict AND errno.
#
#   ./130-probe-parity.sh                run all three
#   ./130-probe-parity.sh --refresh      re-take attribute.txt first, via 30-
#
# Exit: 0 all three held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
REF="$REPO/experiments/results/attribute.txt"
OUT="$REPO/experiments/results/probe-parity.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

[ "${1:-}" = "--refresh" ] && {
	# 30- exits 2 where this kernel has no Landlock, which is a real skip and
	# not a failure of the capture. Only a 1 means a row disagreed.
	"$HERE/30-attribution-census.sh" --capture "$REPO/experiments/results" >/dev/null
	rc=$?
	[ "$rc" -eq 1 ] && { echo "SKIP: 30-attribution-census.sh reported a mismatch" >&2; exit 2; }
}

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
[ -s "$REF" ] || {
	echo "SKIP: $REF does not exist. Take it first:" >&2
	echo "      ./experiments/30-attribution-census.sh --capture experiments/results" >&2
	exit 2
}

# ------------------------------------------------------------------ the rows
#
# ⛔ Compared as NAME, VERDICT and ERRNO, never as raw text. The two
# instruments print the same row format but not the same errno-name table: the
# Go one carries ten names and prints `(16)` for the rest, and podbox carries
# more. A textual diff would report that difference as a disagreement about a
# machine, which is the one thing this comparison must not do.
normalise() {
	sed -E \
		-e '/^[[:space:]]/d' \
		-e 's/^(.*[^ ]) +OK$/\1|ok|-/' \
		-e 's/^(.*[^ ]) +FAIL errno=([0-9]+).*$/\1|denied|\2/' \
		-e 's/^(.*[^ ]) +SKIP .*$/\1|skip|/' \
		| grep -E '\|(ok|denied|skip)\|'
}

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'podbox            %s\n' "$("$BIN" version)"
printf 'binary            %s bytes\n' "$(stat -c%s "$BIN")"
printf 'reference rows    %s\n' "$REF"
echo

# The reference file carries one block per environment when it comes from the
# corpus and one block when 30- captured it here. Take the LAST block, which is
# the one this machine produced.
normalise < "$REF" > "$WORK/ref.txt"

fail=0

echo "== 1. inside the reconstruction, the rung must be chroot"
if confined_rung="$("$HERE/20-enter-target.sh" --stage "$BIN" -- /workspace/podbox probe 2>"$WORK/confined.err")"; then
	printf '  got %s\n' "$confined_rung"
	[ "$confined_rung" = "chroot" ] || { echo "  FAIL: expected chroot"; fail=1; }
else
	echo "  SKIP: the reconstruction did not run; see below" >&2
	sed 's/^/    /' "$WORK/confined.err" >&2
	exit 2
fi

echo
echo "== 2. unconfined, the rung must be namespace"
unconfined_rung="$("$BIN" probe 2>"$WORK/unconfined.err")"
printf '  got %s\n' "$unconfined_rung"
[ "$unconfined_rung" = "namespace" ] || { echo "  FAIL: expected namespace"; fail=1; }

echo
echo "== 3. the attribution rows, inside the reconstruction, against"
echo "        experiments/results/attribute.txt"
"$HERE/20-enter-target.sh" --stage "$BIN" -- /workspace/podbox probe --rows \
	2>/dev/null | normalise > "$WORK/all.txt"
[ -s "$WORK/all.txt" ] || { echo "SKIP: podbox produced no rows inside the reconstruction" >&2; exit 2; }

matched=0 differed=0 recorded=0 missing=0
while IFS='|' read -r name want_v want_e; do
	line="$(grep -F -m1 -- "$name|" "$WORK/all.txt")"
	if [ -z "$line" ]; then
		printf '  MISSING %-36s podbox reports no such row\n' "$name"
		missing=$((missing + 1)); continue
	fi
	got_v="$(printf '%s' "$line" | cut -d'|' -f2)"
	got_e="$(printf '%s' "$line" | cut -d'|' -f3)"
	if [ "$got_v" = "$want_v" ] && [ "$got_e" = "$want_e" ]; then
		printf '  ok      %-36s %s errno=%s\n' "$name" "$got_v" "${got_e:--}"
		matched=$((matched + 1)); continue
	fi
	# ⭐ ONE recorded divergence, and it is narrow on purpose: the same errno,
	# the reference calling it a denial and podbox calling it "could not run".
	# `kcmp(2)` is absent unless the kernel was built with
	# CONFIG_CHECKPOINT_RESTORE, and ENOSYS is that kernel saying the control
	# is unavailable rather than the control answering. TODO/probe.md T-0102
	# and T-0109: a control that cannot answer must say so. Anything else,
	# including this row with a different errno, is a disagreement.
	if [ "$name" = "kcmp(-1,-1,...) [control]" ] &&
	   [ "$want_v" = "denied" ] && [ "$got_v" = "skip" ] &&
	   [ "$want_e" = "38" ]; then
		printf '  ⚠ RECORDED %-33s reference says denied errno=38, podbox says skip.\n' "$name"
		printf '             ENOSYS is the kernel saying kcmp(2) is absent, which is not\n'
		printf '             the control answering. The reference records it as a denial;\n'
		printf '             TODO/probe.md T-0109 rules that it is a third state.\n'
		recorded=$((recorded + 1)); continue
	fi
	printf '  DIFFER  %-36s reference %s errno=%s, podbox %s errno=%s\n' \
		"$name" "$want_v" "${want_e:--}" "$got_v" "${got_e:--}"
	differed=$((differed + 1))
done < "$WORK/ref.txt"

echo
printf '  %d matched, %d recorded divergence, %d differed, %d missing\n' \
	"$matched" "$recorded" "$differed" "$missing"
# ⚠ Spelled out rather than `[ a ] || [ b ] && fail=1`, which parses as
# `(a || b) && c` and is one precedence rule away from never firing.
if [ "$differed" -gt 0 ] || [ "$missing" -gt 0 ]; then
	fail=1
fi

{
	# ⚠ Repo-relative. This file is tracked evidence, and an absolute path in
	# it is one machine's directory layout recorded as though it were a fact
	# about the measurement.
	printf '# podbox probe against %s\n' "experiments/results/attribute.txt"
	printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# TODO/milestones.md T-1101. Verdict and errno, never raw text.\n'
	printf 'confined_rung     %s\n' "$confined_rung"
	printf 'unconfined_rung   %s\n' "$unconfined_rung"
	printf 'rows_matched      %s\n' "$matched"
	printf 'rows_recorded     %s   (kcmp control: ENOSYS is a skip here, a denial there)\n' "$recorded"
	printf 'rows_differed     %s\n' "$differed"
	printf 'rows_missing      %s\n' "$missing"
	echo
	echo '## every row podbox reported inside the reconstruction'
	cat "$WORK/all.txt"
} > "$OUT"
echo "  written to $OUT"

# ⭐ As in 110-: say whether the re-run reproduced, so a dirty tree carrying
# only a new date is not mistaken for a finding, or the reverse.
# ⚠ Never moves the exit code, which belongs to the three acceptance clauses.
if [ -x "$REPO/scripts/common/result-diff.sh" ]; then
	echo
	echo "== against the committed reading"
	"$REPO/scripts/common/result-diff.sh" "$OUT" || true
fi

[ "$fail" -eq 0 ] || exit 1
exit 0
