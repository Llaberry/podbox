#!/usr/bin/env bash
# Question: which libc entry points must `podbox-interpose` define, and can
# that question be answered by a command rather than by a list somebody
# maintains?
#
# A list is what rots. Every entry point a payload can reach and the interposer
# does not define is a path that goes through untranslated, and the failure it
# produces points at the payload rather than at the interposer: one measured
# case is a program finding its data files and then failing to import a module
# from the same directory, because the finder asks `stat` and the loader asks
# `open`.
#
# Four checks:
#
#   A. an interposer that defines `execve` alone catches `execve` alone. Seven
#      callers, one row each.
#   B. ⛔ THE CONTROL. The same seven with every entry point defined. Without
#      it, A's zeros are equally consistent with a broken harness.
#   C. the adopted mechanism's real exported set, read from a built object
#      rather than from its source, because the definitions are macro-generated
#      and a grep of the source undercounts them.
#   D. ⭐ the completeness test. For a pinned rootfs, enumerate the path-taking
#      libc symbols its own binaries import, and subtract what the interposer
#      defines. The remainder is the gap, named. This is the check podbox
#      keeps; A, B and C are what establish that it is worth keeping.
#
# Inputs pinned: this host's toolchain and libc, the pathmap tree at the commit
# TODO/reference-map.md records, and for D:
#   debian@sha256:2f65600e1252c5649d2213e1d1ea4d74253d26514dc6530102a875e429245929
#
# Exit: 0 every check ran and matched, 1 a check ran and did not match,
#       2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
OUT="${OUT:-$HERE/.symbols}"
rm -rf "$OUT"; mkdir -p "$OUT"

SRC="$ROOT/references/VHSgunzo__pathmap/tree/path-mapping.c"
ROOTFS='debian@sha256:2f65600e1252c5649d2213e1d1ea4d74253d26514dc6530102a875e429245929'

# The path-taking libc surface podbox cares about. ⚠ Every name here takes a
# PATH. `fstat`, `fexecve` and `fdopendir` take a descriptor and are therefore
# absent on purpose: there is nothing in them to rewrite.
PATH_TAKING='open open64 openat openat64 openat2 creat creat64
fopen fopen64 freopen freopen64 opendir
stat lstat stat64 lstat64 fstatat fstatat64 statx
__xstat __lxstat __xstat64 __lxstat64 __fxstatat __fxstatat64
access faccessat eaccess euidaccess
readlink readlinkat realpath canonicalize_file_name
execve execv execvp execvpe execl execlp execle
posix_spawn posix_spawnp
dlopen dlmopen
mkdir mkdirat rmdir unlink unlinkat rename renameat renameat2
link linkat symlink symlinkat chdir chown lchown chmod truncate
utime utimes utimensat statfs statvfs mount umount umount2
mkstemp mkdtemp tmpfile glob scandir'

echo "== conditions"
printf 'date              %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf 'host kernel       %s\n' "$(uname -r)"
_ldd="$(ldd --version 2>&1)"
printf 'host libc         %s\n' "${_ldd%%$'\n'*}"
printf 'cc                %s\n' "$(cc --version 2>/dev/null | head -1)"
_dv="$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
printf 'docker            %s\n' "${_dv:-MISSING}"
printf 'subject           %s\n' "references/VHSgunzo__pathmap/tree/path-mapping.c"
printf 'rootfs for D      %s\n' "$ROOTFS"
echo

command -v cc >/dev/null 2>&1 || { echo "SKIP: no cc" >&2; exit 2; }
command -v nm >/dev/null 2>&1 || { echo "SKIP: no nm" >&2; exit 2; }
[ -r "$SRC" ] || { echo "SKIP: $SRC is absent" >&2; exit 2; }

rc=0
fail() { printf 'FAIL %s\n' "$1"; rc=1; }
pass() { printf 'ok   %s\n' "$1"; }

# -- the caller: one process per exec entry point --------------------------
cat > "$OUT/caller.c" <<'EOF'
#define _GNU_SOURCE
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <spawn.h>
#include <sys/wait.h>
extern char **environ;
int main(int argc, char **argv) {
    char *av[] = {"true", NULL};
    const char *w = argc > 1 ? argv[1] : "";
    if      (!strcmp(w, "execve"))  execve("/bin/true", av, environ);
    else if (!strcmp(w, "execv"))   execv ("/bin/true", av);
    else if (!strcmp(w, "execvp"))  execvp("/bin/true", av);
    else if (!strcmp(w, "execl"))   execl ("/bin/true", "true", (char *)NULL);
    else if (!strcmp(w, "execlp"))  execlp("/bin/true", "true", (char *)NULL);
    else if (!strcmp(w, "spawn"))  { pid_t p; posix_spawn (&p, "/bin/true", NULL, NULL, av, environ); waitpid(p, NULL, 0); return 0; }
    else if (!strcmp(w, "spawnp")) { pid_t p; posix_spawnp(&p, "/bin/true", NULL, NULL, av, environ); waitpid(p, NULL, 0); return 0; }
    return 1;
}
EOF
cc -o "$OUT/caller" "$OUT/caller.c" 2>"$OUT/cc.log" || {
  echo "SKIP: cannot build the caller; see $OUT/cc.log" >&2; exit 2; }

# -- arm A: execve only ----------------------------------------------------
cat > "$OUT/only-execve.c" <<'EOF'
#define _GNU_SOURCE
#include <stdio.h>
#include <unistd.h>
#include <dlfcn.h>
int execve(const char *p, char *const a[], char *const e[]) {
    fprintf(stderr, "SEEN execve %s\n", p);
    int (*real)(const char *, char *const[], char *const[]) = dlsym(RTLD_NEXT, "execve");
    return real(p, a, e);
}
EOF
# -- arm B: every entry point ---------------------------------------------
cat > "$OUT/all-exec.c" <<'EOF'
#define _GNU_SOURCE
#include <stdio.h>
#include <stdarg.h>
#include <unistd.h>
#include <spawn.h>
#include <dlfcn.h>
static void note(const char *fn, const char *p) { fprintf(stderr, "SEEN %s %s\n", fn, p); }
#define FWD(name) __typeof__(name) *real = dlsym(RTLD_NEXT, #name)
int execve(const char *p, char *const a[], char *const e[]) { note("execve", p); FWD(execve); return real(p, a, e); }
int execv (const char *p, char *const a[])                  { note("execv",  p); FWD(execv);  return real(p, a); }
int execvp(const char *p, char *const a[])                  { note("execvp", p); FWD(execvp); return real(p, a); }
int posix_spawn(pid_t *pid, const char *p, const posix_spawn_file_actions_t *fa,
                const posix_spawnattr_t *at, char *const a[], char *const e[]) {
    note("posix_spawn", p); FWD(posix_spawn); return real(pid, p, fa, at, a, e); }
int posix_spawnp(pid_t *pid, const char *p, const posix_spawn_file_actions_t *fa,
                 const posix_spawnattr_t *at, char *const a[], char *const e[]) {
    note("posix_spawnp", p); FWD(posix_spawnp); return real(pid, p, fa, at, a, e); }
static int collect(const char *a0, va_list ap, char **av, int max) {
    int n = 0; av[n++] = (char *)a0;
    while (n < max - 1 && (av[n] = va_arg(ap, char *))) n++;
    av[n] = NULL; return n;
}
int execl(const char *p, const char *a0, ...) {
    note("execl", p); char *av[64]; va_list ap; va_start(ap, a0);
    collect(a0, ap, av, 64); va_end(ap); FWD(execv); return real(p, av); }
int execlp(const char *p, const char *a0, ...) {
    note("execlp", p); char *av[64]; va_list ap; va_start(ap, a0);
    collect(a0, ap, av, 64); va_end(ap); FWD(execvp); return real(p, av); }
EOF
cc -shared -fPIC -o "$OUT/only-execve.so" "$OUT/only-execve.c" -ldl 2>>"$OUT/cc.log" || {
  echo "SKIP: cannot build arm A" >&2; exit 2; }
cc -shared -fPIC -o "$OUT/all-exec.so"    "$OUT/all-exec.c"    -ldl 2>>"$OUT/cc.log" || {
  echo "SKIP: cannot build arm B" >&2; exit 2; }

CALLERS='execve execv execvp execl execlp spawn spawnp'

echo "== A. an interposer that defines execve alone"
a_seen=0
for m in $CALLERS; do
  n="$(LD_PRELOAD="$OUT/only-execve.so" "$OUT/caller" "$m" 2>&1 | grep -c '^SEEN ')"
  if [ "$n" -gt 0 ]; then printf '  %-8s rewritable\n' "$m"; a_seen=$((a_seen+1))
  else printf '  %-8s NOT SEEN\n' "$m"; fi
done
if [ "$a_seen" -eq 1 ]; then
  pass "A: 1 of 7 entry points reached the interposer"
else
  fail "A: $a_seen of 7 reached an execve-only interposer; expected exactly 1"
fi
echo

echo "== B. the control: the same seven with every entry point defined"
b_seen=0
for m in $CALLERS; do
  n="$(LD_PRELOAD="$OUT/all-exec.so" "$OUT/caller" "$m" 2>&1 | grep -c '^SEEN ')"
  [ "$n" -gt 0 ] && b_seen=$((b_seen+1))
  printf '  %-8s %s\n' "$m" "$([ "$n" -gt 0 ] && echo seen || echo 'NOT SEEN')"
done
if [ "$b_seen" -eq 7 ]; then
  pass "B: 7 of 7. A's zeros are the entry points, not the harness."
else
  fail "B: only $b_seen of 7 fired with every entry point defined; A is uninterpretable"
fi
echo

echo "== C. the adopted mechanism's real exported set"
# ⚠ Read from a BUILT object. The definitions are macro-generated, and three
# different macros generate them, so a grep of the source undercounts.
if cc -shared -fPIC -O1 -o "$OUT/pathmap.so" "$SRC" 2>"$OUT/pathmap.log"; then
  nm -D --defined-only "$OUT/pathmap.so" | awk '$2=="T"{print $3}' | sort -u > "$OUT/defined.txt"
  total="$(grep -c . "$OUT/defined.txt")"
  internal="$(grep -cE '^(_|pm_)' "$OUT/defined.txt")"
  printf '  exported text symbols %s (%s internal, %s libc entry points)\n' \
    "$total" "$internal" "$((total - internal))"
  missing=""
  for s in $PATH_TAKING; do
    grep -qx "$s" "$OUT/defined.txt" || missing="$missing $s"
  done
  if [ -n "$missing" ]; then
    printf '  path-taking names it does NOT define:%s\n' "$missing"
  fi
  # The exec family is the specific claim worth asserting, because it is the
  # one an interposer is most likely to under-cover.
  exec_missing=""
  for s in execve execv execvp execl execlp execle posix_spawn posix_spawnp; do
    grep -qx "$s" "$OUT/defined.txt" || exec_missing="$exec_missing $s"
  done
  if [ -z "$exec_missing" ]; then
    pass "C: every exec entry point A measured as bypassable is defined here"
  else
    fail "C: the adopted mechanism does not define$exec_missing"
  fi
else
  echo "  COULD NOT RUN: the subject does not build here; see $OUT/pathmap.log"
  rc=2
fi
echo

echo "== D. the completeness test, against a pinned rootfs"
if ! { command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; }; then
  echo "  COULD NOT RUN: no docker daemon."
  echo "  D is the check podbox keeps; A, B and C stand without it."
  [ "$rc" -eq 0 ] && exit 2
  exit "$rc"
fi
timeout 300 docker pull -q "$ROOTFS" >/dev/null 2>&1 || {
  echo "  COULD NOT RUN: cannot pull the rootfs." ; [ "$rc" -eq 0 ] && exit 2; exit "$rc"; }
# ⛔ READ THE ROOTFS FROM OUTSIDE, with this host's readelf. A reader run inside
# the image measures whatever that image happens to ship, and most images ship
# no readelf at all, so the answer becomes a property of the image's package
# list rather than of its binaries. It is also the wrong position: podbox
# extracts a rootfs and inspects it from outside, which is what this reproduces.
cid="$(timeout 180 docker create "$ROOTFS" /bin/true 2>/dev/null)"
[ -n "$cid" ] || { echo "  COULD NOT RUN: cannot create a container from the rootfs."
                   [ "$rc" -eq 0 ] && exit 2; exit "$rc"; }
mkdir -p "$OUT/rootfs"
timeout 600 docker export "$cid" 2>/dev/null \
  | tar x -C "$OUT/rootfs" --wildcards 'bin/*' 'usr/bin/*' 'lib/*' 'usr/lib/*' 2>/dev/null
timeout 60 docker rm -f "$cid" >/dev/null 2>&1
find "$OUT/rootfs" -type f 2>/dev/null | head -400 | while read -r f; do
  readelf -sW --dyn-syms "$f" 2>/dev/null | awk '$7=="UND"{print $8}'
done | sed 's/@.*//' | sort -u > "$OUT/rootfs-imports.txt"
printf '  files read from the exported rootfs: %s\n' \
  "$(find "$OUT/rootfs" -type f 2>/dev/null | head -400 | grep -c . )"
imports="$(grep -c . "$OUT/rootfs-imports.txt")"
printf '  distinct undefined symbols across the rootfs sample: %s\n' "$imports"
if [ "$imports" -eq 0 ]; then
  echo "SKIP: the rootfs sample yielded no imports, so D measured nothing" >&2
  exit 2
fi
: > "$OUT/gap.txt"
reach=0
for s in $PATH_TAKING; do
  grep -qx "$s" "$OUT/rootfs-imports.txt" || continue
  reach=$((reach+1))
  if [ -f "$OUT/defined.txt" ] && ! grep -qx "$s" "$OUT/defined.txt"; then
    echo "$s" >> "$OUT/gap.txt"
  fi
done
gap="$(grep -c . "$OUT/gap.txt" 2>/dev/null || echo 0)"
printf '  path-taking names this rootfs actually reaches: %s\n' "$reach"
printf '  of those, NOT defined by the interposer: %s\n' "$gap"
[ "$gap" -gt 0 ] && sed 's/^/     /' "$OUT/gap.txt"
if [ "$reach" -eq 0 ]; then
  fail "D: the rootfs reaches no path-taking name, which cannot be right"
else
  pass "D: the gap is a number this command produces, not a list somebody maintains"
fi
echo

echo "== verdict"
[ "$rc" -eq 0 ] && echo "  every check that ran matched" || echo "  see the FAIL lines above"
exit "$rc"
