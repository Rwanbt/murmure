#!/usr/bin/env python3
"""Refuse a log statement that carries the user's own words.

The privacy rule of this project is that the application never writes a
recording, a transcription or the text given to the speech synthesis into a
log. A log file is written to disk, travels into bug reports and outlives the
audio it came from, so a single `debug!` that interpolates the wrong variable
undoes the property silently.

The check is deliberately narrow: it looks at the arguments of the logging
macros and refuses a bare identifier whose name says it holds user content.
`transcription.chars().count()` is fine, because the count is not the text.

Usage: check-private-logs.py --root src-tauri/src
"""

import argparse
import pathlib
import re
import sys

LOGGING = ("debug", "info", "warn", "error", "trace")

# A statement like `debug!("...", transcription)` leaks the text.
PATTERN = re.compile(
    r"\b(?:" + "|".join(LOGGING) + r")!\s*\((.*)\)\s*;?\s*$",
    re.DOTALL,
)

# Names that hold what the user said, typed or selected.
SENSITIVE = {
    "text",
    "transcription",
    "transcript",
    "prompt",
    "selection",
    "selected",
    "content",
    "speech",
    "dictation",
    "words",
}

# An argument that carries the value, not a property of it.
BARE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)(?:\.clone\(\)|\.to_string\(\))?$")


def arguments_of(body: str) -> list[str]:
    """Split a macro body on the commas that are not inside a literal."""
    arguments = []
    current = []
    in_string = False
    escaped = False
    depth = 0
    for character in body:
        if in_string:
            current.append(character)
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
            current.append(character)
        elif character in "([{":
            depth += 1
            current.append(character)
        elif character in ")]}":
            depth -= 1
            current.append(character)
        elif character == "," and depth == 0:
            arguments.append("".join(current))
            current = []
        else:
            current.append(character)
    if current:
        arguments.append("".join(current))
    return arguments


def findings(path: pathlib.Path) -> list[tuple[int, str]]:
    problems = []
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    for number, line in enumerate(lines, 1):
        stripped = line.strip()
        if stripped.startswith("//"):
            continue
        match = PATTERN.search(stripped)
        if not match:
            continue
        body = match.group(1)
        # The first argument is the format string; only the rest can leak.
        for argument in arguments_of(body)[1:]:
            name = BARE.match(argument.strip())
            if name and name.group(1) in SENSITIVE:
                problems.append((number, argument.strip()))
    return problems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    options = parser.parse_args()

    root = pathlib.Path(options.root)
    if not root.is_dir():
        print(f"FAIL: {root} is not a directory", file=sys.stderr)
        return 2

    failures = 0
    checked = 0
    for path in sorted(root.rglob("*.rs")):
        if not path.is_file():
            continue
        checked += 1
        for number, argument in findings(path):
            failures += 1
            print(f"{path}:{number}: a log statement interpolates `{argument}`")

    if failures > 0:
        print(
            f"FAIL: {failures} log statement(s) would write user content. "
            "Log a length or a boolean instead.",
            file=sys.stderr,
        )
        return 1

    print(f"PASS: {checked} Rust files, no log statement carries user content")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
