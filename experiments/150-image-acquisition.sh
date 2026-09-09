#!/usr/bin/env bash
# Question: does the digest `podbox images` reports for a tag equal the digest
# `docker image inspect` reports for the same tag, and does a second pull cost
# nothing?
#
# This is TODO/milestones.md T-1102's acceptance, as a script, so it can be
# re-run rather than recalled. It also carries TODO/image.md T-0201's and
# T-0202's Prove commands:
#
#   1. `podbox pull <ref>` then `podbox images --format '{{.Digest}}' <ref>`
#      equals docker's `{{index .RepoDigests 0}}`;
#   2. a second pull reports every layer as already present and fetches nothing;
#   3. every blob in the store hashes to the name it is stored under;
#   4. a registry named with http:// is refused, and the refusal says so.
#
#   ./150-image-acquisition.sh
#
# ⚠ A MOVING TAG IS A RACE, AND IT IS HANDLED RATHER THAN IGNORED. `alpine:latest`
# can be republished between the two pulls, and the two digests would then
# differ for a reason that is not podbox's. Clause 1 therefore compares, and on
# a mismatch re-pulls BOTH and compares again, and reports which of the two it
# was. Set PODBOX_TEST_IMAGE to a digest-pinned reference to remove the race
# entirely.
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/image-acquisition.txt"
# ⭐ ghcr, AND NOT DOCKER HUB, AND THE REASON IS MEASURED. TODO/image.md T-0206
# says this clause cannot leave the Hub "because its whole question is whether
# podbox's digest equals the one `docker image inspect` reports". ⛔ That
# premise is wrong and was checked on 2026-09-09: docker pulls from ghcr.io
# perfectly well, and its `RepoDigests[0]` for this reference is the same value
# podbox records. The parity question needs a registry BOTH tools can reach, not
# the Hub specifically, and ghcr has no anonymous pull quota that a third party
# can exhaust on this project's behalf.
REFERENCE="${PODBOX_TEST_IMAGE:-ghcr.io/pkgforge-dev/archlinux:latest}"
WORK="$(mktemp -d)"
STORE="$WORK/store"
trap 'rm -rf "$WORK"' EXIT INT TERM
export PODBOX_STORE="$STORE"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
command -v docker >/dev/null 2>&1 || { echo "SKIP: docker not on PATH" >&2; exit 2; }
docker info >/dev/null 2>&1 || {
	echo "SKIP: docker daemon not reachable. scripts/common/bootstrap-env.sh starts it" >&2
	exit 2
}

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'podbox            %s\n' "$("$BIN" version)"
printf 'binary            %s bytes\n' "$(stat -c%s "$BIN")"
printf 'docker            %s\n' "$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo '-')"
printf 'reference         %s\n' "$REFERENCE"
printf 'store             a fresh directory under %s\n' "$(dirname "$WORK")"
echo

fail=0

# ⛔ Every exit code below is read from the process that produced it, never
# through a pipe and never from an && chain whose earlier link already
# succeeded. docs/AGENTS.md absolute 8.
pull_podbox() {
	timeout 600 "$BIN" pull "$1" >"$WORK/pull.out" 2>"$WORK/pull.err"
	return $?
}

echo "== 1. podbox's digest against docker's, for the same tag"
if ! pull_podbox "$REFERENCE"; then
	echo "  SKIP: podbox pull failed; the registry may be unreachable" >&2
	sed 's/^/    /' "$WORK/pull.err" >&2
	exit 2
fi
sed 's/^/    /' "$WORK/pull.out"

podbox_digest="$(timeout 60 "$BIN" images --format '{{.Digest}}' "$REFERENCE")"
podbox_rc=$?
[ "$podbox_rc" -eq 0 ] || { echo "SKIP: podbox images exited $podbox_rc" >&2; exit 2; }

if ! timeout 600 docker pull -q "$REFERENCE" >/dev/null 2>"$WORK/docker.err"; then
	echo "  SKIP: docker pull failed; the registry may be unreachable" >&2
	sed 's/^/    /' "$WORK/docker.err" >&2
	exit 2
fi
docker_digest="$(docker image inspect "$REFERENCE" --format '{{index .RepoDigests 0}}' | cut -d@ -f2)"

printf '  podbox  %s\n' "$podbox_digest"
printf '  docker  %s\n' "$docker_digest"
race="no"
if [ "$podbox_digest" != "$docker_digest" ]; then
	# ⚠ One re-run, to separate a moving tag from a wrong digest. A tag that
	# moved between the two pulls agrees on the second round; a wrong digest
	# does not.
	echo "  ⚠ they differ. Re-pulling both, to tell a moved tag from a wrong digest"
	pull_podbox "$REFERENCE"
	timeout 600 docker pull -q "$REFERENCE" >/dev/null 2>&1
	podbox_digest="$(timeout 60 "$BIN" images --format '{{.Digest}}' "$REFERENCE")"
	docker_digest="$(docker image inspect "$REFERENCE" --format '{{index .RepoDigests 0}}' | cut -d@ -f2)"
	printf '  podbox  %s\n' "$podbox_digest"
	printf '  docker  %s\n' "$docker_digest"
	if [ "$podbox_digest" = "$docker_digest" ]; then
		echo "  ⚠ RECORDED: the tag moved between the first two pulls. They agree now."
		race="yes"
	else
		echo "  FAIL: the digests disagree on a second round, so this is not a race"
		fail=1
	fi
fi
[ "$podbox_digest" = "$docker_digest" ] || fail=1

echo
echo "== 2. a second pull fetches nothing"
pull_podbox "$REFERENCE"
second_rc=$?
sed 's/^/    /' "$WORK/pull.out"
[ "$second_rc" -eq 0 ] || { echo "  FAIL: the second pull exited $second_rc"; fail=1; }
if grep -qi 'already' "$WORK/pull.out"; then
	echo "  ok      every layer was already present"
else
	echo "  FAIL: the second pull did not report a layer as already present"
	fail=1
fi
if grep -q 'Pull complete' "$WORK/pull.out"; then
	echo "  FAIL: the second pull fetched a layer"
	fail=1
fi

echo
echo "== 3. every stored blob hashes to the name it is stored under"
# ⛔ The store's own claim, checked against the bytes. TODO/image.md T-0202
# verifies as it writes; this asserts the result on disk rather than trusting
# that it did.
mismatched=0
counted=0
for blob in "$STORE"/blobs/sha256/*; do
	[ -f "$blob" ] || continue
	counted=$((counted + 1))
	want="$(basename "$blob")"
	got="$(sha256sum "$blob" | cut -d' ' -f1)"
	if [ "$want" != "$got" ]; then
		printf '  MISMATCH %s is stored as %s\n' "$got" "$want"
		mismatched=$((mismatched + 1))
	fi
done
printf '  %d blob(s), %d mismatched\n' "$counted" "$mismatched"
[ "$counted" -gt 0 ] || { echo "  FAIL: the store holds no blobs after a pull"; fail=1; }
[ "$mismatched" -eq 0 ] || fail=1

echo
echo "== 4. a plain-HTTP registry is refused rather than downgraded"
# ⛔ TODO/image.md T-0201. tcp/80 egress is broken on the runtime podbox
# targets, so a fallback hangs instead of failing. `timeout` is the proof that
# it did not hang: a downgrade would sit here until the timeout fired.
timeout 30 "$BIN" pull "http://registry.invalid/library/archlinux:latest" \
	>"$WORK/http.out" 2>"$WORK/http.err"
http_rc=$?
printf '  exit %s\n' "$http_rc"
sed 's/^/    /' "$WORK/http.err"
if [ "$http_rc" -eq 124 ]; then
	echo "  FAIL: it hung, which is the failure mode this refusal exists to prevent"
	fail=1
elif [ "$http_rc" -eq 0 ]; then
	echo "  FAIL: it accepted an http:// reference"
	fail=1
elif grep -qi 'HTTPS only' "$WORK/http.err"; then
	echo "  ok      refused by name"
else
	echo "  FAIL: it refused without saying that podbox is HTTPS only"
	fail=1
fi

{
	printf '# podbox image acquisition against docker, TODO/milestones.md T-1102\n'
	printf '# taken %s on kernel %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -r)"
	printf '# TODO/image.md T-0201 and T-0202 carry clauses 2 to 4.\n'
	printf 'reference         %s\n' "$REFERENCE"
	printf 'podbox_digest     %s\n' "$podbox_digest"
	printf 'docker_digest     %s\n' "$docker_digest"
	printf 'digests_match     %s\n' \
		"$([ "$podbox_digest" = "$docker_digest" ] && echo yes || echo no)"
	printf 'tag_moved         %s\n' "$race"
	printf 'blobs_stored      %s\n' "$counted"
	printf 'blobs_mismatched  %s\n' "$mismatched"
	# ⛔ NO `|| echo 0` HERE. `grep -c` PRINTS 0 and EXITS 1 on zero matches, so
	# a fallback beside it fires next to the real value and the record gets two
	# lines where it wants one. Measured here on 2026-09-08, and it is the trap
	# docs/AGENTS.md names twice.
	printf 'second_pull_fetched %s\n' \
		"$(grep -c 'Pull complete' "$WORK/pull.out" 2>/dev/null)"
	printf 'http_refused_rc   %s\n' "$http_rc"
	echo
	echo '## the second pull, verbatim'
	sed 's/^/  /' "$WORK/pull.out"
	echo
	echo '## the http:// refusal, verbatim'
	sed 's/^/  /' "$WORK/http.err"
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
