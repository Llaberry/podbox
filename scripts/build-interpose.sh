#!/usr/bin/env bash
# Question: none. This is a build step, not a measurement.
#
# Builds crates/podbox-interpose as an LD_PRELOAD cdylib, once per libc.
#
# ⛔ RUSTFLAGS, NOT A CRATE-LOCAL .cargo/config.toml. Cargo MERGES config files
# up the directory tree and appends the parent's rustflags AFTER the child's,
# so a crate-local `-C target-feature=-crt-static` is followed by the root's
# `+crt-static` and loses. The RUSTFLAGS environment variable replaces the
# config value outright, which is why this is a script and not a config file.
# Measured by experiments/60-interposer-libc.sh.
#
# ⛔ ONE OBJECT PER LIBC. A preloaded object is loaded by the payload's own
# dynamic loader and resolves its own imports against the payload's libc, so a
# musl-linked object cannot be preloaded into a glibc payload. podbox embeds
# both and selects on the payload's PT_INTERP (TOOL.md section 6.7, TODO/interpose.md
# T-0702).
#
# Exit: 0 every requested target built, 1 a build failed, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
CRATE="$ROOT/crates/podbox-interpose"

command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo not on PATH" >&2; exit 2; }
[ -d "$CRATE" ] || { echo "SKIP: $CRATE is absent" >&2; exit 2; }

TARGETS="${TARGETS:-x86_64-unknown-linux-musl x86_64-unknown-linux-gnu}"
rc=0
for t in $TARGETS; do
  if ! rustup target list --installed 2>/dev/null | grep -qx "$t"; then
    printf 'SKIP %s: target not installed\n' "$t" >&2
    continue
  fi
  printf '== %s\n' "$t"
  if ( cd "$CRATE" && RUSTFLAGS="-C target-feature=-crt-static" \
        cargo build --release --target "$t" ); then
    ls -l "$CRATE/target/$t/release/libpodbox_interpose.so"
  else
    printf 'FAIL %s\n' "$t" >&2
    rc=1
  fi
done
exit "$rc"
