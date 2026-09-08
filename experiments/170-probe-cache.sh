#!/usr/bin/env bash
# Question: does `$store/probe.json` serve the same answer twice on one
# machine, and does it REFUSE to serve the host's answer to a confined process?
#
# TODO/probe.md T-0111. ⛔ A cache keyed wrongly is worse than no cache, and
# this is the one component where a stale answer is the exact failure the
# project exists to prevent: a `namespace` verdict served to a process that has
# a `chroot` is podbox telling the lie it was built to refuse.
#
# ⭐ THE SPECIFICATION'S KEY DOES NOT WORK, AND CLAUSE 3 IS WHY. `TOOL.md`
# section 6.1 says to key on `/proc/sys/kernel/random/boot_id`. That value is
# the KERNEL'S: the host, the reconstruction and a plain docker container all
# read the same one while producing two different rungs. Clause 3 prints both
# boot ids so the reader can see they are equal while the verdicts are not.
#
#   ./170-probe-cache.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/probe-cache.txt"
WORK="$(mktemp -d)"
STORE="$WORK/store"
trap 'rm -rf "$WORK"' EXIT INT TERM

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
command -v jq >/dev/null 2>&1 || { echo "SKIP: jq is not on PATH" >&2; exit 2; }

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'podbox            %s\n' "$("$BIN" version)"
printf 'store             a fresh directory under %s\n' "$(dirname "$WORK")"
echo

fail=0
mkdir -p "$STORE"

echo "== 1. two runs on one machine agree on the rung"
# T-0111's Prove, first half, run against --json exactly as it is written.
"$BIN" probe --json >"$WORK/a.json" 2>"$WORK/a.err"
a_rc=$?
"$BIN" probe --json >"$WORK/b.json" 2>"$WORK/b.err"
b_rc=$?
[ "$a_rc" -eq 0 ] && [ "$b_rc" -eq 0 ] || {
	echo "SKIP: podbox probe --json exited $a_rc and $b_rc" >&2
	exit 2
}
jq -e --slurpfile a "$WORK/a.json" '.rung == $a[0].rung' "$WORK/b.json" >/dev/null
same_rc=$?
host_rung="$(jq -r .rung "$WORK/a.json")"
printf '  both runs report %s\n' "$host_rung"
[ "$same_rc" -eq 0 ] || { echo "  FAIL: two runs disagreed about the rung"; fail=1; }

echo
echo "== 2. the first --cached run measures and the second is served"
PODBOX_STORE="$STORE" "$BIN" probe --cached >"$WORK/c1.out" 2>"$WORK/c1.err"
PODBOX_STORE="$STORE" "$BIN" probe --cached >"$WORK/c2.out" 2>"$WORK/c2.err"
sed 's/^/    /' "$WORK/c1.err"
sed 's/^/    /' "$WORK/c2.err"
if grep -q 'measured now' "$WORK/c1.err"; then
	echo "  ok      the first run measured"
else
	echo "  FAIL: the first run did not measure into an empty store"
	fail=1
fi
if grep -q 'served from' "$WORK/c2.err"; then
	echo "  ok      the second run was served from the cache"
else
	echo "  FAIL: the second run did not use the cache it had just written"
	fail=1
fi
[ "$(cat "$WORK/c1.out")" = "$(cat "$WORK/c2.out")" ] || {
	echo "  FAIL: the cached rung is not the measured one"
	fail=1
}
[ -s "$STORE/probe.json" ] || { echo "  FAIL: no $STORE/probe.json"; fail=1; }

echo
echo "== 3. the cache is REFUSED inside the reconstruction"
# ⭐ The clause the entry exists for. The host wrote `namespace` into the store
# above; a copy of that store is staged into the reconstruction, where the rung
# is `chroot`. A cache keyed on the boot id alone would serve `namespace`.
host_boot="$(jq -r .cache_key.boot_id "$STORE/probe.json")"
confined_out="$WORK/confined.txt"
# ⚠ A FRESH STAGE DIRECTORY, so nothing a previous run left behind can be what
# the confined process reads. 20-enter-target.sh now clears each destination
# before copying, and this makes the isolation belt-and-braces: the whole point
# of the clause is which probe.json is present inside.
export TARGET_STAGE="$WORK/stage"
if "$HERE/20-enter-target.sh" --stage "$BIN" --stage "$STORE" -- \
	/bin/sh -c 'PODBOX_STORE=/workspace/store; export PODBOX_STORE;
	            /workspace/podbox probe --cached
	            cat /proc/sys/kernel/random/boot_id' \
	>"$confined_out" 2>"$WORK/confined.err"; then
	confined_rung="$(sed -n '1p' "$confined_out")"
	confined_boot="$(sed -n '2p' "$confined_out")"
	why="$(grep -o 'measured now, because.*' "$WORK/confined.err" | head -1)"
	printf '  host      rung=%-10s boot_id=%s\n' "$host_rung" "$host_boot"
	printf '  confined  rung=%-10s boot_id=%s\n' "$confined_rung" "$confined_boot"
	printf '  %s\n' "${why:-<the cache was served>}"

	if [ "$confined_rung" = "chroot" ]; then
		echo "  ok      the confined run measured its own answer"
	else
		echo "  FAIL: expected chroot inside the reconstruction, got $confined_rung"
		fail=1
	fi
	if [ "$host_boot" = "$confined_boot" ]; then
		echo "  ⭐ the boot ids are EQUAL and the rungs are NOT. That is the"
		echo "     measurement TOOL.md section 6.1's cache key does not survive."
	else
		# Not a failure of podbox: it would mean this kernel gives the
		# container its own boot id, which would make the specification's key
		# work here and not elsewhere. Recorded as a reading.
		echo "  ⚠ RECORDED: the boot ids differ on this host, so the"
		echo "     specification's key would have caught this one case."
	fi
	if grep -q 'mnt_ns' "$WORK/confined.err"; then
		echo "  ok      the refusal names the mount namespace as a differing component"
	else
		echo "  FAIL: the refusal did not name mnt_ns"
		fail=1
	fi
else
	echo "  SKIP: the reconstruction did not run; see below" >&2
	sed 's/^/    /' "$WORK/confined.err" >&2
	exit 2
fi

{
	printf '# podbox probe cache, TODO/probe.md T-0111\n'
	printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# ⛔ The boot id is the KERNEL S. Both readings below are on one kernel.\n'
	printf 'host_rung         %s\n' "$host_rung"
	printf 'host_boot_id      %s\n' "$host_boot"
	printf 'confined_rung     %s\n' "$confined_rung"
	printf 'confined_boot_id  %s\n' "$confined_boot"
	printf 'boot_ids_equal    %s\n' \
		"$([ "$host_boot" = "$confined_boot" ] && echo yes || echo no)"
	printf 'cache_served_host %s   (no is the pass)\n' \
		"$(grep -q 'served from' "$WORK/confined.err" && echo yes || echo no)"
	echo
	echo '## why the confined run refused the cache'
	printf '  %s\n' "${why:-<none: the cache was served>}"
	echo
	echo '## the key the host wrote'
	jq -S .cache_key "$STORE/probe.json" | sed 's/^/  /'
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
