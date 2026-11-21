#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Guard against documentation drift (run at the end of every session).

Checks:
  - START.md exists, is short, has a "Stand: YYYY-MM-DD" line and the sections
    "Stand" / "Als Nächstes", and carries no history (no checked-off boxes)
  - PLAN.md keeps no list of next steps (START.md is the single source)
  - CLAUDE.md tells sessions to read and update START.md
  - relative Markdown links in the top-level and docs/ files resolve
  - START.md is not behind the work: no commits or uncommitted changes to other
    files since START.md was last changed

Usage:
    python3 -I tools/check_docs.py        # exit code 0 = no drift
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_START_LINES = 40
errors: list[str] = []


def err(msg: str) -> None:
    errors.append(msg)


def git(*args: str) -> str:
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True).stdout


def check_start() -> None:
    p = ROOT / "START.md"
    if not p.exists():
        err("START.md missing")
        return
    text = p.read_text(encoding="utf-8")
    lines = text.splitlines()
    if len(lines) > MAX_START_LINES:
        err(f"START.md has {len(lines)} lines (max {MAX_START_LINES}): trim it, no history")
    if not re.search(r"^Stand: \d{4}-\d{2}-\d{2}", text, re.M):
        err('START.md lacks a "Stand: YYYY-MM-DD" line')
    for h in ("## Stand", "## Als Nächstes"):
        if h not in text:
            err(f'START.md lacks section "{h}"')
    if re.search(r"- \[[xX]\]", text):
        err("START.md contains checked-off boxes: remove finished items instead")


def check_plan_and_claude() -> None:
    plan = (ROOT / "PLAN.md").read_text(encoding="utf-8")
    sec = re.search(r"^## 7\..*?(?=^## |\Z)", plan, re.M | re.S)
    if sec and re.search(r"^\s*(\d+\.|-)\s", sec.group(0), re.M):
        err("PLAN.md §7 contains a list: next steps belong only in START.md")
    claude = (ROOT / "CLAUDE.md").read_text(encoding="utf-8")
    if "START.md" not in claude:
        err("CLAUDE.md does not mention START.md")


def check_links() -> None:
    files = [ROOT / n for n in ("START.md", "PLAN.md", "CLAUDE.md", "README.md")] + sorted((ROOT / "docs").rglob("*.md"))
    for f in files:
        if not f.exists():
            continue
        for m in re.finditer(r"\]\(([^)\s]+)\)", f.read_text(encoding="utf-8")):
            t = m.group(1)
            if re.match(r"[a-z]+:", t) or t.startswith("#"):
                continue
            path = re.sub(r"[#:].*$", "", t)
            if path and not (f.parent / path).exists():
                err(f"{f.relative_to(ROOT)}: broken link {t}")


def check_staleness() -> None:
    last = git("log", "-1", "--format=%H", "--", "START.md").strip()
    if not last:
        return  # not committed yet
    behind = git("rev-list", "--count", f"{last}..HEAD").strip()
    if behind not in ("", "0"):
        err(f"START.md is {behind} commit(s) behind HEAD: update it")
    changed = [l[3:] for l in git("status", "--porcelain").splitlines()]
    other = [c for c in changed if c != "START.md"]
    if other and "START.md" not in changed:
        err("working tree changed but START.md was not updated: " + ", ".join(other[:5]))


def main() -> int:
    check_start()
    check_plan_and_claude()
    check_links()
    check_staleness()
    for e in errors:
        print("DRIFT", e)
    if not errors:
        print("OK   docs consistent")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
