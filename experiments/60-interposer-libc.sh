#!/usr/bin/env bash
# Question: podbox is one static musl binary (TOOL.md section 3.4, section 7 M7), and its
# interposer is an LD_PRELOAD object (section 6.7). Can one build satisfy both, and
# can one object reach every payload?
#
# Two claims are checked, both of which the specification leaves open:
#
#   A. can `crates/podbox-interpose` be built as a cdylib under the workspace's
#      own `-C target-feature=+crt-static`?
#   B. can a musl-linked preload object be loaded into a glibc payload?
#
# ⭐ **B IS ANSWERED, and not here.** `experiments/80-interposer-abi.sh` answers
# it with both controls: no, and the reason is the SONAME rather than any
# symbol. musl's libc declares no SONAME, so an object linked against it records
# `libc.so` in DT_NEEDED, and on a glibc host `/lib/x86_64-linux-gnu/libc.so` is
# a GNU ld script. The loader rejects it at the ELF header, before a symbol is
# looked at. 80- also shows the same refusal in the other direction. What
# remains of this script is the pair of facts about podbox's OWN Rust object,
# which 80- does not measure because it uses the C reference as its subject.
#
# ⚠ B needs a musl libc on the build host. `--target x86_64-unknown-linux-musl`
# with `-crt-static` hands the link to the host's own C toolchain, so on a host
# with no musl the resulting object records `libc.so.6` in DT_NEEDED and is a
# glibc object under a musl target name. This script reads DT_NEEDED and exits
# 2 rather than answering B from that object: an object that is not musl-linked
# cannot measure whether a musl-linked one loads.
#
# ⛔ AND A MUSL TOOLCHAIN IS NOT ENOUGH FOR THIS CRATE. Measured on 2026-09-08
# with `musl-tools` 1.2.4-2 installed and `musl-gcc` on PATH:
#
#     RUSTFLAGS="-C target-feature=-crt-static -C linker=musl-gcc" \
#       cargo build --release --target x86_64-unknown-linux-musl
#     /usr/bin/ld: cannot find libgcc_s.so.1: No such file or directory
#
# rustc passes `-lgcc_s` for the unwinder on this target even though the crate
# sets `panic = "abort"`, and `-C link-arg=-static-libgcc` does not remove it.
# `musl-tools` ships no musl-linked `libgcc_s.so.1`. This is the same shortage
# that makes a glibc-linked Rust object fail inside alpine with
# `_Unwind_Resume: symbol not found`. A musl build of THIS crate needs a musl
# cross toolchain that carries its own libgcc, not just `musl-gcc`.
# `TODO/interpose.md` T-0702 carries it, and 80- is what settles the design
# question in the meantime.
#
# Inputs pinned: the toolchain named by rust-toolchain.toml, the two targets
# named below, and this host's own /usr/bin/env as the glibc payload. No
# network, no container, no registry.
#
# Exit: 0 every question this host can answer was answered and matched, 1 an
#       answer contradicts what this script expects, 2 a question could not be
#       run here.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
CRATE="$ROOT/crates/podbox-interpose"
OUT="${OUT:-$HERE/.interpose}"
mkdir -p "$OUT"

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
printf 'rustc             %s\n' "$(rustc --version 2>/dev/null || echo MISSING)"
printf 'cargo             %s\n' "$(cargo --version 2>/dev/null || echo MISSING)"
printf 'targets installed %s\n' "$(rustup target list --installed 2>/dev/null | tr '\n' ' ')"
echo

command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo not on PATH" >&2; exit 2; }
[ -d "$CRATE" ] || { echo "SKIP: $CRATE is absent" >&2; exit 2; }
for t in x86_64-unknown-linux-musl x86_64-unknown-linux-gnu; do
  rustup target list --installed 2>/dev/null | grep -qx "$t" || {
    echo "SKIP: target $t is not installed" >&2; exit 2; }
done

rc=0

echo "== A. cdylib under the workspace's +crt-static"
# ⚠ RUSTFLAGS is deliberately UNSET here so the crate picks up the root
# .cargo/config.toml, which is the condition being measured.
a_log="$OUT/a-crt-static.txt"
( cd "$CRATE" && unset RUSTFLAGS && cargo build --release \
    --target x86_64-unknown-linux-musl ) >"$a_log" 2>&1
a_rc=$?
if [ "$a_rc" -eq 0 ]; then
  echo "UNEXPECTED: the build succeeded. The root config no longer sets +crt-static,"
  echo "            or cargo's cdylib rule changed. Re-read scripts/build-interpose.sh."
  rc=1
else
  if grep -q 'does not support these crate types' "$a_log"; then
    echo "REFUSED, as expected:"
    grep -m1 'does not support these crate types' "$a_log" | sed 's/^/  /'
  else
    echo "FAILED for another reason; see $a_log"
    tail -3 "$a_log" | sed 's/^/  /'
    rc=1
  fi
fi
echo

echo "== A2. the same crate with -crt-static, one object per libc"
# ⭐ THE MUSL ARM NEEDS A LINKER THAT CARRIES ITS OWN libgcc_s, and 2026-09-08
# is when it got one. rustc passes `-lgcc_s` on this target even under
# `panic = "abort"`, and `musl-tools` ships no musl-linked `libgcc_s.so.1`, so
# the default `cc` linked the "musl" object against the HOST's glibc and B
# below could not run. `zig cc` carries `compiler-rt`, which provides those
# symbols. Where zig is absent the arm falls back to the default linker and
# says so rather than reporting a musl object that is not one.
ZIG_LINKER="$ROOT/scripts/zig-cc.sh"
if command -v zig >/dev/null 2>&1 && [ -x "$ZIG_LINKER" ]; then
  MUSL_LINK="-C linker=$ZIG_LINKER"
  # ⚠ Repo-relative in the record: this file is tracked evidence and an
  # absolute path in it is one machine's layout written down as a measurement.
  printf '  linker for musl: %s (zig %s)\n' "scripts/zig-cc.sh" "$(zig version)"
else
  MUSL_LINK=""
  echo "  ⚠ zig is absent, so the musl arm uses the default linker and will"
  echo "    produce a glibc-linked object. Install it with"
  echo "    ./scripts/common/bootstrap-env.sh zig"
fi
for t in x86_64-unknown-linux-musl x86_64-unknown-linux-gnu; do
  link=""
  [ "$t" = x86_64-unknown-linux-musl ] && link="$MUSL_LINK"
  if ( cd "$CRATE" && RUSTFLAGS="-C target-feature=-crt-static $link" \
        cargo build --release --target "$t" ) >>"$OUT/a2-build.txt" 2>&1; then
    so="$CRATE/target/$t/release/libpodbox_interpose.so"
    printf '  %-30s OK   %8s bytes  %s\n' "$t" "$(stat -c%s "$so")" \
      "$(readelf -d "$so" 2>/dev/null | grep -c NEEDED) NEEDED entries"
  else
    printf '  %-30s FAIL\n' "$t"
    rc=1
  fi
done
echo

echo "== B. cross-libc preload reach"
MUSL_SO="$CRATE/target/x86_64-unknown-linux-musl/release/libpodbox_interpose.so"
GNU_SO="$CRATE/target/x86_64-unknown-linux-gnu/release/libpodbox_interpose.so"
# The payload: this host's own /usr/bin/env, which is glibc-linked. Its
# interpreter is read rather than assumed.
PAYLOAD=/usr/bin/env
if [ ! -x "$PAYLOAD" ]; then
  echo "SKIP: $PAYLOAD is absent, so there is no glibc payload to preload into" >&2
  exit 2
fi
needed() { readelf -d "$1" 2>/dev/null | grep NEEDED | tr -s ' ' \
  | cut -d'[' -f2 | tr -d ']' | tr '\n' ' '; }

MUSL_NEEDED="$(needed "$MUSL_SO")"
GNU_NEEDED="$(needed "$GNU_SO")"
printf '  payload           %s\n' "$PAYLOAD"
printf '  payload PT_INTERP %s\n' \
  "$(readelf -l "$PAYLOAD" 2>/dev/null | grep -o '/[^]]*ld[^]]*' | head -1)"
printf '  musl-target DT_NEEDED   %s\n' "$MUSL_NEEDED"
printf '  gnu-target  DT_NEEDED   %s\n' "$GNU_NEEDED"
echo

# ⛔ THE CONTROL COMES FIRST. Without a matching-libc object that does load,
# a cross-libc object that does not load proves nothing about the libc.
c_out="$(LD_PRELOAD="$GNU_SO" "$PAYLOAD" true 2>&1)"; c_rc=$?
printf '  glibc object into a glibc payload (control): rc=%d\n' "$c_rc"
[ -n "$c_out" ] && printf '%s\n' "$c_out" | sed 's/^/    /'
if [ "$c_rc" -ne 0 ]; then
  echo "FAIL: the control did not load. Nothing below is interpretable."
  exit 1
fi
echo

case "$MUSL_NEEDED" in
  *libc.so.6*)
    echo "== B: COULD NOT RUN on this host"
    echo "  The musl-target object records libc.so.6 in DT_NEEDED, so it was"
    echo "  linked against this host's glibc and is not a musl object."
    # ⚠ Captured without a pipe. Under `pipefail`, `ldd --version | head -1`
    # returns ldd's SIGPIPE status and the `||` branch fires beside the value.
    _ldd="$(ldd --version 2>&1)"
    printf '    host libc     %s\n' "${_ldd%%$'\n'*}"
    printf '    musl-gcc      %s\n' "$(command -v musl-gcc || echo absent)"
    printf '    ld-musl       %s\n' \
      "$([ -e /lib/ld-musl-x86_64.so.1 ] && echo present || echo absent)"
    printf '    musl libgcc_s %s\n' \
      "$(ls /usr/lib/x86_64-linux-musl/libgcc_s.so.1 2>/dev/null || echo absent)"
    # ⛔ NAME THE ACTUAL SHORTAGE. A musl libc being present is not the same as
    # this crate being buildable against it: rustc passes -lgcc_s on this target
    # regardless of `panic = "abort"`, and musl-tools ships no musl-linked
    # libgcc_s.so.1. Saying "no musl here" when musl-gcc is on PATH sends the
    # next session to install a toolchain it already has.
    echo "  What is missing is a musl-linked libgcc_s.so.1, not musl itself."
    echo "  ⭐ This does not leave B unanswered. Run"
    echo "     ./experiments/80-interposer-abi.sh, which answers it against the C"
    echo "     reference interposer and needs only musl-gcc, not a musl libgcc."
    echo "  TODO/interpose.md T-0702 carries what is still open here: a musl"
    echo "  build of podbox's own Rust cdylib, which needs a cross toolchain"
    echo "  carrying its own libgcc_s. See this script's header."
    exit 2
    ;;
esac

b_out="$(LD_PRELOAD="$MUSL_SO" "$PAYLOAD" true 2>&1)"; b_rc=$?
printf '  musl object into a glibc payload: rc=%d\n' "$b_rc"
[ -n "$b_out" ] && printf '%s\n' "$b_out" | sed 's/^/    /'
echo

echo "== verdict"
if [ "$b_rc" -ne 0 ]; then
  echo "  one object per libc is REQUIRED: the musl object does not load into a"
  echo "  glibc payload while the glibc object does. TOOL.md section 6.7 does not say"
  echo "  this; TODO/interpose.md T-0702 carries it."
else
  echo "  the musl object loaded into a glibc payload. T-0702's premise is"
  echo "  wrong as stated and the entry takes the correction underneath it."
  rc=1
fi

exit "$rc"
