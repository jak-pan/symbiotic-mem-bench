#!/usr/bin/env python3
"""Generate deterministic notices for every dependency redistributed by v2 bundles."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
from collections import defaultdict


SUPPORTED_TARGETS = ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin")
NOTICE_NAMES = ("license", "licence", "copying", "copyright", "notice", "authors")
LONGMEMEVAL_LICENSE = """MIT License

Copyright (c) 2024 Di Wu

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""
MIT_TEMPLATE = """MIT License

Copyright (c) {holder}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=pathlib.Path, default=pathlib.Path(__file__).parent.parent)
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("THIRD_PARTY_NOTICES.md"))
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def notice_files(package_root: pathlib.Path) -> list[pathlib.Path]:
    selected = []
    for path in sorted(package_root.iterdir(), key=lambda item: item.name.casefold()):
        if not path.is_file() or path.is_symlink():
            continue
        lowered = path.name.casefold()
        if any(lowered.startswith(name) for name in NOTICE_NAMES):
            selected.append(path)
    return selected


def read_notice(path: pathlib.Path) -> str:
    data = path.read_bytes()
    if len(data) > 1024 * 1024:
        raise SystemExit(f"notice file exceeds 1 MiB: {path}")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SystemExit(f"notice file is not UTF-8: {path}") from error
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    return "\n".join(line.rstrip() for line in normalized.splitlines()).rstrip() + "\n"


def cargo_packages(repo: pathlib.Path) -> list[dict[str, str | pathlib.Path]]:
    packages: dict[tuple[str, str, str], dict[str, str | pathlib.Path]] = {}
    for target in SUPPORTED_TARGETS:
        raw = subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--features",
                "server",
                "--filter-platform",
                target,
            ],
            cwd=repo,
        )
        metadata = json.loads(raw)
        nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
        roots = [package["id"] for package in metadata["packages"] if package["source"] is None]
        resolved = set(roots)
        pending = list(roots)
        while pending:
            node = nodes[pending.pop()]
            for dependency in node["deps"]:
                if not any(kind["kind"] is None for kind in dependency["dep_kinds"]):
                    continue
                if dependency["pkg"] not in resolved:
                    resolved.add(dependency["pkg"])
                    pending.append(dependency["pkg"])
        for package in metadata["packages"]:
            if package["id"] not in resolved or package["source"] is None:
                continue
            key = (package["name"], package["version"], package["source"])
            packages[key] = {
                "ecosystem": "Cargo",
                "name": package["name"],
                "version": package["version"],
                "license": package.get("license") or "UNKNOWN",
                "source": package["source"],
                "authors": ", ".join(package.get("authors", [])),
                "root": pathlib.Path(package["manifest_path"]).parent,
            }
    return [packages[key] for key in sorted(packages)]


def npm_packages(repo: pathlib.Path) -> list[dict[str, str | pathlib.Path]]:
    lock = json.loads((repo / "dashboard/package-lock.json").read_text())
    packages = []
    for key, package in sorted(lock["packages"].items()):
        if not key or package.get("optional", False):
            continue
        if not key.startswith("node_modules/"):
            raise SystemExit(f"unexpected package-lock path: {key}")
        package_root = repo / "dashboard" / key
        package_json = json.loads((package_root / "package.json").read_text())
        author = package_json.get("author", "")
        if isinstance(author, dict):
            author = author.get("name", "")
        packages.append(
            {
                "ecosystem": "npm",
                "name": key.removeprefix("node_modules/"),
                "version": package["version"],
                "license": package.get("license", "UNKNOWN"),
                "source": package.get("resolved", "npm lockfile"),
                "authors": author,
                "root": package_root,
            }
        )
    return packages


def generate(repo: pathlib.Path) -> str:
    packages = [*cargo_packages(repo), *npm_packages(repo)]
    texts: dict[str, str] = {}
    references: dict[str, list[str]] = defaultdict(list)
    package_rows: list[tuple[str, str, str, str, str, str]] = []
    for package in packages:
        root = package["root"]
        assert isinstance(root, pathlib.Path)
        files = notice_files(root)
        digests = []
        label = f"{package['ecosystem']} {package['name']} {package['version']}"
        if not files:
            authors = package.get("authors", "")
            if package["license"] != "MIT" or not isinstance(authors, str) or not authors:
                raise SystemExit(f"no license/notice file found for {label}")
            text = MIT_TEMPLATE.format(holder=authors)
            digest = hashlib.sha256(text.encode()).hexdigest()
            texts.setdefault(digest, text)
            references[digest].append(f"{label} (MIT metadata + package author)")
            digests.append(digest[:12])
        for path in files:
            text = read_notice(path)
            digest = hashlib.sha256(text.encode()).hexdigest()
            texts.setdefault(digest, text)
            references[digest].append(f"{label} ({path.name})")
            digests.append(digest[:12])
        package_rows.append(
            (
                str(package["ecosystem"]),
                str(package["name"]),
                str(package["version"]),
                str(package["license"]),
                ", ".join(digests),
                str(package["source"]),
            )
        )

    longmemeval_digest = hashlib.sha256(LONGMEMEVAL_LICENSE.encode()).hexdigest()
    texts.setdefault(longmemeval_digest, LONGMEMEVAL_LICENSE)
    references[longmemeval_digest].append("LongMemEval dataset and benchmark artifacts (LICENSE)")

    lines = [
        "# Third-Party Notices",
        "",
        "This file is generated by `scripts/generate-third-party-notices.py` from the two supported",
        "Cargo target graphs and the locked, non-optional dashboard dependency graph. The release",
        "bundle redistributes compiled Rust dependencies, compiled SPA dependencies, and portable",
        "LongMemEval-derived record artifacts. License/notice text hashes make the inventory",
        "reproducible without machine-local paths.",
        "",
        "## LongMemEval dataset attribution",
        "",
        "The portable records contain questions, answers, and evaluation material derived from",
        "LongMemEval by Di Wu et al. Upstream source: <https://github.com/xiaowu0162/LongMemEval>.",
        f"The complete upstream MIT notice is reproduced below as `{longmemeval_digest[:12]}`.",
        "",
        "## Dependency inventory",
        "",
        "| Ecosystem | Package | Version | Declared license | Notice SHA-256 prefix | Locked source |",
        "|---|---|---:|---|---|---|",
    ]
    for ecosystem, name, version, license_name, digests, source in package_rows:
        lines.append(f"| {ecosystem} | `{name}` | `{version}` | `{license_name}` | `{digests}` | `{source}` |")

    lines.extend(["", "## Complete license and notice texts", ""])
    for digest in sorted(texts):
        lines.append(f"### `{digest}`")
        lines.append("")
        lines.append("Applies to:")
        lines.append("")
        for reference in sorted(set(references[digest])):
            lines.append(f"- {reference}")
        lines.extend(["", "```text", texts[digest].rstrip(), "```", ""])
    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    args = parse_args()
    repo = args.repo_root.resolve()
    output = args.output if args.output.is_absolute() else repo / args.output
    generated = generate(repo)
    if args.check:
        if not output.is_file() or output.read_text() != generated:
            print(f"FAIL: regenerate {output.relative_to(repo)}", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {output.relative_to(repo)} matches locked Cargo/npm notices")
        return
    output.write_text(generated)
    print(f"wrote {output} ({len(generated.encode())} bytes)")


if __name__ == "__main__":
    main()
