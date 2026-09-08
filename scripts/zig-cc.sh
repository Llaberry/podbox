#!/usr/bin/env bash
# `zig cc` as a single-word C cross-compiler, for the target podbox ships.
#
# ⛔ WHY A WRAPPER AND NOT A VARIABLE. `cc-rs` and `rustc`'s linker option both
# take a PROGRAM, not a command line, so `CC="zig cc"` sets the program to
# `zig` and passes `cc` as the first source file. One executable that is
# `zig cc` is the only shape either of them accepts.
#
# ⛔ WHY IT REWRITES `-target` RATHER THAN PREPENDING ONE. Measured on
# 2026-09-08: `ring`'s build script passes `--target x86_64-unknown-linux-musl`,
# the RUST triple, and a prepended `-target` loses to it because the later flag
# wins. `zig` answers
#     error: unable to parse target query 'x86_64-unknown-linux-musl'
#            UnknownOperatingSystem
# which reads like a broken toolchain and is a four-field triple where zig
# wants three. Every `-target` is therefore removed from the command line and
# one translated triple is put back.
#
# ⛔ WHY `zig cc` AND NOT `musl-gcc`. Two reasons, both measured on 2026-09-08:
#
#   1. `musl-tools` ships no musl-linked `libgcc_s.so.1`, and `rustc` passes
#      `-lgcc_s` on the musl target even under `panic = "abort"`. That is the
#      shortage `experiments/60-interposer-libc.sh` exits 2 on, and it is an
#      open question in TODO/PROGRESS.md. `zig cc` carries its own
#      `compiler-rt`, which provides those symbols.
#   2. `musl-tools` is a host package pinned to the host's musl. `zig cc`
#      carries the musl sources it compiles against, so the same command
#      produces the same object on any machine with the same zig, which is what
#      `docs/methodology/experiments.md` means by a pinned input.
#
# ⚠ It is a cost, not a free win: zig is a ~53 MB download and a machine
# without it cannot build a C dependency at all. TODO/deps.md question 3 asks
# whether a candidate pulls C, and the answer stays "yes, and here is what that
# drags" rather than becoming "no".
#
#   CC_x86_64_unknown_linux_musl=scripts/zig-cc.sh cargo build ...
#
# `.cargo/config.toml` sets exactly that, so an ordinary `cargo build` in this
# repository already uses it. `ZIG_TARGET` overrides the triple outright.
#
# Exit: whatever `zig cc` exits, or 2 when zig is not installed.
set -u

command -v zig >/dev/null 2>&1 || {
	echo "zig-cc.sh: zig is not on PATH. Install it with" >&2
	echo "  ./scripts/common/bootstrap-env.sh zig" >&2
	exit 2
}

# Rust writes `<arch>-<vendor>-<os>-<abi>`; zig wants `<arch>-<os>-<abi>`.
# Anything already in zig's three-field shape passes through unchanged.
to_zig_triple() {
	case "$1" in
	*-*-*-*) printf '%s-%s-%s' "${1%%-*}" "$(echo "$1" | cut -d- -f3)" "${1##*-}" ;;
	*) printf '%s' "$1" ;;
	esac
}

target="${ZIG_TARGET:-x86_64-linux-musl}"
args=()
skip_next=0
for a in "$@"; do
	if [ "$skip_next" -eq 1 ]; then
		# ⚠ The caller's triple wins over this script's default, translated.
		# Overriding it would silently build for a target nobody asked for.
		[ -n "${ZIG_TARGET:-}" ] || target="$(to_zig_triple "$a")"
		skip_next=0
		continue
	fi
	case "$a" in
	-target | --target)
		skip_next=1
		;;
	--target=* | -target=*)
		[ -n "${ZIG_TARGET:-}" ] || target="$(to_zig_triple "${a#*=}")"
		;;
	*)
		args+=("$a")
		;;
	esac
done

exec zig cc -target "$target" ${args+"${args[@]}"}
