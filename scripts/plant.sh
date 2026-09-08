#!/usr/bin/env bash
# plant.sh - break each of the gate's checks on purpose and assert it goes red.
#
# ⛔ AN ASSERTION NOBODY HAS SEEN FAIL IS NOT AN ASSERTION. `check-todo.py`
# carries eighteen checks and this script carries twenty cases, because
# check 17 has three assertions that fail apart and check 18 two. A check that
# quietly matches nothing exits 0 exactly like one whose assertions all passed,
# and the second is what everybody assumes they are looking at. This script is
# what tells them apart.
#
# ⭐ READ THE FINDING, NOT THE EXIT CODE. A gate already red for another reason
# exits 1 either way. Each case here asserts that the planted defect's OWN
# message appears and that it did not appear on the clean tree. A case that
# only watched the exit code passes vacuously the moment anything else breaks.
#
# Four guards, each of which exists because the harness shape without it
# reports success while planting nothing:
#
#   1. THE MUTATION MUST LAND. A `sed` that matches nothing leaves the source
#      correct, the gate green, and the case reads as a missing assertion when
#      what is missing is the plant. Asserted by hashing the files before and
#      after.
#   2. RESTORE FROM A COPY, NEVER `git checkout --`. Checkout restores from the
#      INDEX, so anything a plant staged survives it, and a run started while
#      work is staged restores the plant instead of the source. Measured: with
#      a plant staged, `git checkout -- f` leaves the plant in the worktree.
#   3. ONE FILE LIST. A restore that carries its own second copy of the list
#      stops putting back the first file a new case learns to touch, and every
#      later case then measures the leftovers.
#   4. CONTROLS ARE COUNTED APART FROM PLANTS. A control stays quiet; a plant
#      goes red. Adding them together reports more defects caught than were.
#
# Exit: 0 every plant was caught and every control stayed quiet, 1 a plant was
#       missed or a control fired, 2 could not run.
set -uo pipefail

HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$HERE/.." && pwd)"
BACKUP="$(mktemp -d)"
GATE="$ROOT/scripts/check-todo.py"

cd "$ROOT" || { echo "SKIP: cannot enter $ROOT" >&2; exit 2; }
[ -x "$GATE" ] || { echo "SKIP: $GATE is not executable" >&2; exit 2; }
command -v git >/dev/null 2>&1 || { echo "SKIP: no git" >&2; exit 2; }

# ⛔ GUARD 3: ONE LIST. Everything any case may touch is named here once, and
# both the backup and the restore iterate this and nothing else.
FILES="TODO/INDEX.md TODO/PROGRESS.md TODO/probe.md TODO/reference-map.md README.md docs/conventions/prose.md experiments/110-bloat-delta.sh experiments/results/bloat-baseline.txt"

# ⛔ CHECK 18'S SUBJECT IS A NUMBER THAT IS ALREADY TAKEN, so writing one here
# literally would put a second name on it in this very file and make the clean
# tree red, exactly as two literal citations did on 2026-09-08 and as
# CEILING_NUM below would. It is read out of the listing at run time.
# ⚠ No pipe: `set -o pipefail` is on above, and `ls | head -1` returns `ls`'s
# SIGPIPE status. A glob and a `break` need neither.
TAKEN_EXP=""
for _f in experiments/[0-9]*-*.sh; do
  [ -e "$_f" ] || continue
  _b="${_f##*/}"; TAKEN_EXP="${_b%%-*}"; break
done
if [ -z "$TAKEN_EXP" ]; then
  echo "SKIP: experiments/ carries no numbered script to collide with" >&2
  exit 2
fi
export TAKEN_EXP
DUP_PLANT="experiments/${TAKEN_EXP}-plantdup.sh"

# ⛔ Refuse to start over staged work. Guard 2 makes the restore safe, but a
# dirty index means the "clean" baseline below is not clean, and every case
# then compares against a tree somebody was mid-edit on.
if ! git diff --quiet -- $FILES || ! git diff --cached --quiet -- $FILES; then
  echo "SKIP: one of the files this script plants into has uncommitted changes:" >&2
  git status --porcelain -- $FILES >&2
  echo "Commit or stash them. This script must own the tree while it runs." >&2
  exit 2
fi

plants_caught=0; plants_missed=0
controls_quiet=0; controls_fired=0

backup() { for f in $FILES; do mkdir -p "$BACKUP/$(dirname "$f")"; cp "$ROOT/$f" "$BACKUP/$f"; done; }
# ⛔ RESTORE UNDOES EVERY KIND OF PLANT, not only an edit to a file in the list.
# Case 15 creates a file and stages it, so putting the listed files back is not
# enough: an unstaged leftover makes every later case measure a dirty tree.
SCRATCH_PLANT="experiments/.plantscratch"
restore() {
  for f in $FILES; do cp "$BACKUP/$f" "$ROOT/$f"; done
  git -C "$ROOT" rm -q --cached -f --ignore-unmatch -r "$SCRATCH_PLANT" >/dev/null 2>&1
  rm -rf "${ROOT:?}/$SCRATCH_PLANT"
  # ⚠ Case 18b stages a NEW experiment script, so the listed files going back is
  # not enough here either.
  git -C "$ROOT" rm -q --cached -f --ignore-unmatch "$DUP_PLANT" >/dev/null 2>&1
  rm -f "${ROOT:?}/$DUP_PLANT"
}

# ⛔ HASH THE WHOLE WORKING STATE, not just the listed files. Guard 1 asserts
# the mutation landed; a plant that creates a NEW file changes nothing in the
# list, so a list-only hash would report "did not land" on a plant that did.
hashes() {
  for f in $FILES; do git hash-object "$ROOT/$f"; done
  git -C "$ROOT" status --porcelain
}

trap 'restore; rm -rf "$BACKUP"' EXIT INT TERM
backup

echo "== baseline: the gate on the clean tree"
base_out="$("$GATE" 2>&1)"; base_rc=$?
printf '  exit %s\n' "$base_rc"
if [ "$base_rc" -ne 0 ]; then
  echo "SKIP: the gate is already red. Nothing below would be interpretable." >&2
  printf '%s\n' "$base_out" | sed 's/^/    /' >&2
  exit 2
fi
echo

# case NAME EXPECTED-SUBSTRING COMMAND...
#   The expected substring is the planted defect's OWN message. A case that
#   matched only the exit code would pass on any unrelated failure.
case_plant() {
  local name="$1" want="$2"; shift 2
  local before after out rc
  before="$(hashes)"
  "$@"
  after="$(hashes)"
  # ⛔ GUARD 1: did the mutation land?
  if [ "$before" = "$after" ]; then
    printf '  MISS %-34s the mutation did not land; the files are byte-identical\n' "$name"
    plants_missed=$((plants_missed + 1)); restore; return
  fi
  out="$("$GATE" 2>&1)"; rc=$?
  restore
  if [ "$rc" -eq 0 ]; then
    printf '  MISS %-34s the gate stayed green\n' "$name"
    plants_missed=$((plants_missed + 1)); return
  fi
  # ⭐ The finding, not the exit code: this message, and not on the clean tree.
  case "$out" in
    *"$want"*)
      case "$base_out" in
        *"$want"*)
          printf '  MISS %-34s that message is already on the clean tree\n' "$name"
          plants_missed=$((plants_missed + 1)) ;;
        *)
          printf '  ok   %-34s red, and it named it\n' "$name"
          plants_caught=$((plants_caught + 1)) ;;
      esac ;;
    *)
      printf '  MISS %-34s red for another reason, not this one\n' "$name"
      printf '%s\n' "$out" | sed -n '2,4p' | sed 's/^/         /'
      plants_missed=$((plants_missed + 1)) ;;
  esac
}

# A control changes something the gate must NOT object to. It proves the gate
# is discriminating rather than merely noisy.
case_control() {
  local name="$1"; shift
  local out rc
  "$@"
  out="$("$GATE" 2>&1)"; rc=$?
  restore
  if [ "$rc" -eq 0 ]; then
    printf '  ok   %-34s quiet, as a control must be\n' "$name"
    controls_quiet=$((controls_quiet + 1))
  else
    printf '  FIRE %-34s the gate objected to a legitimate edit\n' "$name"
    printf '%s\n' "$out" | sed -n '2,4p' | sed 's/^/         /'
    controls_fired=$((controls_fired + 1))
  fi
}

# ⛔ THE PLANTED CITATIONS ARE ASSEMBLED AT RUN TIME, NEVER WRITTEN OUT HERE.
# The gate reads this file like any other, so a literal bad citation in this
# source is a permanent gate failure rather than a plant. Caught exactly that
# way on 2026-09-08: two literals here reddened the clean-tree baseline, and
# the harness then refused to run at all.
G_PATH="scripts/check-todo""$(printf '.py')"
export BAD_LINE_A="$G_PATH:999999"
export BAD_LINE_B="$G_PATH:888888"
export BAD_BARE="TODO/no-such-""category.md"

# ⛔ SAME RULE, FOR CHECK 17. Its subject is a NUMBER that may appear in exactly
# one tracked file, so writing that number literally in this source would make
# the clean tree red and the harness would refuse to run, exactly as two
# literal citations did on 2026-09-08. It is read out of the one file that owns
# it, at run time.
CEILING_NUM="$(awk -F= '/^CEILING_BYTES=/{print $2}' experiments/110-bloat-delta.sh)"
if [ -z "$CEILING_NUM" ]; then
  echo "SKIP: experiments/110-bloat-delta.sh declares no CEILING_BYTES" >&2
  exit 2
fi
export CEILING_NUM

echo "== plants"

case_plant "1 row without an entry" "has a row and no entry" \
  sh -c 'printf "| [T-9999](probe.md) | P0 | probe | open | A row naming nothing |\n" >> TODO/INDEX.md'

case_plant "2 entry without a row" "has an entry and no row" \
  sh -c 'printf "\n---\n\n### T-9998 An entry no row names\n\nStatus:      open\n" >> TODO/probe.md'

# ⚠ STATUS-AGNOSTIC, for the same reason cases 4 and 10 are count-agnostic.
# This case named `open` literally and rotted the moment probe.md had no open
# entry left, which is what closing M0 did: guard 1 reported "the mutation did
# not land" rather than a silent green, and that is the whole reason guard 1
# exists. A plant must mutate whatever is there.
case_plant "3 status disagreement" "disagrees with the index" \
  sh -c 'from=$(grep -m1 -oE "^Status:      [a-z]+" TODO/probe.md | awk "{print \$2}");
         to=blocked; [ "$from" = blocked ] && to=open;
         sed -i -E "0,/^Status:      $from/s//Status:      $to/" TODO/probe.md'

# ⚠ COUNT-AGNOSTIC. These two cases named the counts literally and rotted the
# first time the counts moved: guard 1 reported "the mutation did not land"
# rather than a silent green, which is the whole reason guard 1 exists. A plant
# must mutate whatever number is there.
case_plant "4 count block is stale" "the totals line is not the rows" \
  sh -c 'sed -i -E "s/^([0-9]+) items: [0-9]+ open/\\1 items: 999 open/" TODO/INDEX.md'

case_plant "5 a missing field" "has no \`Decision:\` field" \
  sh -c 'sed -i "0,/^Decision:/s//Decisionx:/" TODO/probe.md'

# ⚠ EVERY mention, not the first row. Each tree is named twice in the map, in
# the licence table and again in the verdicts table, so deleting one row leaves
# the other and check 6 never loses the tree. Measured on 2026-09-08: this case
# landed its mutation and stayed green until it deleted both.
case_plant "6 a corpus tree the map lost" "is on disk and the map does not name it" \
  sh -c 'sed -i "s|\`references/qaidvoid__onelf\`|the onelf tree|g" TODO/reference-map.md'

case_plant "7 a citation past end of file" "lines" \
  sh -c 'printf "\nSee \`%s\` for this.\n" "$BAD_LINE_A" >> TODO/probe.md'

case_plant "8 a TODO link to nothing" "does not resolve" \
  sh -c 'printf "\nSee [nothing](no-such-file.md).\n" >> TODO/probe.md'

case_plant "9 a T-NNNN naming nothing" "which is not an entry" \
  sh -c 'printf "\nBlocked behind T-8888.\n" >> TODO/probe.md'

case_plant "10 PROGRESS count is stale" "the count line is not the rows" \
  sh -c 'sed -i -E "s/^([0-9]+) entries: [0-9]+ open/\\1 entries: 999 open/" TODO/PROGRESS.md'

case_plant "11 a tree citation past EOF" "lines" \
  sh -c 'printf "\nSee \`%s\` for the gate.\n" "$BAD_LINE_B" >> README.md'

case_plant "12 a link out of README" "does not resolve" \
  sh -c 'printf "\n[gone](docs/no-such-page.md)\n" >> README.md'

case_plant "13 a tenth dangling link" "dangling link" \
  sh -c 'printf "\n[gone](../../scripts/no-such-template-script.sh)\n" >> docs/conventions/prose.md'

case_plant "14 a bare path naming nothing" "git tracks no such file" \
  sh -c 'printf "\nThe work is in \`%s\`.\n" "$BAD_BARE" >> README.md'

# ⚠ Case 15 stages a file INSIDE a scratch directory and must force-add it,
# because .gitignore is what stops it in normal use. The plant is that the
# ignore rule was wrong, which is how the defect actually happened.
case_plant "15 tracked experiment scratch" "build artefact or experiment scratch" \
  sh -c 'mkdir -p experiments/.plantscratch && : > experiments/.plantscratch/x.bin && git add -f experiments/.plantscratch/x.bin'

# ⚠ Check 17 has three assertions and three cases, because they fail apart: a
# second copy of the number, a baseline that outgrew it, and a baseline with
# nothing in it to compare. One case would leave two of them unseen, which is
# the vacuity this whole harness exists to catch.
case_plant "17a the ceiling in a second file" "names the binary size ceiling" \
  sh -c 'printf "\nThe release binary must stay under %s bytes.\n" "$CEILING_NUM" >> README.md'

case_plant "17b a baseline over the ceiling" "at or over the ceiling" \
  sh -c 'sed -i -E "s/^total_bytes [0-9]+$/total_bytes ${CEILING_NUM}/" experiments/results/bloat-baseline.txt'

case_plant "17c a baseline with no total" "carries no \`total_bytes <n>\` line" \
  sh -c 'sed -i -E "s/^total_bytes /total_bytes_renamed /" experiments/results/bloat-baseline.txt'

# ⚠ Check 18 has two cases because its two halves fail apart, and the four real
# collisions were all the first kind: a document promising a script at a number
# something else already answers to. The second kind is the same number arriving
# on disk. A case for one would leave the other unseen.
# ⛔ NO BACKTICKS in 18a's planted line, deliberately: in backticks it is also a
# bare path naming nothing, check 14 fires too, and the case would pass on the
# wrong message.
case_plant "18a a Prove at a taken number" "Give the new one a free number" \
  sh -c 'printf "\nThe loop is driven by experiments/%s-plantdup.sh, once it exists.\n" "$TAKEN_EXP" >> README.md'

case_plant "18b a second script on disk" "Give the new one a free number" \
  sh -c 'printf "#!/bin/sh\n# a plant\n" > "experiments/${TAKEN_EXP}-plantdup.sh" && git add "experiments/${TAKEN_EXP}-plantdup.sh"'

echo
# ⛔ SAY WHAT IS NOT COVERED. A harness that lists passing cases without naming
# the check that has none implies a coverage it does not have, which is the same
# vacuity it exists to catch.
echo "== not planted against"
echo "  16 coverage floor    no case. Planting it means making a check examine"
echo "                       nothing, which requires editing check-todo.py's own"
echo "                       matchers rather than the tree. Every other check's"
echo "                       counter is asserted non-zero on every run instead,"
echo "                       and a zero is reported as a failure."
echo
echo "== controls"

case_control "an ordinary prose edit" \
  sh -c 'printf "\nOne more sentence that cites nothing.\n" >> README.md'

case_control "a citation that does resolve" \
  sh -c 'printf "\nThe gate is \`scripts/check-todo.py:1\`.\n" >> README.md'

case_control "a forward reference to a result" \
  sh -c 'printf "\nIt will land in \`experiments/results/not-yet-taken.txt\`.\n" >> README.md'

echo
echo "== verdict"
printf '  plants   %s caught, %s missed\n' "$plants_caught" "$plants_missed"
printf '  controls %s quiet, %s fired\n' "$controls_quiet" "$controls_fired"
# ⛔ GUARD 4: these two are reported apart. A single "defects caught" number
# adds the controls in and overstates the coverage by exactly their count.
if [ "$plants_missed" -eq 0 ] && [ "$controls_fired" -eq 0 ]; then
  echo "  every check that was planted against went red with its own message."
  exit 0
fi
echo "  see the MISS and FIRE lines above."
exit 1
