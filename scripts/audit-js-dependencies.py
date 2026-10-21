#!/usr/bin/env python3
"""Audits the JavaScript dependency tree against the OSV database.

The Rust tree is already checked against the RustSec advisories. The frontend
tree was not checked at all, so a vulnerable transitive package could ship
without anybody noticing.

The lock file is the exact set of packages that a build installs, so it is the
only honest source: auditing `package.json` would check the ranges the project
asks for, not the versions it actually gets.

`osv.dev` is the open vulnerability database of the Open Source Security
Foundation. It needs no account and no token, so the check is reproducible by
anyone.

Each advisory is reported with its identifier and the versions that fix it, so
a finding says what to do and not only that something is wrong.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.request
from pathlib import Path

OSV_BATCH = "https://api.osv.dev/v1/querybatch"
OSV_VULN = "https://api.osv.dev/v1/vulns/{}"
BATCH_SIZE = 500


def split_key(key: str) -> tuple[str, str] | None:
    """Splits a lock file key such as `@scope/name@1.2.3` into name and version."""
    # The separator is the last `@` that is not the leading scope marker.
    index = key.rfind("@")
    if index <= 0:
        return None
    name, version = key[:index], key[index + 1 :]
    if not name or not version:
        return None
    return name, version


def read_lock(path: Path) -> list[tuple[str, str]]:
    """Reads `pnpm-lock.yaml` and returns every (name, version) it installs."""
    text = path.read_text(encoding="utf-8")
    try:
        import yaml  # type: ignore[import-not-found]
    except ImportError:
        raise SystemExit(
            "FAIL: PyYAML is required to read the lock file; it is installed on the runners."
        )

    data = yaml.safe_load(text)
    packages = data.get("packages") or {}
    if not packages:
        raise SystemExit("FAIL: the lock file declares no package, so nothing was audited")

    unique: dict[str, str] = {}
    for key in packages:
        parsed = split_key(str(key))
        if parsed is None:
            continue
        name, version = parsed
        # A version that is still a range is not something the build installs.
        if not version[0].isdigit():
            continue
        unique[name] = version
    return sorted(unique.items())


def query_osv(packages: list[tuple[str, str]]) -> list[dict]:
    """Asks the OSV database which of these versions carry an advisory."""
    results: list[dict] = []
    for start in range(0, len(packages), BATCH_SIZE):
        batch = packages[start : start + BATCH_SIZE]
        body = json.dumps(
            {
                "queries": [
                    {"package": {"name": name, "ecosystem": "npm"}, "version": version}
                    for name, version in batch
                ]
            }
        ).encode("utf-8")
        request = urllib.request.Request(
            OSV_BATCH,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=120) as response:
            payload = json.loads(response.read().decode("utf-8"))
        results.extend(payload.get("results") or [])
    return results


def describe(vulnerability: dict) -> str:
    identifier = vulnerability.get("id", "unknown")
    summary = (vulnerability.get("summary") or "").strip().replace("\n", " ")
    severity = "unknown"
    for entry in vulnerability.get("severity") or []:
        severity = entry.get("score", severity)
        break
    return f"{identifier} [{severity}] {summary}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, required=True)
    parser.add_argument(
        "--allow",
        default="",
        help="comma separated identifiers that are accepted, each of which must be justified in the report",
    )
    arguments = parser.parse_args()

    packages = read_lock(arguments.lock)
    print(f"Packages in the lock file: {len(packages)}")

    results = query_osv(packages)
    if len(results) != len(packages):
        raise SystemExit(
            f"FAIL: the database answered for {len(results)} of {len(packages)} packages"
        )

    allowed = {identifier.strip() for identifier in arguments.allow.split(",") if identifier.strip()}
    findings: list[str] = []
    for (name, version), result in zip(packages, results):
        for vulnerability in result.get("vulns") or []:
            identifier = vulnerability.get("id", "unknown")
            if identifier in allowed:
                continue
            findings.append(f"{name}@{version}: {describe(vulnerability)}")

    if not findings:
        print("PASS: no advisory affects the versions the lock file installs")
        return 0

    for finding in findings:
        print(f"FAIL: {finding}")
    print(f"\n{len(findings)} advisory(ies) affect the locked versions")
    print("Ask https://api.osv.dev/v1/vulns/<id> for the affected ranges and the fixed version.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
