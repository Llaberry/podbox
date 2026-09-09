#!/usr/bin/env bash
# Question: how long does a fresh session wait before it can run podbox, and how
# much of that wait can be spent reading instead?
#
# ⭐ EVERY SESSION RUNS IN A NEW MACHINE. The tools the last one installed are
# gone and `target/` is empty, so a session pays the environment and the whole
# dependency graph again, `ring` and its C included. `scripts/dev.sh` exists to
# put that cost BEHIND the reading `docs/AGENTS.md` opens with, and this is the
# measurement that says whether that is worth doing.
#
# ⚠ WHAT THIS CAN AND CANNOT MEASURE HERE. The container this runs in already
# has its tools and its cargo registry cache, so the apt and download halves
# cannot be re-measured without destroying them. They are reported as
# `not measured here` rather than estimated, and the compile half, which is the
# one that dominates on a warm registry, is measured properly against an empty
# target directory.
#
#   ./310-session-startup.sh
#
# Exit: 0 it ran, 2 it could not.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
OUT="$REPO/experiments/results/session-startup.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

cd "$REPO" || exit 2
command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo is not on PATH" >&2; exit 2; }

# ⚠ Checked before writing: a cold target directory is hundreds of megabytes and
# `docs/AGENTS.md` says the writable allowance is fixed and `df` misleads.
avail_mb=$(df -Pm "$REPO" | awk 'NR==2 {print $4}')
if [ "${avail_mb:-0}" -lt 2048 ]; then
	echo "SKIP: ${avail_mb}MB free under $REPO, and a cold target needs more" >&2
	exit 2
fi

ms() { date +%s; }
took() { echo $(($2 - $1)); }

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'cpus              %s\n' "$(nproc)"
	printf 'rustc             %s\n' "$(rustc --version)"
	printf 'cargo registry    %s\n' \
		"$([ -d "$HOME/.cargo/registry" ] && echo 'present, so no crate is downloaded below' || echo absent)"
	echo
} >"$WORK/report"

say() { printf '%s\n' "$*" >>"$WORK/report"; }

# --------------------------------------------------------------------- 1
say "== 1. the compile, against an EMPTY target directory"
# ⛔ A separate target dir, so the repository's own is not destroyed to take a
# reading. This is the whole dependency graph: rustls, ring's C, ureq, tar.
cold="$WORK/coldtarget"
t0=$(ms)
cargo build --release --target x86_64-unknown-linux-musl --target-dir "$cold" \
	>"$WORK/cold.log" 2>&1
rc=$?
t1=$(ms)
cold_s=$(took "$t0" "$t1")
say "  exit              $rc"
say "  wall clock        ${cold_s}s"
say "  target size       $(du -sh "$cold" 2>/dev/null | cut -f1)"
say "  crates compiled   $(grep -c '^   Compiling' "$WORK/cold.log")"

# --------------------------------------------------------------------- 2
say ""
say "== 2. the same build, warm"
t0=$(ms)
cargo build --release --target x86_64-unknown-linux-musl --target-dir "$cold" \
	>/dev/null 2>&1
t1=$(ms)
say "  wall clock        $(took "$t0" "$t1")s"

# --------------------------------------------------------------------- 3
say ""
say "== 3. what the environment costs when it is already there"
t0=$(ms)
"$REPO/scripts/common/bootstrap-env.sh" --check >/dev/null 2>&1
t1=$(ms)
say "  bootstrap --check $(took "$t0" "$t1")s"
say "  ⚠ the INSTALL path is not measured here: this container already has"
say "    every component, and re-measuring would mean removing them. What a"
say "    genuinely fresh machine pays for apt and the zig download is"
say "    not measured rather than estimated."

# --------------------------------------------------------------------- 4
say ""
say "== 4. what a session can read while that happens"
# ⚠ Words rather than a guess at a reading speed: the number a reader can check.
words=0
for f in TODO/PROGRESS.md docs/AGENTS.md; do
	n=$(wc -w <"$REPO/$f")
	say "  $(printf '%-22s' "$f") $n words"
	words=$((words + n))
done
say "  together              $words words"
say "  ⭐ That is the reading docs/AGENTS.md opens with, and it needs no"
say "    toolchain. scripts/dev.sh starts the ${cold_s}s of clause 1 behind it."

# --------------------------------------------------------------------- 5
say ""
say "== 5. dev.sh returns immediately rather than blocking"
t0=$(ms)
"$REPO/scripts/dev.sh" start >/dev/null 2>&1
t1=$(ms)
say "  dev.sh start      $(took "$t0" "$t1")s to return"
"$REPO/scripts/dev.sh" wait >/dev/null 2>&1
say "  then wait says    $("$REPO/scripts/dev.sh" status 2>&1 | head -1)"

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"
exit 0
