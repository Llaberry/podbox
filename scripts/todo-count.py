#!/usr/bin/env python3
"""todo-count.py - the writer half of docs/methodology/work-todo.md's two scripts.

It re-derives every count in TODO/INDEX.md from the rows and rewrites the
Counts block in place, and with `--set T-NNNN <status>` it moves one entry's
status in both the row and the entry so the two cannot drift apart.

⛔ DO NOT DO THIS ARITHMETIC BY HAND. Closing one entry moves the totals line,
one priority row, that row's total, and the All row. The reader,
scripts/check-todo.py, asserts independently that this script's output is what
the rows say, so a hand edit that gets one of the four right and one wrong
fails the gate rather than reaching a commit.

Usage:
  scripts/todo-count.py                       rewrite the counts from the rows
  scripts/todo-count.py --check               print them, change nothing
  scripts/todo-count.py --set T-0101 done     move one status, then rewrite

⛔ Read the exit code from this process, unpiped.
Exit: 0 written or already correct, 1 refused, 2 could not run.
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TODO = os.path.join(ROOT, "TODO")
INDEX = os.path.join(TODO, "INDEX.md")

STATUSES = ["open", "partial", "blocked", "done"]
PRIORITIES = ["P0", "P1", "P2", "P3"]
ROW = re.compile(
    r"^(\|\s*\[(T-\d{4})\]\([^)]+\)\s*\|\s*)(P\d)(\s*\|\s*[a-z-]+\s*\|\s*)"
    r"(\*{0,2}[a-z]+\*{0,2})(\s*\|.*)$"
)


def status_of(cell):
    return cell.strip("*").strip()


def render(cell, status):
    """Keep the row's own emphasis. bit-cli bolds a status a session changed;
    the marker carries no meaning to this script and is not invented here."""
    return f"**{status}**" if cell.startswith("**") else status


def main(argv):
    if not os.path.isfile(INDEX):
        print("todo-count: TODO/INDEX.md does not exist", file=sys.stderr)
        return 2

    check_only = "--check" in argv
    set_id = set_status = None
    if "--set" in argv:
        i = argv.index("--set")
        if len(argv) < i + 3:
            print("todo-count: --set needs an id and a status", file=sys.stderr)
            return 2
        set_id, set_status = argv[i + 1], argv[i + 2]
        if not re.fullmatch(r"T-\d{4}", set_id):
            print(f"todo-count: {set_id} is not a T-NNNN id", file=sys.stderr)
            return 2
        if set_status not in STATUSES:
            print(f"todo-count: {set_status!r} is not one of {STATUSES}", file=sys.stderr)
            return 2

    with open(INDEX, encoding="utf-8") as fh:
        lines = fh.read().splitlines()

    # -- move one status, in the row and in the entry ------------------------
    moved = False
    if set_id:
        for n, line in enumerate(lines):
            m = ROW.match(line)
            if m and m.group(2) == set_id:
                lines[n] = m.group(1) + m.group(3) + m.group(4) + \
                    render(m.group(5), set_status) + m.group(6)
                moved = True
        if not moved:
            print(f"todo-count: {set_id} has no row in TODO/INDEX.md", file=sys.stderr)
            return 1
        hits = 0
        for name in sorted(os.listdir(TODO)):
            if not name.endswith(".md"):
                continue
            path = os.path.join(TODO, name)
            with open(path, encoding="utf-8") as fh:
                text = fh.read()
            if not re.search(rf"^### {set_id} ", text, re.M):
                continue
            head = text.index(f"### {set_id} ")
            nxt = text.find("\n### T-", head + 1)
            nxt = len(text) if nxt < 0 else nxt
            body = text[head:nxt]
            new, k = re.subn(r"^(Status: +)\*{0,2}[a-z]+\*{0,2}",
                             lambda mm: mm.group(1) + set_status, body, count=1, flags=re.M)
            if k:
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write(text[:head] + new + text[nxt:])
                hits += k
        if hits == 0:
            print(f"todo-count: {set_id} has a row but no entry to move", file=sys.stderr)
            return 1

    # -- derive --------------------------------------------------------------
    derived = {s: 0 for s in STATUSES}
    per_prio = {p: {s: 0 for s in STATUSES} for p in PRIORITIES}
    bad = 0
    for line in lines:
        m = ROW.match(line)
        if not m:
            continue
        prio, status = m.group(3), status_of(m.group(5))
        if status not in STATUSES or prio not in PRIORITIES:
            print(f"todo-count: row {m.group(2)} has status {status!r} priority {prio!r}",
                  file=sys.stderr)
            bad += 1
            continue
        derived[status] += 1
        per_prio[prio][status] += 1
    if bad:
        return 1
    total = sum(derived.values())
    if total == 0:
        print("todo-count: no rows found", file=sys.stderr)
        return 1

    block = [
        f"{total} items: {derived['open']} open, {derived['partial']} partial, "
        f"{derived['blocked']} blocked, {derived['done']} done.",
        "",
        "Counted from the rows above by `scripts/todo-count.py` and asserted",
        "independently by `scripts/check-todo.py`, which is the gate. A number here",
        "that disagrees with the rows cannot reach a commit.",
        "",
        "| Priority | Open | Partial | Blocked | Done | Total |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for p in PRIORITIES:
        c = per_prio[p]
        block.append(f"| {p} | {c['open']} | {c['partial']} | {c['blocked']} | "
                     f"{c['done']} | {sum(c.values())} |")
    block.append(f"| **All** | **{derived['open']}** | **{derived['partial']}** | "
                 f"**{derived['blocked']}** | **{derived['done']}** | **{total}** |")

    if check_only:
        print("\n".join(block))
        return 0

    # -- splice --------------------------------------------------------------
    try:
        start = lines.index("## Counts")
    except ValueError:
        print("todo-count: TODO/INDEX.md has no `## Counts` heading", file=sys.stderr)
        return 1
    end = start + 1
    while end < len(lines) and not lines[end].startswith("## "):
        end += 1
    new = lines[:start + 1] + [""] + block + [""] + lines[end:]
    with open(INDEX, "w", encoding="utf-8") as fh:
        fh.write("\n".join(new).rstrip("\n") + "\n")
    print(f"todo-count: wrote {total} items: {derived['open']} open, "
          f"{derived['partial']} partial, {derived['blocked']} blocked, "
          f"{derived['done']} done")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
