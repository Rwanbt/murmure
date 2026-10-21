#!/usr/bin/env python3
"""Validate the GitHub Actions workflow definitions of this repository.

PyYAML accepts a duplicate mapping key silently and keeps the last one. A
duplicated key inside a step therefore produces a workflow that GitHub
refuses to load, and the pull request checks report a failure with no job
at all, which is very hard to diagnose. This validator rejects duplicate
keys explicitly and fails with the file and the offending key.
"""
from __future__ import annotations

import glob
import os
import sys

try:
    import yaml
except ImportError:  # pragma: no cover - the CI step installs PyYAML
    sys.stderr.write("PyYAML is required: python3 -m pip install pyyaml\n")
    raise SystemExit(2)


class StrictLoader(yaml.SafeLoader):
    """A loader that refuses duplicate mapping keys."""


def _construct_mapping(loader, node, deep=False):
    mapping = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if key in mapping:
            raise yaml.constructor.ConstructorError(
                "while constructing a mapping",
                node.start_mark,
                "found duplicate key %r" % (key,),
                key_node.start_mark,
            )
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


StrictLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, _construct_mapping
)


def main() -> int:
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    patterns = (
        os.path.join(root, ".github", "workflows", "*.yml"),
        os.path.join(root, ".github", "workflows", "*.yaml"),
    )
    paths: list[str] = []
    for pattern in patterns:
        paths.extend(sorted(glob.glob(pattern)))

    if not paths:
        sys.stderr.write("No workflow definition was found.\n")
        return 1

    failures = 0
    for path in paths:
        relative = os.path.relpath(path, root).replace(os.sep, "/")
        with open(path, encoding="utf-8") as handle:
            source = handle.read()
        try:
            document = yaml.load(source, Loader=StrictLoader)
        except yaml.YAMLError as error:
            print("FAIL %s: %s" % (relative, error))
            failures += 1
            continue
        if not isinstance(document, dict):
            print("FAIL %s: a workflow must be a mapping." % relative)
            failures += 1
            continue
        jobs = document.get("jobs")
        if not isinstance(jobs, dict) or not jobs:
            print("FAIL %s: the workflow declares no job." % relative)
            failures += 1
            continue
        print("OK   %s (%d job(s))" % (relative, len(jobs)))

    if failures:
        print("%d workflow definition(s) are invalid." % failures)
        return 1
    print("%d workflow definition(s) are valid." % len(paths))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
