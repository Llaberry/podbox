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
