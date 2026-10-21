#!/usr/bin/env python3
"""Static accessibility audit of the React screens.

Reading a screen by eye does not scale and cannot be re-run, so this script
checks the patterns that WCAG 2.2 makes mandatory and that a compiler does not
catch:

* 2.1.1 Keyboard: a clickable `div`, `span`, `li` or `td` that is not a
  button must expose a role and a keyboard handler, otherwise it is reachable
  by mouse only.
* 4.1.2 Name, Role, Value: an input, select or textarea must be named, either
  by `aria-label`, by `aria-labelledby`, or by a `label` whose `htmlFor`
  matches its `id`.
* 1.1.1 Non-text Content: an `img` must carry an `alt` attribute, empty when
  the image is decorative.
* 2.4.3 Focus Order: a positive `tabIndex` reorders the focus sequence away
  from the reading order.

The audit is deliberately conservative: it reports only what it can decide
from the source, and it never guesses. A file it cannot parse is reported
instead of being skipped silently.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Only the screens of this fork are audited. The UI primitives that come from
# the design system follow the same rules and are covered once they are
# rendered; auditing them statically here would mostly report their props.
SKIPPED_DIRECTORIES = {"smartmic", "ui"}

INTERACTIVE_WITHOUT_BUTTON = {"div", "span", "li", "td", "tr", "p", "h1", "h2", "h3"}

TAG_PATTERN = re.compile(r"<([a-zA-Z][a-zA-Z0-9]*)((?:\s[^<>]*?)?)>", re.DOTALL)
HTML_FOR_PATTERN = re.compile(r"htmlFor=\{?['\"]([^'\"]+)['\"]")
ID_PATTERN = re.compile(r"\bid=\{?['\"]([^'\"]+)['\"]")
ARIA_NAME_PATTERN = re.compile(r"\baria-label(?:ledby)?=\{?['\"`]")


def attributes_of(raw: str) -> str:
    return raw


def enclosing_label_depth(text: str, position: int) -> int:
    """How many `<label>` elements enclose the given position."""
    return len(re.findall(r"<label\b", text[:position])) - len(
        re.findall(r"</label>", text[:position])
    )


def audit(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    findings: list[str] = []
    labelled_targets = set(HTML_FOR_PATTERN.findall(text))

    for match in TAG_PATTERN.finditer(text):
        tag = match.group(1)
        attributes = match.group(2) or ""
        line = text.count("\n", 0, match.start()) + 1

        if tag in INTERACTIVE_WITHOUT_BUTTON and "onClick" in attributes:
            has_role = "role=" in attributes
            has_keyboard = "onKeyDown" in attributes or "onKeyUp" in attributes
            if not (has_role and has_keyboard):
                findings.append(
                    f"{path}:{line}: a clickable <{tag}> has no role and no keyboard handler (2.1.1)"
                )

        if tag in {"input", "select", "textarea"}:
            # An element that is hidden from assistive technology needs no
            # name: it is not announced at all.
            hidden_from_assistive_technology = 'aria-hidden="true"' in attributes
            # An input nested in a label is named by it, which is valid HTML.
            implicitly_labelled = enclosing_label_depth(text, match.start()) > 0
            if (
                not hidden_from_assistive_technology
                and not ARIA_NAME_PATTERN.search(attributes)
                and not implicitly_labelled
            ):
                identifier = ID_PATTERN.search(attributes)
                named_by_label = identifier is not None and identifier.group(1) in labelled_targets
                if not named_by_label:
                    findings.append(
                        f"{path}:{line}: a <{tag}> has no accessible name (4.1.2)"
                    )

            # A field that the interface hides with a class is still in the
            # accessibility tree and still focusable unless it says otherwise,
            # so keyboard users land on something invisible.
            hidden = "className=" in attributes and re.search(
                r"className=\{?[`\"'][^`\"']*\bhidden\b", attributes
            )
            if hidden and "tabIndex={-1}" not in attributes:
                findings.append(
                    f"{path}:{line}: a hidden <{tag}> stays in the tab order (2.4.3)"
                )

        if tag == "img" and "alt=" not in attributes:
            findings.append(f"{path}:{line}: an <img> has no alt attribute (1.1.1)")

        tabindex = re.search(r"tabIndex=\{?['\"]?(\d+)", attributes)
        if tabindex and int(tabindex.group(1)) > 0:
            findings.append(
                f"{path}:{line}: a positive tabIndex reorders the focus sequence (2.4.3)"
            )

    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="print the findings without failing, for screens not yet reviewed",
    )
    arguments = parser.parse_args()

    files = [
        path
        for path in sorted(arguments.root.rglob("*.tsx"))
        if not SKIPPED_DIRECTORIES & set(path.parts)
        and not path.name.endswith(".test.tsx")
        and not path.name.endswith(".spec.tsx")
    ]
    if not files:
        print(f"FAIL: no screen was found under {arguments.root}", file=sys.stderr)
        return 1

    findings: list[str] = []
    for path in files:
        findings.extend(audit(path))

    print(f"Screens audited: {len(files)}")
    if not findings:
        print("PASS: no keyboard, naming or focus-order finding")
        return 0

    for finding in findings:
        print(("ADVISORY: " if arguments.report_only else "FAIL: ") + finding)
    print(f"\n{len(findings)} finding(s)")
    return 0 if arguments.report_only else 1


if __name__ == "__main__":
    sys.exit(main())
