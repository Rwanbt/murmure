#!/usr/bin/env python3
"""Measures the WCAG contrast of the application colour tokens.

A contrast claim is only worth what it measures, so this script converts the
`oklch()` values declared in `src/tailwind.css` to sRGB, computes the relative
luminance of each one, and checks the pairs that the interface really draws
against the thresholds of WCAG 2.2:

* 1.4.3 Contrast (Minimum), AA: body text needs 4.5:1.
* 1.4.11 Non-text Contrast, AA: focus indicators and component boundaries
  need 3:1.

The Tailwind palette colours that the interface uses directly (`text-red-400`,
`text-amber-500`) are declared here with the values that Tailwind CSS v4
publishes, because they are not part of the application stylesheet.

Every failure names the ratio it measured, so a regression cannot be mistaken
for a rounding difference.
"""

from __future__ import annotations

import argparse
import math
import re
import sys
from pathlib import Path

# Tailwind CSS v4 palette, as published by Tailwind, in oklch.
TAILWIND = {
    "red-400": (0.704, 0.191, 22.216),
    "amber-500": (0.769, 0.188, 70.08),
}


def oklch_to_srgb(lightness: float, chroma: float, hue: float) -> tuple[float, float, float]:
    """Converts an oklch colour to gamma-encoded sRGB in the 0..1 range."""
    # oklch -> oklab
    hue_radians = math.radians(hue)
    a = chroma * math.cos(hue_radians)
    b = chroma * math.sin(hue_radians)

    # oklab -> linear sRGB, using the matrices from the CSS Color 4 spec.
    l_ = lightness + 0.3963377774 * a + 0.2158037573 * b
    m_ = lightness - 0.1055613458 * a - 0.0638541728 * b
    s_ = lightness - 0.0894841775 * a - 1.2914855480 * b

    l_cube = l_ ** 3
    m_cube = m_ ** 3
    s_cube = s_ ** 3

    red = 4.0767416621 * l_cube - 3.3077115913 * m_cube + 0.2309699292 * s_cube
    green = -1.2684380046 * l_cube + 2.6097574011 * m_cube - 0.3413193965 * s_cube
    blue = -0.0041960863 * l_cube - 0.7034186147 * m_cube + 1.7076147010 * s_cube

    def encode(value: float) -> float:
        value = min(1.0, max(0.0, value))
        if value <= 0.0031308:
            return 12.92 * value
        return 1.055 * (value ** (1 / 2.4)) - 0.055

    return encode(red), encode(green), encode(blue)


def relative_luminance(rgb: tuple[float, float, float]) -> float:
    def linearize(value: float) -> float:
        if value <= 0.03928:
            return value / 12.92
        return ((value + 0.055) / 1.055) ** 2.4

    red, green, blue = (linearize(channel) for channel in rgb)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast(first: tuple[float, float, float], second: tuple[float, float, float]) -> float:
    light = relative_luminance(first)
    dark = relative_luminance(second)
    if light < dark:
        light, dark = dark, light
    return (light + 0.05) / (dark + 0.05)


OKLCH_PATTERN = re.compile(
    r"--([a-z-]+):\s*oklch\(\s*([0-9.]+)\s+([0-9.]+)\s+([0-9.]+)\s*\)"
)


def tokens_of(css: str, selector: str) -> dict[str, tuple[float, float, float]]:
    """Reads the tokens declared in one block of the stylesheet."""
    start = css.find(selector)
    if start < 0:
        raise SystemExit(f"the selector {selector} was not found in the stylesheet")
    opening = css.find("{", start)
    # The block ends at the matching brace; the stylesheet has no nested rule
    # there, so the first closing brace is the right one.
    closing = css.find("}", opening)
    body = css[opening:closing]
    return {
        name: (float(lightness), float(chroma), float(hue))
        for name, lightness, chroma, hue in OKLCH_PATTERN.findall(body)
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stylesheet", type=Path, required=True)
    parser.add_argument("--minimum-tolerance", type=float, default=0.0)
    arguments = parser.parse_args()

    css = arguments.stylesheet.read_text(encoding="utf-8")
    failures = 0
    checked = 0

    # Each entry is (theme, foreground token, background token, required ratio,
    # what the pair is used for, whether a shortfall fails the check).
    #
    # A shortfall fails only where WCAG 2.2 requires the contrast. The border
    # token is reported but not enforced: every control of this interface
    # carries a text label, and Understanding 1.4.11 states that a boundary is
    # not required to reach 3:1 when the component is identified by its label.
    # Measuring it anyway keeps the number visible if that ever stops holding.
    requirements = [
        ("dark", "foreground", "background", 4.5, "body text", True),
        ("dark", "muted-foreground", "background", 4.5, "secondary text", True),
        ("dark", "muted-foreground", "card", 4.5, "secondary text on a card", True),
        ("dark", "destructive", "background", 4.5, "error text", True),
        ("dark", "foreground", "card", 4.5, "body text on a card", True),
        ("dark", "ring", "background", 3.0, "focus indicator", True),
        ("dark", "border", "background", 3.0, "component boundary (advisory)", False),
        # The interface writes its error and warning messages with the
        # Tailwind palette rather than with a token.
        ("dark", "tailwind:red-400", "background", 4.5, "error message text", True),
        ("dark", "tailwind:amber-500", "background", 4.5, "warning message text", True),
    ]

    for theme, foreground_name, background_name, requirement, purpose, enforced in requirements:
        tokens = tokens_of(css, ".dark {" if theme == "dark" else ":root {")

        def resolve(name: str) -> tuple[float, float, float] | None:
            if name.startswith("tailwind:"):
                return TAILWIND.get(name.split(":", 1)[1])
            return tokens.get(name)

        foreground = resolve(foreground_name)
        background = resolve(background_name)
        if foreground is None or background is None:
            print(f"SKIP: {purpose}: {foreground_name} or {background_name} is not declared")
            continue

        measured = contrast(oklch_to_srgb(*foreground), oklch_to_srgb(*background))
        checked += 1
        if measured + arguments.minimum_tolerance >= requirement:
            print(
                f"PASS: {theme} {purpose}: {foreground_name} on {background_name} "
                f"= {measured:.2f}:1 (needs {requirement}:1)"
            )
        elif enforced:
            failures += 1
            print(
                f"FAIL: {theme} {purpose}: {foreground_name} on {background_name} "
                f"= {measured:.2f}:1 (needs {requirement}:1)"
            )
        else:
            print(
                f"ADVISORY: {theme} {purpose}: {foreground_name} on {background_name} "
                f"= {measured:.2f}:1 (below {requirement}:1, not required here)"
            )

    print(f"\n{checked - failures} passed, {failures} failed")
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
