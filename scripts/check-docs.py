#!/usr/bin/env python3
"""Compile every `rust` code block in the docs.

The docs are how an agent (or a person) learns this codebase, so a stale snippet
is a real defect: it teaches an API that no longer exists. Doctests only cover
`src/`, so this script covers the Markdown:

- a fence tagged `rust` **must** compile;
- a fence tagged `rust,ignore` is a deliberate fragment and is skipped (write
  one line above it saying what is missing);
- `text`, `toml`, `bash`, `console`, `cpp`, ... are skipped.

Usage:  python3 scripts/check-docs.py [--verbose]
Exit:   0 when every `rust` block compiles, 1 otherwise.

How it works: generate one binary per block in a scratch crate that path-depends
on the workspace members, then `cargo check` it. The scratch crate lives in a
temp dir and is deleted afterwards.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOURCES = ["README.md", "AGENTS.md", "release.md", "docs"]
# Crates a doc example may legitimately `use`. Everything backend-neutral plus
# the headless/recording pieces; window/GPU/browser crates are left out so the
# check stays fast.
DENY = ("igui_backend_wgpu", "igui_winit", "igui_backend_canvas", "igui_wasm")


def workspace_members():
    text = open(os.path.join(ROOT, "Cargo.toml")).read()
    members = re.search(r"members = \[(.*?)\]", text, re.S).group(1)
    names = re.findall(r'"([^"]+)"', members)
    out = []
    for rel in names:
        manifest = os.path.join(ROOT, rel, "Cargo.toml")
        if not os.path.exists(manifest):
            continue
        text = open(manifest).read()
        pkg = re.search(r'^name = "([^"]+)"', text, re.M)
        if not pkg or pkg.group(1) in DENY:
            continue
        # Only library targets can be depended on.
        lib_path = re.search(r'^\[lib\]\s*\n(?:.*\n)*?path = "([^"]+)"', text, re.M)
        has_lib = os.path.exists(os.path.join(ROOT, rel, lib_path.group(1))) if lib_path \
            else os.path.exists(os.path.join(ROOT, rel, "src", "lib.rs"))
        if not has_lib:
            continue
        out.append((pkg.group(1), os.path.join(ROOT, rel)))
    return out


def markdown_files():
    for src in SOURCES:
        path = os.path.join(ROOT, src)
        if os.path.isfile(path):
            yield path
        for dirpath, _, filenames in os.walk(path):
            for name in sorted(filenames):
                if name.endswith(".md"):
                    yield os.path.join(dirpath, name)


def rust_blocks(path):
    """Yields (line_number, body) for every fence tagged exactly `rust`."""
    lines = open(path).read().split("\n")
    i = 0
    while i < len(lines):
        stripped = lines[i].lstrip()
        if stripped.startswith("```"):
            info = stripped[3:].strip()
            start = i + 1
            j = start
            while j < len(lines) and not lines[j].lstrip().startswith("```"):
                j += 1
            if info == "rust":
                yield start + 1, "\n".join(lines[start:j])
            i = j + 1
        else:
            i += 1


def main():
    verbose = "--verbose" in sys.argv
    members = workspace_members()
    scratch = tempfile.mkdtemp(prefix="igui_docs_")
    plugins = "\n".join(
        f'{name} = {{ path = "{path}" }}' for name, path in members
    )
    open(os.path.join(scratch, "Cargo.toml"), "w").write(
        "[package]\nname = \"igui_docs\"\nversion = \"0.0.0\"\nedition = \"2021\"\n"
        "\n[workspace]\n\n[dependencies]\n" + plugins + "\n"
    )
    os.makedirs(os.path.join(scratch, "src", "bin"))

    cases = []
    for md in markdown_files():
        rel = os.path.relpath(md, ROOT)
        for line, body in rust_blocks(md):
            if not body.strip():
                continue
            name = f"b{len(cases):03d}"
            code = body if "fn main" in body else "fn main() {\n" + body + "\n}\n"
            header = f"//! {rel}:{line}\n#![allow(unused, unused_variables, dead_code)]\n"
            open(os.path.join(scratch, "src", "bin", name + ".rs"), "w").write(header + code)
            cases.append((name, f"{rel}:{line}"))

    if not cases:
        print("no rust blocks found")
        return 0

    failed = {}
    for name, where in cases:
        proc = subprocess.run(
            ["cargo", "check", "-q", "--bin", name],
            cwd=scratch,
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            first = next(
                (l for l in proc.stderr.split("\n") if l.startswith("error")),
                "does not compile",
            )
            failed[name] = first.removeprefix("error: ").strip()

    shutil.rmtree(scratch, ignore_errors=True)

    if failed:
        print(f"{len(failed)} of {len(cases)} `rust` doc blocks do not compile:\n")
        for name, where in cases:
            if name in failed:
                print(f"  {where}\n      {failed[name]}")
        print("\nFix the snippet, or tag the fence `rust,ignore` and say what it needs.")
        return 1

    print(f"ok: {len(cases)} rust doc blocks compile")
    if verbose:
        for _, where in cases:
            print("  " + where)
    return 0


if __name__ == "__main__":
    sys.exit(main())
