#!/usr/bin/env python3
"""Check the repository scaffold and local Markdown link targets."""

from __future__ import annotations

import argparse
import html
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit


REQUIRED_DIRECTORIES = (
    "docs/contracts",
    "docs/decisions",
    "engine/cef",
    "core",
    "platform/macos",
    "platform/linux",
    "platform/windows",
    "ui",
    "plugins/runtime",
    "plugins/reference/account-routing",
    "presets/workspace",
    "presets/classic",
    "tools",
    "tests/contracts",
    "tests/browser",
    "tests/security",
    "tests/upgrade",
    "tests/fixtures",
)

REQUIRED_FILES = (
    ".gitignore",
    "README.md",
    "DESIGN_PHILOSOPHY.md",
    "IMPLEMENTATION_PLAN.md",
    "MODULES.md",
    "tools/check_structure.py",
    "tests/test_structure.py",
    *(f"{directory}/README.md" for directory in REQUIRED_DIRECTORIES),
)

INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(([^)\n]+)\)")
REFERENCE_LINK = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)")
FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
EXCLUDED_PARTS = {".git", ".hermes", "__pycache__", "build", "dist"}


def check_required_paths(root: Path) -> list[str]:
    errors = []
    for relative in REQUIRED_DIRECTORIES:
        if not (root / relative).is_dir():
            errors.append(f"missing required directory: {relative}")
    for relative in REQUIRED_FILES:
        if not (root / relative).is_file():
            errors.append(f"missing required file: {relative}")
    return errors


def markdown_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*.md")
        if not EXCLUDED_PARTS.intersection(path.relative_to(root).parts)
    )


def _link_target(raw_target: str) -> str | None:
    target = raw_target.strip()
    if target.startswith("<"):
        closing = target.find(">")
        if closing == -1:
            return None
        target = target[1:closing]
    else:
        target = target.split(maxsplit=1)[0]

    target = html.unescape(target)
    parsed = urlsplit(target)
    if parsed.scheme or target.startswith("//"):
        return None
    path = unquote(parsed.path)
    return path or None


def _line_links(line: str) -> list[str]:
    links = [match.group(1) for match in INLINE_LINK.finditer(line)]
    reference = REFERENCE_LINK.match(line)
    if reference:
        links.append(reference.group(1))
    return links


def check_markdown_links(root: Path) -> list[str]:
    root = root.resolve()
    errors = []
    for markdown in markdown_files(root):
        in_fence = False
        fence_marker = ""
        for line_number, line in enumerate(
            markdown.read_text(encoding="utf-8").splitlines(), start=1
        ):
            fence = FENCE.match(line)
            if fence:
                marker = fence.group(1)[0]
                if not in_fence:
                    in_fence = True
                    fence_marker = marker
                elif marker == fence_marker:
                    in_fence = False
                continue
            if in_fence:
                continue

            for raw_target in _line_links(line):
                target = _link_target(raw_target)
                if target is None:
                    continue
                if target.startswith("/"):
                    candidate = root / target.lstrip("/")
                else:
                    candidate = markdown.parent / target
                candidate = candidate.resolve()
                try:
                    candidate.relative_to(root)
                except ValueError:
                    errors.append(
                        f"{markdown.relative_to(root)}:{line_number}: "
                        f"local link escapes repository: {raw_target}"
                    )
                    continue
                if not candidate.exists():
                    errors.append(
                        f"{markdown.relative_to(root)}:{line_number}: "
                        f"missing local link target: {raw_target}"
                    )
    return errors


def check_structure(root: Path) -> list[str]:
    return check_required_paths(root) + check_markdown_links(root)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check required scaffold paths and local Markdown links."
    )
    parser.add_argument(
        "root",
        nargs="?",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (defaults to this script's parent repository)",
    )
    args = parser.parse_args()
    errors = check_structure(args.root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(
        f"Structure check passed: {len(REQUIRED_DIRECTORIES)} directories, "
        f"{len(REQUIRED_FILES)} files, "
        f"{len(markdown_files(args.root.resolve()))} Markdown files."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
