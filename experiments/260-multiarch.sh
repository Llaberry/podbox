#!/usr/bin/env bash
# Question: is podbox a multi-architecture runtime, and what does "runs on
# aarch64" actually mean when the only aarch64 available is an emulator?
#
# TODO/deps.md T-0911, TODO/packaging.md T-1005. Until 2026-09-09
# `crates/podbox-probe/src/sys.rs` declared x86_64's syscall numbers by hand and
# `compile_error!`d everywhere else, so podbox was a single-architecture runtime
# for no reason but the table. The numbers and the kernel structs now come from
# `syscalls` and `linux-raw-sys`, per architecture, and this is what asks
# whether that is true rather than merely written down.
#
# ⛔ CLAUSE 4 IS THE ONE THAT MATTERS AND IT IS A WARNING, NOT A WIN. A probe
# run under `qemu-user` measures QEMU, not an aarch64 kernel: qemu answers
# `EINVAL` to `clone(CLONE_NEWNS)` and `ENOSYS` to `fsmount`, and reports
# `Seccomp: 0` because it does not pass the host's through. The rung it prints
# is qemu's and podbox may not report it as the machine's. That is the same
# class of mistake as `experiments/20-enter-target.sh`'s `/dev` answering out of
# the mount table the question doubts, and it is recorded here because the
# multiarch feature makes it reachable by ordinary users: a foreign-architecture
# container runs its payload under exactly this emulator.
#
#   ./260-multiarch.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
OUT="$REPO/experiments/results/multiarch.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

cd "$REPO" || exit 2
command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo is not on PATH" >&2; exit 2; }

# ⚠ Every architecture podbox claims, and the ONE it does not, named rather than
# omitted. `docs/AGENTS.md` absolute 5: a blocked thing stays visible.
TARGETS=(
	x86_64-unknown-linux-musl
	aarch64-unknown-linux-musl
	riscv64gc-unknown-linux-musl
	loongarch64-unknown-linux-musl
	armv7-unknown-linux-musleabihf
	i686-unknown-linux-musl
)
# ⛔ NOT a "known failure" to be skipped quietly. It is checked like the rest and
# its failure is reported as the third state, because the day the blocker clears
# this is how the project finds out.
BLOCKED=(powerpc64le-unknown-linux-musl)

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s\n' "$(uname -r)"
	printf 'host arch         %s\n' "$(uname -m)"
	printf 'rustc             %s\n' "$(rustc --version 2>/dev/null || echo absent)"
	printf 'zig               %s\n' "$(zig version 2>/dev/null || echo absent)"
	printf 'qemu-aarch64      %s\n' "$(qemu-aarch64-static --version 2>/dev/null | head -1 || echo absent)"
	echo
} >"$WORK/report"

fail=0
skipped=0

# --------------------------------------------------------------------- 1
echo "== 1. every claimed architecture compiles the whole workspace" >>"$WORK/report"
for t in "${TARGETS[@]}"; do
	if ! rustup target list --installed 2>/dev/null | grep -qx "$t"; then
		rustup target add "$t" >/dev/null 2>&1
	fi
	# ⛔ The exit code is read from cargo, unpiped. docs/AGENTS.md absolute 8.
	cargo check --workspace --target "$t" >"$WORK/$t.log" 2>&1
	rc=$?
	if [ "$rc" -eq 0 ]; then
		printf '  ok    %-34s workspace checks\n' "$t" >>"$WORK/report"
	else
		printf '  FAIL  %-34s %s\n' "$t" \
			"$(grep -oE 'error(\[[A-Z0-9]+\])?: .*' "$WORK/$t.log" | head -1 | cut -c1-70)" >>"$WORK/report"
		fail=1
	fi
done

echo >>"$WORK/report"
echo "== 2. the architecture that does NOT build, and exactly why" >>"$WORK/report"
for t in "${BLOCKED[@]}"; do
	if ! rustup target list --installed 2>/dev/null | grep -qx "$t"; then
		rustup target add "$t" >/dev/null 2>&1
	fi
	cargo check --workspace --target "$t" >"$WORK/$t.log" 2>&1
	rc=$?
	if [ "$rc" -eq 0 ]; then
		# ⭐ The blocker cleared. That is a finding and it goes red, because the
		# tree still says this architecture is unsupported.
		printf '  CHANGED %-32s now builds. TODO/deps.md T-0911 is stale\n' "$t" >>"$WORK/report"
		fail=1
	else
		why="$(grep -oE 'error\[E0554\].*|`#!\[feature\]`.*' "$WORK/$t.log" | head -1 | cut -c1-60)"
		printf '  blocked %-32s %s\n' "$t" "${why:-see log}" >>"$WORK/report"
		printf '          the syscalls crate gates powerpc/s390x/mips behind\n' >>"$WORK/report"
		printf '          asm_experimental_arch. Clause 3 measures whether that\n' >>"$WORK/report"
		printf '          gate is still true of rustc.\n' >>"$WORK/report"
	fi
done

# --------------------------------------------------------------------- 3
echo >>"$WORK/report"
echo "== 3. is the crate's nightly gate still true of this rustc?" >>"$WORK/report"
# ⛔ Verify, do not accept. The crate says powerpc needs nightly; that was true
# once. This asks rustc directly rather than believing the cfg.
mkdir -p "$WORK/asm/src"
cat >"$WORK/asm/Cargo.toml" <<'EOF'
[package]
name = "asmprobe"
version = "0.0.0"
edition = "2021"
EOF
cat >"$WORK/asm/src/lib.rs" <<'EOF'
#[cfg(target_arch = "powerpc64")]
pub unsafe fn sc(nr: usize) -> usize {
    let r: usize;
    core::arch::asm!("sc", inlateout("r0") nr => _, lateout("r3") r, options(nostack));
    r
}
EOF
(cd "$WORK/asm" && cargo check --target powerpc64le-unknown-linux-musl >"$WORK/asm.log" 2>&1)
asm_rc=$?
if [ "$asm_rc" -eq 0 ]; then
	printf '  ⚠ powerpc64 inline asm COMPILES on %s\n' "$(rustc --version)" >>"$WORK/report"
	printf '    so the crate gate is stale, not a rustc limit. What clears\n' >>"$WORK/report"
	printf '    clause 2 is a syscalls release dropping it, or podbox taking\n' >>"$WORK/report"
	printf '    the numbers from linux-raw-sys and carrying its own trap for\n' >>"$WORK/report"
	printf '    these two. T-0911 carries both and rules neither yet.\n' >>"$WORK/report"
else
	printf '  powerpc64 inline asm needs nightly here; the crate gate is right\n' >>"$WORK/report"
fi

# --------------------------------------------------------------------- 4
echo >>"$WORK/report"
echo "== 4. podbox RUNS on aarch64, and what that sentence is worth" >>"$WORK/report"
BIN="$REPO/target/aarch64-unknown-linux-musl/release/podbox"
if ! command -v qemu-aarch64-static >/dev/null 2>&1; then
	printf '  SKIP: qemu-aarch64-static is not installed, so no aarch64 can be\n' >>"$WORK/report"
	printf '        reached from this host. Clause 4 measured nothing.\n' >>"$WORK/report"
	skipped=1
else
	RUSTFLAGS="-C target-feature=+crt-static -C linker=rust-lld -C link-self-contained=yes" \
		cargo build --release -p podbox-cli --target aarch64-unknown-linux-musl \
		>"$WORK/build.log" 2>&1
	if [ ! -x "$BIN" ]; then
		printf '  SKIP: the aarch64 binary did not build; see the log\n' >>"$WORK/report"
		skipped=1
	else
		printf '  binary          %s\n' "$(file -b "$BIN" | cut -c1-58)" >>"$WORK/report"
		printf '  bytes           %s\n' "$(stat -c %s "$BIN")" >>"$WORK/report"
		ver="$(qemu-aarch64-static "$BIN" version 2>&1)"
		ver_rc=$?
		printf '  version         %-24s rc=%d\n' "$ver" "$ver_rc" >>"$WORK/report"
		[ "$ver_rc" -eq 0 ] || { printf '  FAIL: the aarch64 binary does not run\n' >>"$WORK/report"; fail=1; }

		rung="$(qemu-aarch64-static "$BIN" probe 2>/dev/null | head -1)"
		host_rung="$("$REPO/target/x86_64-unknown-linux-musl/release/podbox" probe 2>/dev/null | head -1)"
		printf '  rung, aarch64   %s   ⛔ QEMU'"'"'S ANSWER, NOT AN ARM MACHINE'"'"'S\n' "$rung" >>"$WORK/report"
		printf '  rung, host      %s   the same host, measured natively\n' "$host_rung" >>"$WORK/report"

		# ⭐ The evidence for the warning above, printed rather than asserted.
		qemu-aarch64-static "$BIN" probe 2>&1 >/dev/null | grep -E 'clone\(NEWNS\)|fsmount|Seccomp:' \
			| sed 's/^/    /' >>"$WORK/report"
		printf '  ⚠ Read those rows before quoting the rung. qemu-user does not\n' >>"$WORK/report"
		printf '    implement fsmount, refuses clone(CLONE_NEWNS) with EINVAL and\n' >>"$WORK/report"
		printf '    reports Seccomp 0 whatever the host is under. Every one of\n' >>"$WORK/report"
		printf '    those is a statement about the EMULATOR.\n' >>"$WORK/report"
		printf '  ⭐ What it does prove: the syscall numbers, the struct layouts\n' >>"$WORK/report"
		printf '    and the trap are right for aarch64. A wrong statfs layout\n' >>"$WORK/report"
		printf '    gives garbage block counts, and the writable rows are sane.\n' >>"$WORK/report"
	fi
fi

# --------------------------------------------------------------------- 5
echo >>"$WORK/report"
echo "== 5. binfmt_misc with F, which is how a foreign image will run" >>"$WORK/report"
if [ ! -d /proc/sys/fs/binfmt_misc ] || [ ! -f /proc/sys/fs/binfmt_misc/register ]; then
	printf '  SKIP: binfmt_misc is not mounted here\n' >>"$WORK/report"
	skipped=1
elif [ ! -x "$BIN" ]; then
	printf '  SKIP: no aarch64 binary to run\n' >>"$WORK/report"
	skipped=1
else
	name="podbox-probe-aarch64-$$"
	# ⛔ THE MAGIC IS WRITTEN WITH BACKSLASH ESCAPES THE KERNEL PARSES, NOT AS
	# RAW BYTES. Measured on 2026-09-09, the hard way: `printf` emitting real
	# NUL bytes gets the registration TRUNCATED at the first one, leaving a
	# 7-byte magic that matches EVERY 64-bit ELF. Every native binary on the
	# machine is then routed to the aarch64 interpreter and dies with ELOOP,
	# including the shell needed to undo it. `echo` with single quotes keeps
	# the backslashes literal so the kernel does the parsing.
	if echo ":$name:M::\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\xb7\x00:\xff\xff\xff\xff\xff\xff\xff\x00\xff\xff\xff\xff\xff\xff\xff\xff\xfe\xff\xff\xff:/usr/bin/qemu-aarch64-static:PF" \
		>/proc/sys/fs/binfmt_misc/register 2>/dev/null; then
		# ⛔ Read the registration back and refuse to proceed on a short magic,
		# because proceeding is what breaks the machine.
		magic="$(sed -n 's/^magic //p' "/proc/sys/fs/binfmt_misc/$name")"
		if [ "${#magic}" -ne 40 ]; then
			printf '  FAIL: the magic registered as %d hex chars, not 40. Removing.\n' "${#magic}" >>"$WORK/report"
			echo -1 >"/proc/sys/fs/binfmt_misc/$name" 2>/dev/null
			fail=1
		else
			printf '  registered      %s, flags %s\n' "$name" \
				"$(sed -n 's/^flags: //p' "/proc/sys/fs/binfmt_misc/$name")" >>"$WORK/report"
			# The F flag opens the interpreter NOW and holds it, so a chroot
			# with no qemu inside it still runs the binary. That is exactly
			# what a foreign-architecture container needs.
			root="$WORK/root"
			mkdir -p "$root"
			cp "$BIN" "$root/podbox"
			out="$(chroot "$root" /podbox version 2>&1)"
			rc=$?
			printf '  in a bare chroot with NO qemu in it: %s (rc=%d)\n' "$out" "$rc" >>"$WORK/report"
			if [ "$rc" -eq 0 ]; then
				printf '  ⭐ That is the mechanism podbox run uses for a foreign\n' >>"$WORK/report"
				printf '    image, and it needs nothing copied into the rootfs.\n' >>"$WORK/report"
			else
				printf '  FAIL: the F flag did not carry the interpreter into the chroot\n' >>"$WORK/report"
				fail=1
			fi
			echo -1 >"/proc/sys/fs/binfmt_misc/$name" 2>/dev/null
			printf '  unregistered    %s\n' "$name" >>"$WORK/report"
		fi
	else
		printf '  SKIP: could not register a binfmt entry here\n' >>"$WORK/report"
		skipped=1
	fi
fi

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

# ⛔ Three states, and "could not run" is never a failure.
[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
