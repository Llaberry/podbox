#!/usr/bin/env bash
# Question: does `podbox run` behave the way an agent that knows docker expects,
# on a runtime where no namespace can be entered?
#
# TODO/milestones.md T-1104, TODO/enter.md T-0501 to T-0506, TODO/cli.md T-0802.
#
# ⭐ THE THREE CONTRACTS AN AUTOMATED CALLER ACTUALLY DEPENDS ON, and each is a
# clause here rather than a claim in a document:
#
#   1. the payload owns stdout, so `podbox run ... | consumer` gives the
#      consumer the payload's bytes and NOTHING else;
#   2. the exit code is the payload's own, and a signalled payload is
#      128+signal, so a caller branching on the code can tell a clean exit from
#      a SIGKILL;
#   3. the banner says what podbox is NOT doing, on stderr, every time.
#
# ⛔ AND THE ONE THAT MATTERS MOST ON A MULTI-ARCHITECTURE HOST: a foreign image
# either runs through a registered interpreter or is REFUSED BY NAME. Never an
# `Exec format error` with nothing attached to it.
#
#   ./300-run.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/run.txt"
WORK="$(mktemp -d)"
BINFMT_NAME="podbox-x300-$$"

cleanup() {
	[ -f "/proc/sys/fs/binfmt_misc/$BINFMT_NAME" ] &&
		echo -1 >"/proc/sys/fs/binfmt_misc/$BINFMT_NAME" 2>/dev/null
	rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

IMAGE="${PODBOX_RUN_IMAGE:-ghcr.io/pkgforge-dev/archlinux:latest}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}

export PODBOX_STORE="$WORK/store"
unset PODBOX_DEFAULT_PLATFORM DOCKER_DEFAULT_PLATFORM

fail=0
skipped=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'host arch         %s\n' "$(uname -m)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'image             %s\n' "$IMAGE"
	echo
} >"$WORK/report"

# --------------------------------------------------------------------- 1
say "== 1. T-1104's own acceptance: a command runs and says so on stdout"
# ⛔ stdout and stderr are captured SEPARATELY. The whole contract is that they
# do not mix, so a clause that merged them would assert nothing.
out="$(timeout 2400 "$BIN" run --rm "$IMAGE" /bin/echo hi 2>"$WORK/e1")"
rc=$?
say "  stdout            [$out]"
say "  exit              $rc"
say "  stderr carries a mode= line: $(grep -c 'mode=' "$WORK/e1")"
[ "$out" = "hi" ] || { say "  FAIL: stdout was not exactly the payload's output"; fail=1; }
[ "$rc" -eq 0 ] || { say "  FAIL: expected exit 0"; fail=1; }
grep -q 'mode=' "$WORK/e1" || { say "  FAIL: no banner on stderr"; fail=1; }
# ⛔ And the banner is NOT on stdout. A banner there corrupts every pipeline.
case "$out" in *mode=*) say "  FAIL: the banner reached stdout"; fail=1 ;; esac

# --------------------------------------------------------------------- 2
say ""
say "== 2. T-0802: the exit code is the payload's, unaltered"
for pair in "0:/bin/true" "1:/bin/false"; do
	want="${pair%%:*}"
	cmd="${pair#*:}"
	timeout 900 "$BIN" run "$IMAGE" "$cmd" >/dev/null 2>&1
	got=$?
	say "  $(printf '%-28s' "$cmd") $got (want $want)"
	[ "$got" -eq "$want" ] || { say "  FAIL"; fail=1; }
done
timeout 900 "$BIN" run "$IMAGE" /bin/sh -c 'exit 42' >/dev/null 2>&1
got=$?
say "  $(printf '%-28s' "sh -c 'exit 42'") $got (want 42)"
[ "$got" -eq 42 ] || { say "  FAIL"; fail=1; }
# ⛔ A signalled payload is 128+signal, which is the shell's convention and
# docker's. Reporting 0 or 1 here would hide a SIGKILL from a caller that
# branches on the code.
timeout 900 "$BIN" run "$IMAGE" /bin/sh -c 'kill -9 $$' >/dev/null 2>&1
got=$?
say "  $(printf '%-28s' "sh -c 'kill -9 \$\$'") $got (want 137 = 128+9)"
[ "$got" -eq 137 ] || { say "  FAIL: a SIGKILLed payload did not report 128+9"; fail=1; }
timeout 900 "$BIN" run "$IMAGE" /no/such/binary >/dev/null 2>&1
got=$?
say "  $(printf '%-28s' "/no/such/binary") $got (want 127)"
[ "$got" -eq 127 ] || { say "  FAIL: a missing command is not 127"; fail=1; }

# --------------------------------------------------------------------- 3
say ""
say "== 3. it is really a chroot into the image, not the host"
os="$(timeout 900 "$BIN" run "$IMAGE" /bin/sh -c '. /etc/os-release; echo $NAME' 2>/dev/null)"
say "  /etc/os-release   $os"
host_os="$(. /etc/os-release 2>/dev/null; echo "${NAME:-unknown}")"
say "  the host's is     $host_os"
[ -n "$os" ] || { say "  FAIL: the payload could not read the image's /etc/os-release"; fail=1; }
[ "$os" != "$host_os" ] || {
	say "  ⚠ the image and the host report the same distribution, so this clause"
	say "    cannot distinguish them. It asserts only that the file was readable."
}

# --------------------------------------------------------------------- 4
say ""
say "== 4. -e, -w and --entrypoint"
got="$(timeout 900 "$BIN" run -e GREETING=hello "$IMAGE" /bin/sh -c 'echo $GREETING' 2>/dev/null)"
say "  -e GREETING=hello  [$got]"
[ "$got" = "hello" ] || { say "  FAIL"; fail=1; }
got="$(timeout 900 "$BIN" run -w /etc "$IMAGE" /bin/pwd 2>/dev/null)"
say "  -w /etc            [$got]"
[ "$got" = "/etc" ] || { say "  FAIL"; fail=1; }
got="$(timeout 900 "$BIN" run --entrypoint /bin/echo "$IMAGE" ep 2>/dev/null)"
say "  --entrypoint       [$got]"
[ "$got" = "ep" ] || { say "  FAIL"; fail=1; }
# ⚠ A bare name is resolved along the image's own PATH, inside the new root.
got="$(timeout 900 "$BIN" run "$IMAGE" sh -c 'echo onpath' 2>/dev/null)"
say "  a bare name        [$got]"
[ "$got" = "onpath" ] || { say "  FAIL: a bare name was not resolved on PATH"; fail=1; }

# --------------------------------------------------------------------- 5
say ""
say "== 5. T-0506: a foreign image is run through an interpreter, or REFUSED"
if ! [ -f /proc/sys/fs/binfmt_misc/register ]; then
	say "  SKIP: binfmt_misc is not mounted, so neither half can be measured"
	skipped=1
elif ! [ -x /usr/bin/qemu-aarch64-static ]; then
	say "  SKIP: qemu-aarch64-static is absent; ./scripts/common/bootstrap-env.sh"
	skipped=1
else
	# ⛔ The magic is written with backslash escapes the KERNEL parses. Raw NUL
	# bytes get it truncated at the first one, leaving a 7-byte magic that
	# matches every 64-bit ELF and routes every native binary to the aarch64
	# interpreter. See experiments/260-multiarch.sh clause 5.
	echo ":$BINFMT_NAME:M::\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\xb7\x00:\xff\xff\xff\xff\xff\xff\xff\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff:/usr/bin/qemu-aarch64-static:PF" \
		>/proc/sys/fs/binfmt_misc/register 2>/dev/null
	magic="$(sed -n 's/^magic //p' "/proc/sys/fs/binfmt_misc/$BINFMT_NAME" 2>/dev/null)"
	if [ "${#magic}" -ne 40 ]; then
		say "  SKIP: the binfmt registration did not take (magic ${#magic} chars, want 40)"
		[ -f "/proc/sys/fs/binfmt_misc/$BINFMT_NAME" ] &&
			echo -1 >"/proc/sys/fs/binfmt_misc/$BINFMT_NAME" 2>/dev/null
		skipped=1
	else
		got="$(timeout 2400 "$BIN" run --platform linux/arm64 "$IMAGE" /bin/uname -m 2>"$WORK/e5")"
		rc=$?
		say "  arm64 image, uname -m inside it: [$got] rc=$rc"
		say "  the host is                      $(uname -m)"
		say "  disclosed: $(grep -o 'Running it through the registered interpreter [^ ]*' "$WORK/e5" | head -1)"
		[ "$got" = "aarch64" ] || {
			say "  FAIL: the arm64 payload did not report aarch64"
			fail=1
		}
		grep -q 'registered interpreter' "$WORK/e5" || {
			say "  FAIL: podbox ran a foreign image without saying so"
			fail=1
		}
	fi

	# ⛔ The other half: an architecture nothing is registered for must be
	# refused with a message a caller can act on, never an Exec format error.
	err="$(timeout 2400 "$BIN" run --platform linux/riscv64 "$IMAGE" /bin/true 2>&1 >/dev/null)"
	rc=$?
	say "  riscv64, nothing registered: rc=$rc"
	say "    $(printf '%s' "$err" | grep -o 'podbox cannot execute it.*' | cut -c1-88)"
	[ "$rc" -eq 125 ] || { say "  FAIL: expected exit 125"; fail=1; }
	printf '%s' "$err" | grep -q 'cannot execute it' || {
		say "  FAIL: the refusal does not say podbox cannot execute it"
		fail=1
	}
	printf '%s' "$err" | grep -q 'linux/amd64' || {
		say "  FAIL: the refusal does not name the HOST platform"
		fail=1
	}
fi

# --------------------------------------------------------------------- 6
say ""
say "== 6. --pull never on a platform the store does not hold"
# ⚠ "held, for another platform" and "not held at all" are different sentences
# and send a caller to different remedies.
err="$(timeout 300 "$BIN" run --pull never --platform linux/ppc64le "$IMAGE" /bin/true 2>&1 >/dev/null)"
rc=$?
say "  exit              $rc"
say "  says              $(printf '%s' "$err" | tr -d '\n' | cut -c1-96)"
printf '%s' "$err" | grep -q 'holds' || {
	say "  FAIL: the refusal does not name what the store does hold"
	fail=1
}

# --------------------------------------------------------------------- 7
say ""
say "== 7. T-1104's own acceptance: inside the reconstruction"
# ⭐ THE CLAUSE THAT MAKES THE OTHERS MEAN SOMETHING. Everything above ran on
# this host, where podbox selects the `namespace` rung and every capability is
# present. The reconstruction is where `mount(2)` is EPERM and podbox has to
# fall to `chroot`, which is the rung this milestone is named for.
#
# ⚠ The store is pre-pulled OUTSIDE and staged in, and the run is `--pull never`.
# The reconstruction has no CA bundle, so a pull from inside it fails on an
# unverifiable certificate, and that is a fact about the reconstruction rather
# than about `run`. Acquisition is M1's and is proven by 150-.
recon_ok=0
if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
	say "  SKIP: no docker daemon, so the reconstruction cannot be entered"
	skipped=1
elif ! docker image inspect container-research/target:1 >/dev/null 2>&1; then
	say "  SKIP: container-research/target:1 is not built."
	say "        ./experiments/10-build-target-image.sh"
	skipped=1
else
	recon_store="$WORK/recon-store"
	PODBOX_STORE="$recon_store" timeout 900 "$BIN" pull "$IMAGE" >/dev/null 2>&1
	if [ ! -d "$recon_store" ]; then
		say "  SKIP: could not pre-pull $IMAGE for the reconstruction"
		skipped=1
	else
		out="$(timeout 2400 "$REPO/experiments/20-enter-target.sh" \
			--stage "$BIN" --stage "$recon_store" \
			-- /bin/sh -c "PODBOX_STORE=/workspace/$(basename "$recon_store") /workspace/podbox run --rm --pull never $IMAGE /bin/echo hi" \
			2>"$WORK/e7")"
		rc=$?
		rung="$(sed -n 's/.*mode=\([a-z]*\).*/\1/p' "$WORK/e7" | head -1)"
		say "  stdout            [$out]"
		say "  exit              $rc"
		say "  rung inside       $rung   (this host selects $("$BIN" probe 2>/dev/null | head -1))"
		say "  says it does NOT provide: $(grep -o 'does NOT provide:.*' "$WORK/e7" | head -1 | cut -c1-64)"
		[ "$out" = "hi" ] || { say "  FAIL: stdout was not the payload's output"; fail=1; }
		[ "$rc" -eq 0 ] || { say "  FAIL: expected exit 0"; fail=1; }
		# ⛔ The rung has to be BELOW this host's, or the reconstruction is not
		# confining anything and this clause proves nothing.
		[ "$rung" = "chroot" ] || {
			say "  FAIL: the reconstruction did not select the chroot rung"
			fail=1
		}
		grep -q 'does NOT provide' "$WORK/e7" || {
			say "  FAIL: the banner does not say what this mode cannot do"
			fail=1
		}
		recon_ok=1
	fi
fi
[ "$recon_ok" -eq 1 ] || say "  ⚠ clause 7 did not run, so the chroot rung is unmeasured here"

# --------------------------------------------------------------------- 8
say ""
say "== 8. T-0505: exec is a fresh chroot, and every channel says so"
# ⛔ `docker exec` enters the container's namespaces. podbox has none to enter,
# so the whole content of this clause is that the difference is STATED rather
# than discovered: on stderr for a person, in `inspect` for a program, and by
# refusing to invent the thing it cannot attach to.
out="$(timeout 900 "$BIN" exec "$IMAGE" /bin/sh -c 'echo marker' 2>"$WORK/e8")"
rc=$?
say "  stdout            [$out]"
say "  exit              $rc"
[ "$out" = "marker" ] || { say "  FAIL: stdout was not exactly the payload's output"; fail=1; }
[ "$rc" -eq 0 ] || { say "  FAIL: expected exit 0"; fail=1; }
say "  says              $(grep -o 'this is a [a-z-]* re-entry' "$WORK/e8" | head -1)"
grep -q 'not an entry into a running container' "$WORK/e8" || {
	say "  FAIL: exec ran without saying it is not a namespace entry"
	fail=1
}
# ⭐ The same fact, to a program rather than to a reader, and it has to be the
# same fact: the banner and this field read one pair of constants.
shares="$(timeout 300 "$BIN" inspect --format '{{.Exec.Shares}}' "$IMAGE" 2>/dev/null)"
mode="$(timeout 300 "$BIN" inspect --format '{{.Exec.Mode}}' "$IMAGE" 2>/dev/null)"
say "  {{.Exec.Shares}}  [$shares]   {{.Exec.Mode}}  [$mode]"
[ "$shares" = "filesystem" ] || { say "  FAIL: inspect does not report the sharing"; fail=1; }
grep -q "$mode" "$WORK/e8" || {
	say "  FAIL: the banner and inspect do not name the same mode"
	fail=1
}
# ⛔ No default command. An image's Cmd is what `run` starts, and re-running it
# from `exec` is a process the caller did not ask for.
# ⚠ The status is read once, from the process that produced it, into a variable.
timeout 300 "$BIN" exec "$IMAGE" >/dev/null 2>&1
rc=$?
say "  exec with no command: rc=$rc  (want 2, invalid input)"
[ "$rc" -eq 2 ] || {
	say "  FAIL: exec fell back to the image's Cmd, or used the wrong exit code"
	fail=1
}
# ⛔ And it never pulls: an empty store is a refusal that names `run`, not a fetch.
err="$(PODBOX_STORE="$WORK/empty-store" timeout 300 "$BIN" exec "$IMAGE" /bin/true 2>&1 >/dev/null)"
rc=$?
say "  against an empty store: rc=$rc"
say "    $(printf '%s' "$err" | grep -o 'never pulls.*' | cut -c1-72)"
[ "$rc" -eq 125 ] || { say "  FAIL: expected exit 125"; fail=1; }
printf '%s' "$err" | grep -q 'never pulls' || {
	say "  FAIL: the refusal does not say exec never pulls"
	fail=1
}

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
