#!/usr/bin/env python3
"""Build a deterministic, self-contained server-backed Membench release bundle."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import pathlib
import shutil
import stat
import subprocess
import tarfile
import tempfile
import tomllib


FORBIDDEN_RECORD_PARTS = {
    "raw",
    "runs",
    "vaults",
    "workflow",
    "provider-queue",
    ".debug-session",
}
FORBIDDEN_RECORD_SUFFIXES = {".sqlite", ".sqlite3", ".db"}


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_digest(root: pathlib.Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(candidate for candidate in root.rglob("*") if candidate.is_file()):
        relative = path.relative_to(root).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(sha256_file(path).encode())
        digest.update(b"\n")
    return digest.hexdigest()


def git(*args: str, cwd: pathlib.Path) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def copy_file(source: pathlib.Path, destination: pathlib.Path, executable: bool = False) -> None:
    if source.is_symlink() or not source.is_file():
        raise SystemExit(f"refusing non-regular release input: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(0o755 if executable else 0o644)


def copy_tree(source: pathlib.Path, destination: pathlib.Path) -> None:
    for path in sorted(source.rglob("*")):
        if path.is_symlink():
            raise SystemExit(f"refusing symlink in release input: {path}")
        relative = path.relative_to(source)
        target = destination / relative
        if path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif path.is_file():
            copy_file(path, target)


def copy_portable_records(repo: pathlib.Path, destination: pathlib.Path) -> int:
    output = subprocess.check_output(
        ["git", "ls-files", "-z", "--", "records"], cwd=repo
    )
    copied = 0
    for raw in output.split(b"\0"):
        if not raw:
            continue
        relative = pathlib.PurePosixPath(os.fsdecode(raw))
        if any(part in FORBIDDEN_RECORD_PARTS for part in relative.parts):
            continue
        if relative.suffix.lower() in FORBIDDEN_RECORD_SUFFIXES:
            continue
        copy_file(repo / relative, destination / relative.relative_to("records"))
        copied += 1
    if copied == 0:
        raise SystemExit("no portable tracked records selected")
    return copied


def add_archive_member(archive: tarfile.TarFile, path: pathlib.Path, arcname: str, epoch: int) -> None:
    info = archive.gettarinfo(str(path), arcname=arcname)
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    info.mtime = epoch
    info.pax_headers = {}
    if path.is_dir():
        info.mode = 0o755
    elif path.stat().st_mode & stat.S_IXUSR:
        info.mode = 0o755
    else:
        info.mode = 0o644
    if path.is_file():
        with path.open("rb") as source:
            archive.addfile(info, source)
    else:
        archive.addfile(info)


def write_deterministic_archive(source: pathlib.Path, output: pathlib.Path, epoch: int) -> None:
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w|", format=tarfile.GNU_FORMAT) as archive:
                paths = [source, *sorted(source.rglob("*"))]
                for path in paths:
                    arcname = path.relative_to(source.parent).as_posix()
                    add_archive_member(archive, path, arcname, epoch)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=pathlib.Path, default=pathlib.Path(__file__).parent.parent)
    parser.add_argument("--binary-dir", type=pathlib.Path, required=True)
    parser.add_argument("--dashboard-dist", type=pathlib.Path, default=pathlib.Path("dashboard/dist"))
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--source-date-epoch", type=int)
    parser.add_argument("--exe-suffix", default="")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    repo = args.repo_root.resolve()
    binary_dir = args.binary_dir.resolve()
    dashboard_dist = args.dashboard_dist
    if not dashboard_dist.is_absolute():
        dashboard_dist = repo / dashboard_dist
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    manifest = tomllib.loads((repo / "Cargo.toml").read_text())
    dashboard_package = json.loads((repo / "dashboard/package.json").read_text())
    if manifest["package"]["version"] != args.version or dashboard_package["version"] != args.version:
        raise SystemExit("release version does not match Cargo.toml and dashboard/package.json")
    if manifest["package"].get("publish") is not False:
        raise SystemExit("Cargo package must remain publish=false for the GitHub bundle distribution")

    commit = git("rev-parse", "HEAD", cwd=repo)
    epoch = args.source_date_epoch
    if epoch is None:
        epoch = int(git("show", "-s", "--format=%ct", "HEAD", cwd=repo))
    root_name = f"membench-v{args.version}-{args.target}"
    archive_path = output_dir / f"{root_name}.tar.gz"
    provenance_path = output_dir / f"{root_name}.provenance.json"

    with tempfile.TemporaryDirectory(prefix="membench-release-") as scratch:
        product = pathlib.Path(scratch) / root_name
        product.mkdir()
        binaries: dict[str, str] = {}
        for name in ("membench-server", "membench-leaderboard"):
            filename = f"{name}{args.exe_suffix}"
            source = binary_dir / filename
            destination = product / filename
            copy_file(source, destination, executable=True)
            binaries[filename] = sha256_file(destination)

        copy_tree(dashboard_dist, product / "dashboard/dist")
        record_count = copy_portable_records(repo, product / "records")
        for name in ("README.md", "RELEASING.md", "SECURITY.md", "LICENSE"):
            copy_file(repo / name, product / name)

        release_readme = f"""# Membench v{args.version} server-backed product bundle

Target: `{args.target}`  
Source commit: `{commit}`

Start the read-only v2 product from this extracted directory:

```sh
./membench-server
```

Open <http://127.0.0.1:8787>. The server resolves this directory from the executable and serves
`dashboard/dist/` plus the portable tracked `records/` registry. Use
`./membench-leaderboard export --records-root records` for a headless export.

This public bundle intentionally excludes the private Symbiotic Memory adapter, native benchmark
state, local runs, credentials, raw provider payloads, and SQLite state. Build an authorized adapter
from source through `adapters/symbiotic-memory/Cargo.toml`; see the top-level README and RELEASING.md.
"""
        (product / "README-RELEASE.md").write_text(release_readme)
        (product / "README-RELEASE.md").chmod(0o644)

        snapshot = json.loads((repo / "dashboard/public/data/leaderboard.json").read_text())
        ui_version = json.loads((dashboard_dist / "version.json").read_text())
        provenance = {
            "schema": "membench.release.provenance.v1",
            "name": "membench",
            "version": args.version,
            "tag": f"v{args.version}",
            "source_commit": commit,
            "source_date_epoch": epoch,
            "target": args.target,
            "public_product": "server-backed-read-only-v2",
            "private_adapter_included": False,
            "portable_record_files": record_count,
            "records_digest": snapshot["source"]["records_digest"],
            "dashboard_bundle": ui_version.get("bundle", "unknown"),
            "dashboard_tree_sha256": tree_digest(product / "dashboard/dist"),
            "records_tree_sha256": tree_digest(product / "records"),
            "binaries": binaries,
        }
        encoded = json.dumps(provenance, indent=2, sort_keys=True) + "\n"
        (product / "PROVENANCE.json").write_text(encoded)
        (product / "PROVENANCE.json").chmod(0o644)
        provenance_path.write_text(encoded)

        write_deterministic_archive(product, archive_path, epoch)

    archive_sha = sha256_file(archive_path)
    checksum_path = archive_path.with_suffix(archive_path.suffix + ".sha256")
    checksum_path.write_text(f"{archive_sha}  {archive_path.name}\n")
    print(json.dumps({
        "archive": str(archive_path),
        "sha256": archive_sha,
        "provenance": str(provenance_path),
        "checksum": str(checksum_path),
    }, sort_keys=True))


if __name__ == "__main__":
    main()
