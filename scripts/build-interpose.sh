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
# ⛔ **THE MUSL TARGET NEEDS `zig cc` AS THE LINKER, AND WITHOUT IT THIS SCRIPT
# PRODUCES TWO GLIBC OBJECTS.** Measured on 2026-09-09: with the default linker
# the `x86_64-unknown-linux-musl` object records `libc.so.6`, `libgcc_s.so.1`
# and `ld-linux-x86-64.so.2` in DT_NEEDED, which is byte-for-byte the wrong
# answer to "one object per libc" and reads as success because the build exits 0.
# `rustc` passes `-lgcc_s` on the musl target even under `panic = "abort"`, and
# `musl-tools` ships no musl-linked `libgcc_s.so.1`, so the host `cc` links it
# against the host's glibc. `zig cc` carries its own `compiler-rt`.
# `scripts/zig-cc.sh` records the whole determination.
ZIG_LINKER="$ROOT/scripts/zig-cc.sh"

# What DT_NEEDED must name, per target. ⛔ Asserted rather than assumed: the
# failure above is silent, and a build that exits 0 having produced the wrong
# object is the shape TODO/interpose.md T-0702 exists to prevent.
want_libc() {
  case "$1" in
  *-musl | *-musl*) printf 'libc.so' ;;
  *) printf 'libc.so.6' ;;
  esac
}

rc=0
for t in $TARGETS; do
  if ! rustup target list --installed 2>/dev/null | grep -qx "$t"; then
    printf 'SKIP %s: target not installed\n' "$t" >&2
    continue
  fi
  printf '== %s\n' "$t"
  link=""
  case "$t" in
  *-musl | *-musl*)
    if command -v zig >/dev/null 2>&1 && [ -x "$ZIG_LINKER" ]; then
      link="-C linker=$ZIG_LINKER"
      printf '   linker: scripts/zig-cc.sh (zig %s)\n' "$(zig version 2>/dev/null)"
    else
      printf 'SKIP %s: no zig, and the default linker produces a GLIBC object\n' "$t" >&2
      printf '      ./scripts/common/bootstrap-env.sh zig\n' >&2
      rc=2
      continue
    fi
    ;;
  esac
  so="$CRATE/target/$t/release/libpodbox_interpose.so"
  # ⛔ THE VERSION SCRIPT, T-0701 constraint 1. Without it a Rust cdylib exports
  # `rust_eh_personality` and the rest of its runtime into every process it is
  # loaded into, and this object is loaded into every process of a container.
  # ⚠ An ABSOLUTE path: the build runs with `$CRATE` as its working directory
  # and the linker is invoked from somewhere else again.
  vs="-C link-arg=-Wl,--version-script=$CRATE/interpose.map"
  if ( cd "$CRATE" && RUSTFLAGS="-C target-feature=-crt-static $link $vs" \
        cargo build --release --target "$t" ); then
    ls -l "$so"
    needed="$(readelf -dW "$so" 2>/dev/null | awk -F'[][]' '/NEEDED/{print $2}' | tr '\n' ' ')"
    printf '   DT_NEEDED: %s\n' "$needed"
    want="$(want_libc "$t")"
    # ⚠ An exact word. `libc.so.6` CONTAINS `libc.so`, so a substring test would
    # call a glibc object musl-linked, which is the very mistake being caught.
    case " $needed " in
    *" $want "*) printf '   ok: it names %s, so it is %s-linked\n' "$want" \
                   "$(case "$t" in *musl*) echo musl ;; *) echo glibc ;; esac)" ;;
    *) printf 'FAIL %s: DT_NEEDED is [%s] and must name %s\n' "$t" "$needed" "$want" >&2
       rc=1 ;;
    esac
  else
    printf 'FAIL %s\n' "$t" >&2
    rc=1
  fi
done
exit "$rc"
