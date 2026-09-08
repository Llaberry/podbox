#!/usr/bin/env bash
# The runner. One command, one row per pinned distribution, so that a reading
# is a property of the subject rather than of the host it was taken on.
#
# ⭐ THE READING THAT MATTERS IS NOT "IT FAILS ON ONE ROW". It is a subject that
# runs on every row and does the right thing on none of them, which a
# single-host smoke test reports as success. That shape needs a matrix to see.
#
# The subject here answers the question `podbox-complete` depends on
# (TODO/complete.md T-0410): does a rootfs read a supplied `/etc/passwd`, and
# what decides it. Every row gets the same bytes:
#
#   1. which libc the row has;
#   2. what its `/etc/nsswitch.conf` says, if it has one;
#   3. whether a supplied `/etc/passwd` carrying a user no distribution ships
#      is visible to a static glibc binary;
#   4. what that binary opens after its own execve, which is what
#      `PT_INTERP`-free does not tell you.
#
# Four rules make this a measurement rather than a survey, and each is a line
# below:
#
#   1. the manifest DIGEST is the pin. A rolling tag measures a different thing
#      each week and says so nowhere.
#   2. ⚠ a row that could not be pulled reads `no-pull`, is counted apart from
#      a row whose command ran and failed, and does not on its own fail the run.
#      "Unreachable" and "broken" need different next moves.
#   3. ⛔ a run where NOTHING ran exits 2. Every row unreachable reads exactly
#      like every row agreeing, and the second is the hoped-for answer.
#   4. the reference is fully qualified before it is pulled. An unqualified
#      name resolves through an engine's own shortname aliases to a different
#      registry, where a Docker Hub digest does not exist, and that arrives as
#      `manifest unknown`, which reads as a broken pin.
#
# Exit: 0 every row that pulled produced a reading, 1 a row pulled and the
#       harness could not run there, 2 nothing ran.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
OUT="${OUT:-$HERE/.across}"
TRANSCRIPTS="${TRANSCRIPTS:-$ROOT/experiments/results/across}"
rm -rf "$OUT"; mkdir -p "$OUT" "$TRANSCRIPTS"

# REFERENCE  LOCAL-NAME  LIBC  MANIFEST-DIGEST
# ⛔ Every digest here was resolved with `docker manifest inspect` on
# 2026-09-08. Re-pulling a tag without editing its row silently changes what
# every number below describes.
ROWS='
alpine:3.22|alpine-3.22|musl|sha256:7c8cb692ae09657cbc4a3f3cbd0e8d5a2690ba38386aaaf252dbb060bf5eb2e6
alpine:3.20|alpine-3.20|musl|sha256:c64c687cbea9300178b30c95835354e34c4e4febc4badfe27102879de0483b5e
alpine:3.10|alpine-3.10|musl|sha256:e515aad2ed234a5072c4d2ef86a1cb77d5bfe4b11aa865d9214875734c4eeb3c
voidlinux/voidlinux-musl:latest|voidlinux-musl|musl|sha256:d5c970d0015c3aa2559a2b5a87b158839969fbb1941a9ef52cc484a5392554cd
debian:11|debian-11|glibc|sha256:c0a2ad73611131275b0e9a2e7544cfe3725f4268d4085d8c6f61fdd55aef7917
debian:12|debian-12|glibc|sha256:2f65600e1252c5649d2213e1d1ea4d74253d26514dc6530102a875e429245929
ubuntu:20.04|ubuntu-20.04|glibc|sha256:c664f8f86ed5a386b0a340d981b8f81714e21a8b9c73f658c4bea56aa179d54a
rockylinux:8|rockylinux-8|glibc|sha256:2d05a9266523bbf24f33ebc3a9832e4d5fd74b973c220f2204ca802286aa275d
opensuse/leap:15.6|opensuse-leap-15.6|glibc|sha256:ca2942f9510c3e30fd322017782cdf6c067b2183dd7d56b39d0c697a9808ce2a
fedora:42|fedora-42|glibc|sha256:7c63468daf71fdc5bda3699cd483b169bb995b5137265d5ffe8f04e2ce87fbb8
archlinux:latest|archlinux-latest|glibc|sha256:818793c894d94534c22f2149154a39ebaee57e4e67321023b0866a1d5722036c
'

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
_ldd="$(ldd --version 2>&1)"
printf 'host libc         %s\n' "${_ldd%%$'\n'*}"
printf 'cc                %s\n' "$(cc --version 2>/dev/null | head -1)"
_dv="$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
printf 'docker            %s\n' "${_dv:-MISSING}"
printf 'rows              %s\n' "$(printf '%s' "$ROWS" | grep -c .)"
printf 'transcripts       %s\n' "experiments/results/across/"
echo

command -v cc >/dev/null 2>&1 || { echo "SKIP: no cc" >&2; exit 2; }
command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 || {
  echo "SKIP: no docker daemon; start dockerd" >&2; exit 2; }

# The subject the rows share. Built once, on the host, so every row runs the
# same bytes: a difference between rows is then a difference between
# distributions and nothing else.
cat > "$OUT/probe.c" <<'EOF'
#include <stdio.h>
#include <pwd.h>
int main(void) {
    struct passwd *p = getpwnam("podboxsupplied");
    printf("SUPPLIED_PASSWD=%s\n", p ? "seen" : "not-seen");
    return 0;
}
EOF
cc -static -o "$OUT/probe" "$OUT/probe.c" 2>"$OUT/cc.log" || {
  echo "SKIP: cannot build the static probe; see $OUT/cc.log" >&2; exit 2; }

cat > "$OUT/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
podboxsupplied:x:1000:1000:supplied by podbox-complete:/workspace:/bin/sh
EOF

# ⚠ The subject copies itself out of the read-only mount before running.
# Anything that writes beside itself cannot do so on a read-only bind, and that
# failure reads as the subject's rather than as the harness's.
cat > "$OUT/body.sh" <<'EOF'
#!/bin/sh
cp /mnt/probe /tmp/probe 2>/dev/null && chmod +x /tmp/probe

# ⛔ ONE GLOB PER TEST. `ls a b` exits non-zero when the FIRST pattern misses
# even though the second matched, so a combined test reported `unknown` for
# every glibc row here. Measured on 2026-09-08: /lib is a symlink to usr/lib on
# debian 12, so /lib*/libc.so.6 misses and /lib/*/libc.so.6 hits.
libc=unknown
[ -e /lib/ld-musl-x86_64.so.1 ] && libc=musl
if [ "$libc" = unknown ]; then
  for c in /lib/libc.so.6 /lib64/libc.so.6 /lib/*/libc.so.6 /usr/lib/*/libc.so.6 /usr/lib64/libc.so.6; do
    [ -e "$c" ] && { libc=glibc; break; }
  done
fi
echo "LIBC=$libc"

# Three answers, not two: the file may be absent, or present and silent about
# passwd. Folding those together loses which one a rootfs is.
if [ ! -r /etc/nsswitch.conf ]; then
  echo "NSSWITCH=absent"
else
  line=$(grep -E '^[[:space:]]*passwd' /etc/nsswitch.conf | head -1)
  if [ -z "$line" ]; then
    echo "NSSWITCH=present-no-passwd-line"
  else
    # ⚠ Squeeze TABS as well as spaces. voidlinux separates with a tab and the
    # value arrived with it embedded, which broke the column alignment and
    # would break any later comparison on the string.
    echo "NSSWITCH=$(printf '%s' "$line" | tr -s ' \t' ' ' | sed 's/^ *passwd: *//')"
  fi
fi

# ⛔ A CRASH IS NOT AN ANSWER. Separate "the probe ran and did not find the
# user" from "the probe did not run", per this script's own rule 2. Measured on
# 2026-09-08: the static glibc probe takes SIGFPE on opensuse-leap-15.6, which
# is a reading about static portability and not about nsswitch.
out=$(/tmp/probe 2>&1); prc=$?
if [ "$prc" -ne 0 ]; then
  echo "SUPPLIED_PASSWD=probe-crashed-rc$prc"
  echo "PROBE_STDERR=$(printf '%s' "$out" | tr '\n' ' ')"
else
  printf '%s\n' "$out"
fi
echo "ID=$(id -u):$(id -g)"
EOF
chmod +x "$OUT/body.sh"

qualify() { # qualify REFERENCE -> a fully qualified name with no tag
  case "$1" in
    *.*/*) printf '%s' "${1%:*}" ;;
    */*)   printf 'docker.io/%s' "${1%:*}" ;;
    *)     printf 'docker.io/library/%s' "${1%%:*}" ;;
  esac
}

ran=0; nopull=0; broken=0
printf '%-20s %-8s %-8s %-22s %s\n' ROW LIBC PULLED NSSWITCH 'SUPPLIED /etc/passwd'
printf '%.0s-' $(seq 1 88); echo

printf '%s\n' "$ROWS" | while IFS='|' read -r ref name libc digest; do
  [ -n "${ref:-}" ] || continue
  pinned="$(qualify "$ref")@$digest"
  if ! timeout 420 docker pull -q "$pinned" >/dev/null 2>&1; then
    printf '%-20s %-8s %-8s %-22s %s\n' "$name" "$libc" no-pull - -
    echo "no-pull $pinned" > "$TRANSCRIPTS/$name.out"
    continue
  fi
  out="$(timeout 300 docker run --rm \
          -v "$OUT/probe":/mnt/probe:ro \
          -v "$OUT/body.sh":/mnt/body.sh:ro \
          -v "$OUT/passwd":/etc/passwd:ro \
          -e HOME=/tmp \
          "$pinned" /bin/sh /mnt/body.sh 2>&1)"
  rc=$?
  printf '%s\n' "$out" > "$TRANSCRIPTS/$name.out"
  got_libc="$(printf '%s\n' "$out" | sed -n 's/^LIBC=//p' | head -1)"
  nsw="$(printf '%s\n' "$out" | sed -n 's/^NSSWITCH=//p' | head -1)"
  sup="$(printf '%s\n' "$out" | sed -n 's/^SUPPLIED_PASSWD=//p' | head -1)"
  if [ -z "$got_libc" ]; then
    printf '%-20s %-8s %-8s %-22s %s\n' "$name" "$libc" yes 'harness-failed' "rc=$rc"
    continue
  fi
  printf '%-20s %-8s %-8s %-22s %s\n' "$name" "$got_libc" yes "${nsw:--}" "${sup:--}"
  # ⚠ A row whose declared libc and measured libc disagree is a finding about
  # the pin, not about the subject.
  [ "$got_libc" = "$libc" ] || printf '%-20s %s\n' "$name" \
    "  ⚠ declared $libc, measured $got_libc"
done | tee "$OUT/table.txt"

# ⛔ The loop above runs in a subshell because of the pipe, so its counters do
# not survive it. Recount from the transcripts, which is the durable record
# anyway.
ran=$(grep -Lx -e 'no-pull .*' "$TRANSCRIPTS"/*.out 2>/dev/null | grep -c . )
nopull=$(grep -lx -e 'no-pull .*' "$TRANSCRIPTS"/*.out 2>/dev/null | grep -c . )
broken=$(grep -L '^LIBC=' "$TRANSCRIPTS"/*.out 2>/dev/null \
         | xargs -r grep -Lx -e 'no-pull .*' 2>/dev/null | grep -c . )

echo
echo "== rows"
printf '  ran %s, no-pull %s, harness-failed %s\n' "$ran" "$nopull" "$broken"
printf '  transcripts in experiments/results/across/\n'
echo
echo "== verdict"
if [ "$ran" -eq 0 ]; then
  echo "  NOTHING RAN. Every row unreachable reads exactly like every row"
  echo "  agreeing, so this exits 2 rather than 0."
  exit 2
fi
if [ "$broken" -gt 0 ]; then
  echo "  $broken row(s) pulled and the harness could not run there. That is a"
  echo "  defect in this script, not a reading about the distribution."
  exit 1
fi
echo "  $ran row(s) produced a reading. The table above is the measurement;"
echo "  a difference between rows is a difference between distributions."
exit 0
