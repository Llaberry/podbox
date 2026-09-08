#!/usr/bin/env python3
"""check-todo.py - the reader half of docs/methodology/work-todo.md's two scripts.

⛔ THIS IS THE GATE. It asserts, independently of the writer, that:

  1. every row in TODO/INDEX.md names an entry that exists, in the file the row
     links to, under a heading `### T-NNNN <title>`;
  2. every entry has a row;
  3. the status in the row and the `Status:` line in the entry agree;
  4. every count in INDEX.md's Counts block is what the rows actually say;
  5. every entry carries all ten fields authoring.md requires, and its `Prove`
     is a command;
  6. every reference named in TODO/reference-map.md resolves to a directory
     under references/ that has a PROVENANCE.md;
  7. every cited `path:line` in TODO/ resolves: the file exists and has at
     least that many lines;
  8. every relative markdown link from a TODO/ file resolves;
  9. every `T-NNNN` mentioned anywhere in TODO/ names an entry that exists;
 10. PROGRESS.md's count line is what the rows say. A file that quotes a number
     another file measures has to be checkable, so it is written as a fixed line
     this script parses rather than as prose that reads better.

⛔ Read the exit code from this process, unpiped.
Exit: 0 everything agrees, 1 something disagrees, 2 could not run.

Python rather than shell because the checks are string and arithmetic work, and
check 4 is the arithmetic this script exists to stop being done by hand.
docs/methodology/work-todo.md names a pair of scripts and not a language.
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TODO = os.path.join(ROOT, "TODO")
REFS = os.path.join(ROOT, "references")

FIELDS = [
    "Source", "Category", "Priority", "Effort", "Status",
    "Problem", "Premise", "Approach", "Decision", "Prove",
]
STATUSES = ["open", "partial", "blocked", "done"]
PRIORITIES = ["P0", "P1", "P2", "P3"]

# A citation this script resolves. Anchored at a top-level directory of the
# repository so that prose mentioning `foo.c:12` in the abstract is not read as
# a path. The trailing `[).,;]?` is stripped by the capture group ending.
CITE = re.compile(
    r"(?<![\w/.-])((?:references|crates|experiments|scripts|docs|TODO)"
    r"/[A-Za-z0-9._+@/-]+?):(\d+)(?:-(\d+))?(?![\w.-])"
)
ROW = re.compile(
    r"^\|\s*\[(T-\d{4})\]\(([^)]+)\)\s*\|\s*(P\d)\s*\|\s*([a-z-]+)\s*\|"
    r"\s*\*{0,2}([a-z]+)\*{0,2}\s*\|\s*(.+?)\s*\|\s*$"
)
HEADING = re.compile(r"^### (T-\d{4}) +(\S.*)$")
LINK = re.compile(r"\[[^\]]*\]\(([^)#][^)]*?)(?:#[^)]*)?\)")

errors = []
warnings = []


def err(where, msg):
    errors.append(f"{where}: {msg}")


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def main():
    if not os.path.isdir(TODO):
        print("check-todo: TODO/ does not exist", file=sys.stderr)
        return 2
    index_path = os.path.join(TODO, "INDEX.md")
    if not os.path.isfile(index_path):
        print("check-todo: TODO/INDEX.md does not exist", file=sys.stderr)
        return 2

    index = read(index_path)

    # -- 1. the rows ---------------------------------------------------------
    rows = {}
    for n, line in enumerate(index.splitlines(), 1):
        if not line.startswith("| [T-"):
            continue
        m = ROW.match(line)
        if not m:
            err(f"TODO/INDEX.md:{n}", f"row does not parse: {line.strip()}")
            continue
        tid, target, prio, cat, status, title = m.groups()
        if tid in rows:
            err(f"TODO/INDEX.md:{n}", f"{tid} appears twice")
        if status not in STATUSES:
            err(f"TODO/INDEX.md:{n}", f"{tid} status {status!r} is not one of {STATUSES}")
        if prio not in PRIORITIES:
            err(f"TODO/INDEX.md:{n}", f"{tid} priority {prio!r} is not one of {PRIORITIES}")
        rows[tid] = dict(target=target, prio=prio, cat=cat, status=status,
                         title=title, line=n)

    if not rows:
        err("TODO/INDEX.md", "no entry rows found")

    # -- 2. the entries ------------------------------------------------------
    entries = {}
    for name in sorted(os.listdir(TODO)):
        if not name.endswith(".md") or name in ("INDEX.md", "PROGRESS.md",
                                                "RULES.md", "reference-map.md"):
            continue
        path = os.path.join(TODO, name)
        text = read(path)
        lines = text.splitlines()
        heads = [(i, HEADING.match(ln)) for i, ln in enumerate(lines)]
        heads = [(i, m.group(1), m.group(2)) for i, m in heads if m]
        for idx, (i, tid, title) in enumerate(heads):
            end = heads[idx + 1][0] if idx + 1 < len(heads) else len(lines)
            body = "\n".join(lines[i:end])
            if tid in entries:
                err(f"TODO/{name}:{i + 1}", f"{tid} is defined twice")
            entries[tid] = dict(file=name, line=i + 1, title=title, body=body)

    # -- cross-check ---------------------------------------------------------
    for tid, row in sorted(rows.items()):
        e = entries.get(tid)
        if e is None:
            err(f"TODO/INDEX.md:{row['line']}", f"{tid} has a row and no entry")
            continue
        if e["file"] != row["target"]:
            err(f"TODO/INDEX.md:{row['line']}",
                f"{tid} row links to {row['target']} and the entry is in {e['file']}")
        if e["title"] != row["title"]:
            err(f"TODO/INDEX.md:{row['line']}",
                f"{tid} row title {row['title']!r} != entry title {e['title']!r}")
    for tid, e in sorted(entries.items()):
        if tid not in rows:
            err(f"TODO/{e['file']}:{e['line']}", f"{tid} has an entry and no row")

    # -- 3, 5. the fields ----------------------------------------------------
    for tid, e in sorted(entries.items()):
        where = f"TODO/{e['file']}:{e['line']}"
        for field in FIELDS:
            m = re.search(rf"^{field}: +(\S.*)$", e["body"], re.M)
            if not m:
                err(where, f"{tid} has no `{field}:` field")
                continue
            value = m.group(1).strip()
            if field == "Status":
                status = value.strip("*").split()[0]
                if status not in STATUSES:
                    err(where, f"{tid} Status {status!r} is not one of {STATUSES}")
                elif tid in rows and status != rows[tid]["status"]:
                    err(where,
                        f"{tid} entry status {status!r} disagrees with the index "
                        f"row's {rows[tid]['status']!r}")
            if field == "Category" and tid in rows and value != rows[tid]["cat"]:
                err(where, f"{tid} Category {value!r} disagrees with the row's "
                           f"{rows[tid]['cat']!r}")
            if field == "Priority" and tid in rows and value != rows[tid]["prio"]:
                err(where, f"{tid} Priority {value!r} disagrees with the row's "
                           f"{rows[tid]['prio']!r}")
        # `Prove` has to be a command, per authoring.md section 6.
        m = re.search(r"^Prove: +(.*)$", e["body"], re.M)
        if m and not re.search(r"```|`[^`]+`|^\s{2,}\S", m.group(1)):
            err(where, f"{tid} Prove is prose, not a command: {m.group(1)[:60]!r}")

    # -- 4. the counts -------------------------------------------------------
    derived = {s: 0 for s in STATUSES}
    per_prio = {p: {s: 0 for s in STATUSES} for p in PRIORITIES}
    for row in rows.values():
        derived[row["status"]] += 1
        per_prio[row["prio"]][row["status"]] += 1
    total = len(rows)

    want = (f"{total} items: {derived['open']} open, {derived['partial']} partial, "
            f"{derived['blocked']} blocked, {derived['done']} done.")
    if want not in index:
        err("TODO/INDEX.md", f"the totals line is not the rows. Expected exactly:\n    {want}")

    for p in PRIORITIES:
        c = per_prio[p]
        t = sum(c.values())
        want_row = (f"| {p} | {c['open']} | {c['partial']} | {c['blocked']} | "
                    f"{c['done']} | {t} |")
        if want_row not in index:
            err("TODO/INDEX.md", f"the {p} count row is not the rows. Expected exactly:\n    {want_row}")
    want_all = (f"| **All** | **{derived['open']}** | **{derived['partial']}** | "
                f"**{derived['blocked']}** | **{derived['done']}** | **{total}** |")
    if want_all not in index:
        err("TODO/INDEX.md", f"the All count row is not the rows. Expected exactly:\n    {want_all}")

    # -- 6. the corpus -------------------------------------------------------
    map_path = os.path.join(TODO, "reference-map.md")
    if not os.path.isfile(map_path):
        err("TODO/reference-map.md", "does not exist")
    else:
        named = set(re.findall(r"`references/([A-Za-z0-9._+-]+)`", read(map_path)))
        if not named:
            err("TODO/reference-map.md", "names no reference under references/")
        for tree in sorted(named):
            d = os.path.join(REFS, tree)
            if not os.path.isdir(d):
                err("TODO/reference-map.md", f"references/{tree} does not exist")
            elif not os.path.isfile(os.path.join(d, "PROVENANCE.md")):
                err("TODO/reference-map.md", f"references/{tree} has no PROVENANCE.md")
        on_disk = {d for d in os.listdir(REFS)
                   if os.path.isdir(os.path.join(REFS, d))} if os.path.isdir(REFS) else set()
        for tree in sorted(on_disk - named):
            err("TODO/reference-map.md",
                f"references/{tree} is on disk and the map does not name it")

    # -- 7, 8. citations and links ------------------------------------------
    for name in sorted(os.listdir(TODO)):
        if not name.endswith(".md"):
            continue
        path = os.path.join(TODO, name)
        for n, line in enumerate(read(path).splitlines(), 1):
            for m in CITE.finditer(line):
                rel, start, end = m.group(1), int(m.group(2)), m.group(3)
                target = os.path.join(ROOT, rel)
                where = f"TODO/{name}:{n}"
                if not os.path.isfile(target):
                    err(where, f"cites {rel}:{start}, and that file does not exist")
                    continue
                with open(target, "rb") as fh:
                    count = sum(1 for _ in fh)
                last = int(end) if end else start
                if last > count:
                    err(where, f"cites {rel}:{m.group(0).split(':', 1)[1]}, "
                               f"and that file has {count} lines")
            for m in LINK.finditer(line):
                href = m.group(1).strip()
                if href.startswith(("http://", "https://", "mailto:")):
                    continue
                target = os.path.normpath(os.path.join(TODO, href.split(":")[0]))
                if not os.path.exists(target):
                    err(f"TODO/{name}:{n}", f"link target {href} does not resolve")

    # -- 9. cross-references ------------------------------------------------
    for name in sorted(os.listdir(TODO)):
        if not name.endswith(".md"):
            continue
        for n, line in enumerate(read(os.path.join(TODO, name)).splitlines(), 1):
            if line.startswith("| [T-") or line.startswith("### T-"):
                continue
            for tid in set(re.findall(r"\bT-\d{4}\b", line)):
                if tid not in entries:
                    err(f"TODO/{name}:{n}", f"names {tid}, which is not an entry")

    # -- 10. the record's count line -----------------------------------------
    prog_path = os.path.join(TODO, "PROGRESS.md")
    if not os.path.isfile(prog_path):
        err("TODO/PROGRESS.md", "does not exist")
    else:
        want_prog = (f"{total} entries: {derived['open']} open, "
                     f"{derived['partial']} partial, {derived['blocked']} blocked, "
                     f"{derived['done']} done.")
        if want_prog not in read(prog_path):
            err("TODO/PROGRESS.md",
                f"the count line is not the rows. Expected exactly:\n    {want_prog}")

    # -- report --------------------------------------------------------------
    print(f"check-todo: {len(rows)} rows, {len(entries)} entries, "
          f"{derived['open']} open, {derived['partial']} partial, "
          f"{derived['blocked']} blocked, {derived['done']} done")
    for w in warnings:
        print(f"  warn {w}")
    if errors:
        print(f"check-todo: {len(errors)} problem(s)", file=sys.stderr)
        for e in errors:
            print(f"  {e}", file=sys.stderr)
        return 1
    print("check-todo: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
