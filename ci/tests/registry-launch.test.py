# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import importlib.util
import pathlib
import unittest
from unittest.mock import patch, sentinel


SPEC = importlib.util.spec_from_file_location(
    "registry_launch", pathlib.Path(__file__).resolve().parents[2]
    / "tests" / "atspi" / "registry_launch.py",
)
REGISTRY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REGISTRY)


class RegistryLaunchTest(unittest.TestCase):
    # @internal — only a known absolute system registry executable may start.
    def test_supported_registry_paths_launch_the_exact_executable(self):
        for allowed in (
            "/usr/libexec/at-spi2-registryd",
            "/usr/lib/at-spi2-core/at-spi2-registryd",
            "/usr/lib/at-spi2-registryd",
        ):
            with self.subTest(path=allowed):
                with patch.object(pathlib.Path, "is_file", lambda path: str(path) == allowed):
                    with patch.object(REGISTRY.subprocess, "Popen", return_value=sentinel.child) as start:
                        self.assertIs(REGISTRY.start_registry(), sentinel.child)
                        start.assert_called_once_with([allowed])

    # @internal — a PATH executable must not replace a missing system registry.
    def test_missing_system_registry_fails_without_starting_a_child(self):
        with patch.object(pathlib.Path, "is_file", return_value=False):
            with patch.object(REGISTRY.subprocess, "Popen") as start:
                with self.assertRaisesRegex(FileNotFoundError, "system AT-SPI registry is unavailable"):
                    REGISTRY.start_registry()
                start.assert_not_called()


if __name__ == "__main__":
    unittest.main()
