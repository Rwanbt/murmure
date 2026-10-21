#!/usr/bin/env python3
"""Verifies that every ignored test is also registered as a test.

A function that carries `#[ignore]` but not `#[test]` is not a test: the
harness never registers it, `cargo test` reports `running 0 tests`, and the
job that was supposed to prove something stays green while proving nothing.
That failure mode is silent, which is why it is checked mechanically rather
than trusted to a review.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ATTRIBUTE_BLOCK = re.compile(
    r"((?:^[ \t]*#\[[^\]]*\][ \t]*\r?\n)+)[ \t]*(?:pub\s+)?fn\s+([a-zA-Z0-9_]+)",
    re.MULTILINE,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    arguments = parser.parse_args()

    files = [
        path
        for path in sorted(arguments.root.rglob("*.rs"))
        if "target" not in path.parts
    ]
    if not files:
        print(f"FAIL: no Rust source under {arguments.root}", file=sys.stderr)
        return 1

    ignored = 0
    findings: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        for match in ATTRIBUTE_BLOCK.finditer(text):
            attributes, name = match.group(1), match.group(2)
            has_ignore = "#[ignore" in attributes
            has_test = "#[test]" in attributes or "#[tokio::test" in attributes
            if not has_ignore:
                continue
            ignored += 1
            if not has_test:
                line = text.count("\n", 0, match.start()) + 1
                findings.append(
                    f"{path}:{line}: `{name}` is ignored but is not a test, so it never runs"
                )

    print(f"Ignored tests found: {ignored}")
    if not findings:
        print("PASS: every ignored function is registered as a test")
        return 0
    for finding in findings:
        print(f"FAIL: {finding}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
