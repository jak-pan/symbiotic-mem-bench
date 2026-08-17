#!/usr/bin/env python3
"""Adversarial tests for release-archive input type validation."""

from __future__ import annotations

import importlib.util
import os
import pathlib
import socket
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "package-release.py"
SPEC = importlib.util.spec_from_file_location("package_release", SCRIPT)
assert SPEC and SPEC.loader
PACKAGE_RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE_RELEASE)


class ReleaseInputTypeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.source = self.root / "source"
        self.destination = self.root / "destination"
        self.source.mkdir()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def assert_tree_rejected(self, path: pathlib.Path) -> None:
        with self.assertRaises(SystemExit):
            PACKAGE_RELEASE.copy_tree(self.source, self.destination)
        if path.exists() or path.is_symlink():
            path.unlink()

    def test_regular_files_and_directories_are_copied(self) -> None:
        nested = self.source / "nested"
        nested.mkdir()
        (nested / "file.txt").write_text("portable\n")
        PACKAGE_RELEASE.copy_tree(self.source, self.destination)
        self.assertEqual((self.destination / "nested/file.txt").read_text(), "portable\n")

    def test_fifo_is_rejected(self) -> None:
        fifo = self.source / "payload.fifo"
        os.mkfifo(fifo)
        self.assert_tree_rejected(fifo)

    def test_unix_socket_is_rejected(self) -> None:
        path = self.source / "payload.sock"
        server = socket.socket(socket.AF_UNIX)
        try:
            server.bind(str(path))
            with self.assertRaises(SystemExit):
                PACKAGE_RELEASE.copy_tree(self.source, self.destination)
        finally:
            server.close()
            if path.exists():
                path.unlink()

    def test_symlink_is_rejected(self) -> None:
        target = self.root / "outside.txt"
        target.write_text("outside\n")
        link = self.source / "payload.link"
        link.symlink_to(target)
        self.assert_tree_rejected(link)

    def test_character_device_is_rejected(self) -> None:
        with self.assertRaises(SystemExit):
            PACKAGE_RELEASE.copy_file(pathlib.Path("/dev/null"), self.destination / "null")


if __name__ == "__main__":
    unittest.main()
