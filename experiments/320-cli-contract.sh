#!/usr/bin/env bash
# Question: can a caller find out what this podbox will and will not do WITHOUT
# running anything, and does podbox answer to the names an agent already types?
#
# TODO/cli.md T-0801 (the verb and flag parity table, as data) and T-0803
# (answering to `docker` and `podman` on PATH).
#
# ⭐ WHY THIS IS A MEASUREMENT AND NOT A UNIT TEST. Both entries are about the
# surface a CALLER sees, and both of their failure modes are invisible from
# inside the process: a flag accepted and silently lost still parses, and a
# binary that answers to `docker` still runs when it is spawned as `podbox`.
# Every clause below drives the shipped binary through the surface an agent uses.
#
# ⚠ A BACKTICK INSIDE DOUBLE QUOTES IS COMMAND SUBSTITUTION, and this script
# quotes verb names constantly. Measured on 2026-09-09: two `say` lines here ran
# `docker` and `podman` and printed docker's own help into the report, with the
# names themselves rendered as empty. Single quotes, or no backticks at all.
#
# ⛔ THE THREE THINGS AN AUTOMATED CALLER DEPENDS ON:
#
#   1. the table is DATA, one row per verb and per flag, with a status from a
#      closed set of four words and a reason on every row;
#   2. a flag podbox cannot honour is refused UP FRONT with that reason, never
#      accepted and lost, and a flag nobody listed cannot be quietly accepted;
#   3. `docker` and `podman` on PATH reach the same parser, and say they did.
#
#   ./320-cli-contract.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/cli-contract.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

IMAGE="${PODBOX_RUN_IMAGE:-ghcr.io/pkgforge-dev/archlinux:latest}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
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

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'jq                %s\n' "$(jq --version)"
	echo
} >"$WORK/report"

# --------------------------------------------------------------------- 1
say "== 1. T-0801's own acceptance: the table is data"
# ⭐ The entry's Prove, verbatim in shape: at least sixty rows, and every status
# one of the four words TOOL.md section 6.8 defines.
"$BIN" system info --format '{{json .Parity}}' >"$WORK/parity.json" 2>"$WORK/e1"
rc=$?
say "  exit              $rc"
if [ "$rc" -ne 0 ]; then
	say "  FAIL: the verb did not run"
	sed 's/^/    /' "$WORK/e1" >>"$WORK/report"
	fail=1
else
	rows="$(jq 'length' "$WORK/parity.json")"
	verbs="$(jq '[.[] | select(.flag == null)] | length' "$WORK/parity.json")"
	say "  rows              $rows"
	say "  of which verbs    $verbs"
	say "  statuses          $(jq -r '[.[].status] | unique | join(" ")' "$WORK/parity.json")"
	jq -e 'length >= 60 and all(.status | IN("Native","Degraded","Stub","None"))' \
		"$WORK/parity.json" >/dev/null
	rc=$?
	say "  length >= 60 and every status one of the four: rc=$rc"
	[ "$rc" -eq 0 ] || { say "  FAIL: T-0801's Prove did not hold"; fail=1; }
	# ⛔ A row with no reason is a row that tells a caller no and not why.
	empty="$(jq '[.[] | select((.note // "") == "")] | length' "$WORK/parity.json")"
	say "  rows with no note $empty"
	[ "$empty" -eq 0 ] || { say "  FAIL: a row refuses without a reason"; fail=1; }
fi

# --------------------------------------------------------------------- 2
say ""
say "== 2. the table DECIDES, rather than describing what a parser does"
# ⛔ A flag with status None is refused up front WITH ITS REASON. This is the
# posture dockless takes at run.py:129-160 and the reason the table is consulted
# rather than written down twice.
# ⭐ TAKEN FROM THE TABLE, for the same reason clause 3 is: this named
# `run --name` and went red the day M4 made it Native.
noneflag="$(jq -r '[.[] | select(.verb == "run" and .flag != null and .status == "None") | .flag] | .[0]' \
	"$WORK/parity.json" | cut -d, -f1)"
err="$(timeout 300 "$BIN" run "$noneflag" "$IMAGE" /bin/true 2>&1 >/dev/null)"
rc=$?
say "  run $noneflag        rc=$rc"
say "    $(printf '%s' "$err" | head -1 | cut -c1-96)"
[ "$rc" -eq 2 ] || { say "  FAIL: a None flag was not invalid input"; fail=1; }
printf '%s' "$err" | grep -q 'status None' || {
	say "  FAIL: the refusal does not name the table's status"
	fail=1
}
# ⚠ THE OTHER HALF, and it is the one a table alone cannot give: a flag NOBODY
# listed must not reach a parser arm either.
err="$(timeout 300 "$BIN" run --no-such-flag "$IMAGE" /bin/true 2>&1 >/dev/null)"
rc=$?
say "  run --no-such-flag rc=$rc"
say "    $(printf '%s' "$err" | head -1 | cut -c1-96)"
[ "$rc" -eq 2 ] || { say "  FAIL: an unlisted flag was not invalid input"; fail=1; }
printf '%s' "$err" | grep -q 'no row in the parity table' || {
	say "  FAIL: the refusal does not say the table has no row"
	fail=1
}
# ⚠ And a Stub is ACCEPTED, because a stub is a difference the table states and
# not a refusal. `-i` is the one podbox has.
timeout 900 "$BIN" run -i --pull always "$IMAGE" /bin/true >/dev/null 2>&1
rc=$?
say "  run -i (a Stub)   rc=$rc"
[ "$rc" -eq 0 ] || { say "  FAIL: a Stub flag was refused"; fail=1; }

# --------------------------------------------------------------------- 3
say ""
say "== 3. every verb the table calls None says so with docker's 125"
# ⛔ Not "unknown command". A caller that reads the table and then runs the verb
# has to get the SAME reason back, from the same place.
# ⭐ TAKEN FROM THE TABLE, not written here. Measured on 2026-09-09: this clause
# named six verbs by hand and went red the day M4 implemented them, which is a
# second declaration of the same thing the table already carries.
for verb in $(jq -r '[.[] | select(.flag == null and .status == "None") | .verb] | .[0:6] | .[]' "$WORK/parity.json"); do
	err="$(timeout 60 "$BIN" "$verb" 2>&1 >/dev/null)"
	rc=$?
	want="$(jq -r --arg v "$verb" '.[] | select(.verb == $v and .flag == null) | .note' \
		"$WORK/parity.json")"
	ok=no
	[ "$rc" -eq 125 ] && printf '%s' "$err" | grep -qF "$want" && ok=yes
	say "  $(printf '%-8s' "$verb") rc=$rc  message is the table's row: $ok"
	[ "$ok" = yes ] || { say "  FAIL: $verb does not answer with its own row"; fail=1; }
done

# --------------------------------------------------------------------- 4
say ""
say '== 4. T-0803: podbox answers to `docker` and `podman` on PATH'
# ⭐ Multicall on argv[0]. The link is made by hand here, exactly as T-0803's
# Prove writes it, so this clause measures the BINARY and not the installer.
mkdir -p "$WORK/bin"
ln -sf "$BIN" "$WORK/bin/docker"
ln -sf "$BIN" "$WORK/bin/podman"
for name in docker podman; do
	out="$(PATH="$WORK/bin:$PATH" timeout 900 "$name" run --rm "$IMAGE" /bin/echo hi \
		2>"$WORK/e4-$name")"
	rc=$?
	said="$(grep -o "invoked as .$name." "$WORK/e4-$name" | head -1)"
	say "  $(printf '%-7s' "$name") stdout [$out] rc=$rc  banner says: ${said:-NOTHING}"
	[ "$out" = "hi" ] || { say "  FAIL: $name did not run the payload"; fail=1; }
	[ "$rc" -eq 0 ] || { say "  FAIL: $name exited $rc"; fail=1; }
	# ⛔ TOOL.md section 4.1: taking the name is the requirement, taking it
	# silently is what the honesty rules forbid.
	[ -n "$said" ] || {
		say "  FAIL: podbox took the $name name without saying which tool ran"
		fail=1
	}
done

# --------------------------------------------------------------------- 5
say ""
say "== 5. T-0803's ruling: the docker name is refused where a daemon answers"
# ⭐ RULED BY THE OPERATOR ON 2026-09-08. ⚠ The check is on a REACHABLE DAEMON
# and not on a `docker` binary being present, so this clause needs to know which
# state this machine is in before it can assert anything.
daemon=absent
if command -v docker >/dev/null 2>&1 && timeout 30 docker info >/dev/null 2>&1; then
	daemon=reachable
fi
say "  a docker daemon here: $daemon"
err="$(timeout 60 "$BIN" system install-names --dir "$WORK/names" 2>&1 >/dev/null)"
rc=$?
say "  install-names     rc=$rc"
if [ "$daemon" = reachable ]; then
	say "    $(printf '%s' "$err" | head -1 | cut -c1-96)"
	[ "$rc" -eq 125 ] || { say "  FAIL: the docker name was not refused"; fail=1; }
	printf '%s' "$err" | grep -q 'podbox is the wrong tool' || {
		say "  FAIL: the refusal does not say why"
		fail=1
	}
	# ⛔ And `podman` is installed anyway: the ruling is about the `docker` name.
	[ -L "$WORK/names/podman" ] || { say "  FAIL: podman was not installed"; fail=1; }
	[ -e "$WORK/names/docker" ] && { say "  FAIL: the docker link was made anyway"; fail=1; }
	# ⚠ And --force takes it, because the ruling is "unless an explicit flag".
	timeout 60 "$BIN" system install-names --dir "$WORK/forced" --force >/dev/null 2>&1
	rc=$?
	say "  install-names --force rc=$rc, docker link: $([ -L "$WORK/forced/docker" ] && echo made || echo absent)"
	[ -L "$WORK/forced/docker" ] || { say "  FAIL: --force did not install it"; fail=1; }
else
	# ⚠ THIS is the state the machines podbox is for are in, and the ruling
	# means the name IS taken there.
	[ "$rc" -eq 0 ] || { say "  FAIL: no daemon here, so both names should install"; fail=1; }
	[ -L "$WORK/names/docker" ] || { say "  FAIL: the docker name was not taken"; fail=1; }
	say "  ⚠ the refusal half could not be measured on this machine: no daemon"
	skipped=1
fi
# ⛔ Symlinks, never copies: one binary is one artefact (T-1001).
for f in "$WORK/names"/* "$WORK/forced"/*; do
	[ -e "$f" ] || continue
	[ -L "$f" ] || { say "  FAIL: $(basename "$f") is not a symlink"; fail=1; }
done
say "  every installed name is a symlink: $([ "$fail" -eq 0 ] && echo yes || echo see above)"

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
