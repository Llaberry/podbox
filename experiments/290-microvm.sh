#!/usr/bin/env bash
# Question: this host's kernel has neither CONFIG_SECURITY_LANDLOCK nor
# CONFIG_CHECKPOINT_RESTORE, so three of podbox's own measurements have been
# recorded as "cannot be answered here" since M0. Can a virtual machine answer
# them instead?
#
# ⭐ IT CAN, AND THAT CLOSES TWO STANDING QUESTIONS. This boots a stock distro
# kernel under QEMU with an initramfs carrying nothing but busybox and podbox,
# runs `podbox probe --json` as pid 1's child, and reads the answers back.
#
#   TODO/probe.md T-0102       the kcmp(2) control of the bogus-argument
#                              discriminator, which answers ENOSYS here and
#                              ESRCH on the target
#   TODO/probe.md T-0101       landlock_create_ruleset(VERSION), which reports
#                              SKIP here and makes 30-attribution-census.sh
#                              exit 2
#   M0's acceptance            move_mount(-> /tmp/mm-probe), which attaches on
#                              this host and is EPERM on the target, so podbox's
#                              chroot rung was selected without the LSM ever
#                              having been present
#
# ⛔ NO KVM AND THAT IS THE POINT. This host is a Firecracker guest with no
# /dev/kvm and no vmx or svm in /proc/cpuinfo, so the VM runs under TCG, pure
# software emulation. It is slow and it is CORRECT, which is the property that
# matters: the kernel is a real kernel making real decisions.
#
# ⚠ WHAT THIS IS NOT. It is not the target runtime. It is a second, DIFFERENT
# machine whose kernel has the options this one lacks, so it answers "what does
# a kernel with Landlock say" and never "what does the target say". The
# reconstruction in `20-enter-target.sh` remains the thing that models the
# target's confinement.
#
#   ./290-microvm.sh
#
# Exit: 0 every clause held, 1 one of them did not, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO="$(CDPATH= cd -- "$HERE/.." && pwd)"
BIN="${PODBOX_BIN:-$REPO/target/x86_64-unknown-linux-musl/release/podbox}"
OUT="$REPO/experiments/results/microvm.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

# ⚠ Pinned. A different Alpine release is a different kernel and therefore a
# different reading, and `docs/methodology/experiments.md` requires a pinned
# input rather than "whatever latest is today".
ALPINE_REL="${PODBOX_MVM_ALPINE:-v3.21}"
BASE="https://dl-cdn.alpinelinux.org/alpine/$ALPINE_REL/releases/x86_64/netboot"
# ⚠ A bound on the whole boot. RULES.md section 8: TCG is slow and "slow" must
# still have a number, or a hung VM costs the session.
BOOT_TIMEOUT="${PODBOX_MVM_TIMEOUT:-600}"

[ -x "$BIN" ] || {
	echo "SKIP: $BIN is not an executable. Build it:" >&2
	echo "      cargo build --release --target x86_64-unknown-linux-musl" >&2
	exit 2
}
for t in qemu-system-x86_64 cpio jq; do
	command -v "$t" >/dev/null 2>&1 || {
		echo "SKIP: $t is not on PATH; ./scripts/common/bootstrap-env.sh" >&2
		exit 2
	}
done
BUSYBOX=""
for c in /bin/busybox /usr/bin/busybox; do [ -x "$c" ] && BUSYBOX="$c" && break; done
[ -n "$BUSYBOX" ] || { echo "SKIP: no busybox; ./scripts/common/bootstrap-env.sh" >&2; exit 2; }

fail=0
skipped=0
say() { printf '%s\n' "$*" >>"$WORK/report"; }

{
	echo "== conditions"
	printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
	printf 'host kernel       %s  (the one that CANNOT answer)\n' "$(uname -r)"
	printf 'host landlock     %s\n' \
		"$(zcat /proc/config.gz 2>/dev/null | grep -E 'CONFIG_SECURITY_LANDLOCK' || echo 'unknown')"
	printf 'host checkpoint   %s\n' \
		"$(zcat /proc/config.gz 2>/dev/null | grep -E 'CONFIG_CHECKPOINT_RESTORE' || echo 'unknown')"
	printf '/dev/kvm          %s\n' "$([ -e /dev/kvm ] && echo present || echo 'absent, so this runs under TCG')"
	printf 'qemu              %s\n' "$(qemu-system-x86_64 --version | head -1)"
	printf 'alpine netboot    %s\n' "$ALPINE_REL"
	printf 'podbox            %s\n' "$("$BIN" version)"
	echo
} >"$WORK/report"

# ------------------------------------------------------------ the kernel
for f in vmlinuz-virt initramfs-virt; do
	[ -f "$WORK/$f" ] && continue
	timeout 300 curl -sSL --fail -o "$WORK/$f" "$BASE/$f" 2>/dev/null || {
		echo "SKIP: could not fetch $BASE/$f" >&2
		cat "$WORK/report"
		exit 2
	}
done

# ------------------------------------------------------------ the initramfs
#
# ⛔ Built here rather than reusing Alpine's: theirs expects to find a repository
# and mount a root, and every one of those steps is a thing that can fail for a
# reason that has nothing to do with the question.
ROOT="$WORK/root"
mkdir -p "$ROOT"/{bin,proc,sys,dev,tmp,mnt,run,var/tmp}
cp "$BUSYBOX" "$ROOT/bin/busybox"
cp "$BIN" "$ROOT/podbox"
cat >"$ROOT/init" <<'INIT'
#!/bin/busybox sh
/bin/busybox --install -s /bin
mount -t proc none /proc
mount -t sysfs none /sys
mount -t devtmpfs none /dev
mount -t tmpfs none /tmp
mount -t securityfs none /sys/kernel/security 2>/dev/null
# ⚠ /dev/pts, so the /dev/ptmx row measures a working pty pair rather than a
# node with nothing behind it. Without this `open(/dev/ptmx)` answers ENOENT and
# that is a statement about this initramfs, not about the kernel.
mkdir -p /dev/pts && mount -t devpts none /dev/pts 2>/dev/null
echo "PODBOX-MVM-BEGIN"
echo "mvm-kernel $(uname -r)"
echo "mvm-lsm $(cat /sys/kernel/security/lsm 2>/dev/null || echo none)"
/podbox probe --json >/tmp/probe.json 2>/tmp/probe.err
echo "mvm-probe-rc $?"
echo "PODBOX-JSON-BEGIN"
cat /tmp/probe.json
echo ""
echo "PODBOX-JSON-END"
echo "PODBOX-MVM-END"
poweroff -f
INIT
chmod +x "$ROOT/init"
(cd "$ROOT" && find . | cpio -o -H newc 2>/dev/null | gzip -9 >"$WORK/initramfs.gz")

# ------------------------------------------------------------ the boot
started=$(date +%s)
timeout "$BOOT_TIMEOUT" qemu-system-x86_64 \
	-M microvm -m 1024 -smp 2 -no-reboot -nographic \
	-kernel "$WORK/vmlinuz-virt" -initrd "$WORK/initramfs.gz" \
	-append "console=ttyS0 quiet init=/init" \
	>"$WORK/console" 2>&1
qemu_rc=$?
elapsed=$(($(date +%s) - started))

say "== 1. the VM booted and podbox ran inside it"
say "  qemu exit         $qemu_rc"
say "  wall clock        ${elapsed}s, under TCG"
if ! grep -q PODBOX-MVM-END "$WORK/console"; then
	say "  SKIP: the VM did not reach the end of its init. Console tail:"
	tail -5 "$WORK/console" | sed 's/^/    /' >>"$WORK/report"
	cat "$WORK/report"
	cp "$WORK/report" "$OUT"
	exit 2
fi
say "  guest kernel      $(sed -n 's/^mvm-kernel //p' "$WORK/console" | tr -d '\r')"
say "  guest LSMs        $(sed -n 's/^mvm-lsm //p' "$WORK/console" | tr -d '\r')"

# ⛔ The JSON is carved out of a serial console, so it is validated before
# anything is read from it. A `jq` over a truncated document answers `null` for
# every question, which reads exactly like a measured absence.
sed -n '/PODBOX-JSON-BEGIN/,/PODBOX-JSON-END/p' "$WORK/console" |
	sed '1d;$d' | tr -d '\r' >"$WORK/probe.json"
if ! jq -e . "$WORK/probe.json" >/dev/null 2>&1; then
	say "  FAIL: the probe document did not survive the serial console intact"
	fail=1
	cat "$WORK/report"
	cp "$WORK/report" "$OUT"
	exit 1
fi

verdict() { jq -r --arg n "$1" '.probes[] | select(.name==$n) | .verdict' "$WORK/probe.json"; }
reason() { jq -r --arg n "$1" '.probes[] | select(.name==$n) | .reason // ""' "$WORK/probe.json"; }

# --------------------------------------------------------------------- 2
say ""
say "== 2. TODO/probe.md T-0102: the kcmp(2) control ANSWERS here"
# ⚠ `.controls.kcmp` is `null` on this host and that is CORRECT rather than a
# bug: the control could not answer, so podbox records a dash instead of a
# value (docs/AGENTS.md absolute 3) and sets `controls_answered` false. Printing
# a bare `null` reads like a defect, so the row's own errno is printed beside it.
"$BIN" probe --json >"$WORK/host.json" 2>/dev/null
host_kcmp="$(jq -r '.controls.kcmp // "none: the control could not answer"' "$WORK/host.json")"
host_kcmp_why="$(jq -r '.probes[] | select(.name|test("kcmp")) | .errno_name // ""' "$WORK/host.json")"
vm_kcmp="$(jq -r '.controls.kcmp' "$WORK/probe.json")"
answered="$(jq -r '.controls_answered' "$WORK/probe.json")"
say "  on this host      $host_kcmp ($host_kcmp_why, CONFIG_CHECKPOINT_RESTORE unset)"
say "  in the VM         $vm_kcmp   controls_answered=$answered"
say "  on the target     ESRCH   references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26"
if [ "$vm_kcmp" = "ENOSYS" ]; then
	say "  FAIL: the VM's kernel cannot answer either, so this closes nothing"
	fail=1
elif [ "$vm_kcmp" != "ESRCH" ]; then
	say "  ⚠ the VM answered $vm_kcmp where the target answers ESRCH. That is a"
	say "    finding about the two kernels and is recorded, not asserted away."
fi
[ "$answered" = "true" ] || { say "  FAIL: controls_answered is not true"; fail=1; }

# --------------------------------------------------------------------- 3
say ""
say "== 3. Landlock, which this host cannot host at all"
ll="$(verdict 'landlock_create_ruleset(VERSION)')"
say "  in the VM         $ll   $(reason 'landlock_create_ruleset(VERSION)')"
say "  on this host      $(jq -r '.probes[] | select(.name=="landlock_create_ruleset(VERSION)") | "\(.verdict) \(.errno_name // "")"' "$WORK/host.json")"
[ "$ll" = "ok" ] || { say "  FAIL: the VM's kernel has no Landlock either"; fail=1; }

# --------------------------------------------------------------------- 4
say ""
say "== 4. move_mount, the row M0's acceptance could never exercise"
mm="$(verdict 'move_mount(-> /tmp/mm-probe)')"
say "  in the VM         $mm"
say "  on this host      $(jq -r '.probes[] | select(.name=="move_mount(-> /tmp/mm-probe)") | .verdict' "$WORK/host.json")"
say "  on the target     EPERM, which is why podbox selects the chroot rung there"
[ -n "$mm" ] || { say "  FAIL: the row is missing from the VM's document"; fail=1; }

# --------------------------------------------------------------------- 5
say ""
say "== 5. the rung, and why it is NOT this VM's headline"
vm_rung="$(jq -r .rung "$WORK/probe.json")"
say "  VM rung           $vm_rung"
say "  host rung         $(jq -r .rung "$WORK/host.json")"
say "  ⚠ A VM with every capability is the EASY case and proves nothing about"
say "    confinement. What this experiment buys is the three rows above, each"
say "    of which this host reports as 'cannot answer' rather than as a value."

cat "$WORK/report"
mkdir -p "$(dirname "$OUT")"
cp "$WORK/report" "$OUT"
echo
echo "written to ${OUT#"$REPO"/}"
[ -x "$REPO/scripts/common/result-diff.sh" ] && "$REPO/scripts/common/result-diff.sh" "$OUT"

[ "$fail" -eq 0 ] || exit 1
[ "$skipped" -eq 0 ] || exit 2
exit 0
