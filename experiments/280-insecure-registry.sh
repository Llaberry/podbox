#!/usr/bin/env bash
# Question: can podbox reach a local registry that speaks plain HTTP, or HTTPS
# with a certificate nothing trusts, WITHOUT losing the refusal that
# TODO/image.md T-0201 exists for?
#
# TODO/image.md T-0213.
#
# ⭐ THE TWO HALVES ARE THE WHOLE POINT AND THEY PULL IN OPPOSITE DIRECTIONS.
# T-0201 refused a plain-HTTP FALLBACK because tcp/80 is black-holed on the
# runtimes podbox targets, so an automatic downgrade hangs instead of failing.
# That finding stands and clauses 1 and 4 assert it. What it never justified is
# refusing a registry the caller explicitly named, which is the ordinary case of
# a registry on loopback; clauses 2, 3 and 5 assert that one.
#
# ⛔ EVERY DOWNGRADE IS ANNOUNCED. Clause 6 reads the announcement back, because
# an agent cannot notice that its transport was downgraded the way a person can,
# and an unannounced downgrade is the exact class of dishonesty podbox exists to
# refuse.
#
#   ./280-insecure-registry.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/insecure-registry.txt"
WORK="$(mktemp -d)"

HTTP_NAME="podbox-x280-http-$$"
TLS_NAME="podbox-x280-tls-$$"
HTTP_PORT="${PODBOX_X280_HTTP_PORT:-5000}"
TLS_PORT="${PODBOX_X280_TLS_PORT:-5443}"

cleanup() {
	docker rm -f "$HTTP_NAME" "$TLS_NAME" >/dev/null 2>&1
	rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
command -v docker >/dev/null 2>&1 || { echo "SKIP: docker is not on PATH" >&2; exit 2; }
docker info >/dev/null 2>&1 || { echo "SKIP: no docker daemon; ./scripts/common/bootstrap-env.sh" >&2; exit 2; }
command -v openssl >/dev/null 2>&1 || { echo "SKIP: openssl is not on PATH" >&2; exit 2; }

# ⚠ The store and the policy come from this script and nothing else, so a
# caller's own configuration cannot decide what is measured.
export PODBOX_STORE="$WORK/store"
unset PODBOX_INSECURE_REGISTRIES PODBOX_CONFIG
export PODBOX_CONFIG="$WORK/registries.conf"
: >"$PODBOX_CONFIG"

# ⭐ The proxy is deliberately LEFT SET where the environment sets one. Clause 3
# is only meaningful with it: the first build of this feature pulled a loopback
# registry through the environment's proxy and got HTTP 405, which is a proxy
# refusing a non-CONNECT request and reads as a broken registry.
PROXY_SET=no
[ -n "${HTTPS_PROXY:-}${https_proxy:-}" ] && PROXY_SET=yes
unset NO_PROXY no_proxy

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'podbox            %s\n' "$("$BIN" version)"
	printf 'docker            %s\n' "$(docker --version | sed 's/,.*//')"
	printf 'proxy in env      %s (clause 3 is only meaningful when yes)\n' "$PROXY_SET"
	echo
} >"$WORK/report"

fail=0
skipped=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

# ------------------------------------------------------------ the fixtures
docker rm -f "$HTTP_NAME" "$TLS_NAME" >/dev/null 2>&1
docker run -d --name "$HTTP_NAME" -p "$HTTP_PORT:5000" registry:2 >/dev/null 2>&1 || {
	echo "SKIP: could not start a plain-HTTP registry:2 on $HTTP_PORT" >&2
	exit 2
}
mkdir -p "$WORK/certs"
openssl req -newkey rsa:2048 -nodes -keyout "$WORK/certs/domain.key" -x509 -days 2 \
	-out "$WORK/certs/domain.crt" -subj "/CN=localhost" \
	-addext "subjectAltName=DNS:localhost,IP:127.0.0.1" >/dev/null 2>&1
docker run -d --name "$TLS_NAME" -p "$TLS_PORT:443" \
	-v "$WORK/certs:/certs" \
	-e REGISTRY_HTTP_ADDR=0.0.0.0:443 \
	-e REGISTRY_HTTP_TLS_CERTIFICATE=/certs/domain.crt \
	-e REGISTRY_HTTP_TLS_KEY=/certs/domain.key \
	registry:2 >/dev/null 2>&1 || {
	echo "SKIP: could not start a TLS registry:2 on $TLS_PORT" >&2
	exit 2
}

# ⚠ A bounded wait, never an unbounded one: RULES.md section 8.
for _ in $(seq 1 30); do
	curl -sS --noproxy '*' -o /dev/null "http://localhost:$HTTP_PORT/v2/" 2>/dev/null && break
	sleep 1
done
for _ in $(seq 1 30); do
	curl -sSk --noproxy '*' -o /dev/null "https://localhost:$TLS_PORT/v2/" 2>/dev/null && break
	sleep 1
done

# Something to pull. ⚠ Any image docker already holds; the question is the
# transport and not the payload.
SRC="${PODBOX_X280_SOURCE:-ghcr.io/pkgforge-dev/archlinux:latest}"
docker pull "$SRC" >/dev/null 2>&1
docker tag "$SRC" "localhost:$HTTP_PORT/x280:t" >/dev/null 2>&1
docker push "localhost:$HTTP_PORT/x280:t" >/dev/null 2>&1 || {
	echo "SKIP: could not push a fixture image into the plain-HTTP registry" >&2
	exit 2
}
mkdir -p "/etc/docker/certs.d/localhost:$TLS_PORT"
cp "$WORK/certs/domain.crt" "/etc/docker/certs.d/localhost:$TLS_PORT/ca.crt" 2>/dev/null
docker tag "$SRC" "localhost:$TLS_PORT/x280:t" >/dev/null 2>&1
docker push "localhost:$TLS_PORT/x280:t" >/dev/null 2>&1
tls_pushed=$?

# --------------------------------------------------------------------- 1
say "== 1. the default refuses an explicit http:// and NAMES the flag"
# ⛔ T-0201 kept. A refusal a caller cannot act on is a refusal that costs a
# session, so the message has to carry the remedy.
out="$(timeout 120 "$BIN" pull "http://localhost:$HTTP_PORT/x280:t" 2>&1)"
rc=$?
say "  exit              $rc (2 is invalid input)"
say "  names the flag    $(printf '%s' "$out" | grep -c -- "--insecure-registry localhost:$HTTP_PORT")"
[ "$rc" -eq 2 ] || { say "  FAIL: expected exit 2"; fail=1; }
printf '%s' "$out" | grep -q -- "--insecure-registry localhost:$HTTP_PORT" || {
	say "  FAIL: the refusal does not name the flag that would permit it"
	fail=1
}

# --------------------------------------------------------------------- 2
say ""
say "== 2. the default refuses a certificate nothing trusts"
rm -rf "$PODBOX_STORE"
out="$(timeout 300 "$BIN" pull "localhost:$TLS_PORT/x280:t" 2>&1)"
rc=$?
say "  exit              $rc (125 is a runtime failure)"
say "  says              $(printf '%s' "$out" | tr -d '\n' | grep -oE 'invalid peer certificate[^)]*' | head -1)"
[ "$rc" -ne 0 ] || { say "  FAIL: an untrusted certificate was accepted"; fail=1; }

# --------------------------------------------------------------------- 3
say ""
say "== 3. --insecure-registry pulls the whole image over plain HTTP"
rm -rf "$PODBOX_STORE"
out="$(timeout 900 "$BIN" pull --insecure-registry "localhost:$HTTP_PORT" \
	"localhost:$HTTP_PORT/x280:t" 2>&1)"
rc=$?
say "  exit              $rc"
say "  layers pulled     $(printf '%s' "$out" | grep -c 'Pull complete')"
if [ "$rc" -ne 0 ]; then
	say "  FAIL: $(printf '%s' "$out" | tail -1 | cut -c1-90)"
	[ "$PROXY_SET" = yes ] && say "        ⚠ HTTP 405 here is the PROXY refusing a non-CONNECT request."
	fail=1
fi

# --------------------------------------------------------------------- 4
say ""
say "== 4. and it changed NOTHING for any other registry"
# ⛔ The clause that keeps clause 3 from being a global downgrade. Naming one
# registry insecure must not make podbox speak HTTP to a second one.
rm -rf "$PODBOX_STORE"
out="$(timeout 120 "$BIN" pull --insecure-registry "localhost:$HTTP_PORT" \
	"http://localhost:$TLS_PORT/x280:t" 2>&1)"
rc=$?
say "  a DIFFERENT registry over http://: exit $rc"
[ "$rc" -eq 2 ] || {
	say "  FAIL: naming one registry insecure permitted plain HTTP to another"
	fail=1
}

# --------------------------------------------------------------------- 5
say ""
say "== 5. --tls-verify=false reaches the self-signed registry"
if [ "$tls_pushed" -ne 0 ]; then
	say "  SKIP: the fixture image is not in the TLS registry"
	skipped=1
else
	rm -rf "$PODBOX_STORE"
	out="$(timeout 900 "$BIN" pull --tls-verify=false "localhost:$TLS_PORT/x280:t" 2>&1)"
	rc=$?
	say "  exit              $rc"
	say "  layers pulled     $(printf '%s' "$out" | grep -c 'Pull complete')"
	[ "$rc" -eq 0 ] || { say "  FAIL: $(printf '%s' "$out" | tail -1 | cut -c1-90)"; fail=1; }

	# ⛔ And it does NOT permit plain HTTP: not verifying a certificate and not
	# having one are different asks.
	rm -rf "$PODBOX_STORE"
	out="$(timeout 120 "$BIN" pull --tls-verify=false "http://localhost:$HTTP_PORT/x280:t" 2>&1)"
	rc=$?
	say "  --tls-verify=false with an http:// reference: exit $rc"
	[ "$rc" -eq 2 ] || {
		say "  FAIL: --tls-verify=false permitted plain HTTP, which is a different ask"
		fail=1
	}
fi

# --------------------------------------------------------------------- 6
say ""
say "== 6. every downgrade is announced on stderr"
rm -rf "$PODBOX_STORE"
err="$(timeout 900 "$BIN" pull --insecure-registry "localhost:$HTTP_PORT" \
	"localhost:$HTTP_PORT/x280:t" 2>&1 >/dev/null)"
said_insecure=$(printf '%s' "$err" | grep -c 'configured as an insecure registry')
said_http=$(printf '%s' "$err" | grep -c 'use http://')
say "  announced insecure          $said_insecure"
say "  announced the http fallback $said_http"
[ "$said_insecure" -ge 1 ] || { say "  FAIL: the downgrade was silent"; fail=1; }

# ⛔ And a NORMAL pull says none of it. A disclosure printed on every run is a
# disclosure nobody reads.
rm -rf "$PODBOX_STORE"
err="$(timeout 900 "$BIN" pull "$SRC" 2>&1 >/dev/null)"
noise=$(printf '%s' "$err" | grep -c 'insecure registry')
say "  a normal pull says it       $noise time(s)"
[ "$noise" -eq 0 ] || { say "  FAIL: an ordinary pull printed a downgrade notice"; fail=1; }

# --------------------------------------------------------------------- 7
say ""
say "== 7. the environment and the config file reach the same place"
rm -rf "$PODBOX_STORE"
# ⛔ The status is captured into a variable BEFORE anything else runs. Reading
# `$?` after a `say` reads `say`'s status, which is docs/AGENTS.md absolute 8
# and cost this very script a wrong green on its first run.
PODBOX_INSECURE_REGISTRIES="localhost:$HTTP_PORT" \
	timeout 900 "$BIN" pull "localhost:$HTTP_PORT/x280:t" >/dev/null 2>&1
env_rc=$?
say "  \$PODBOX_INSECURE_REGISTRIES exit $env_rc"
[ "$env_rc" -eq 0 ] || { say "  FAIL: the environment variable did not permit the registry"; fail=1; }
printf '# a comment\nlocalhost:%s\n' "$HTTP_PORT" >"$PODBOX_CONFIG"
rm -rf "$PODBOX_STORE"
timeout 900 "$BIN" pull "localhost:$HTTP_PORT/x280:t" >/dev/null 2>&1
cfg_rc=$?
say "  \$PODBOX_CONFIG file         exit $cfg_rc"
[ "$cfg_rc" -eq 0 ] || { say "  FAIL: the config file did not permit the registry"; fail=1; }

# ⛔ A bad line in the config is named with its line number, never skipped.
printf 'localhost:%s\nhttps://oops/\n' "$HTTP_PORT" >"$PODBOX_CONFIG"
out="$(timeout 120 "$BIN" pull "localhost:$HTTP_PORT/x280:t" 2>&1)"
rc=$?
say "  a URL where a host belongs: exit $rc, $(printf '%s' "$out" | tr -d '\n' | grep -oE 'registries.conf:[0-9]+' | head -1)"
[ "$rc" -eq 2 ] || { say "  FAIL: a bad config line was not refused as invalid input"; fail=1; }
: >"$PODBOX_CONFIG"

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
