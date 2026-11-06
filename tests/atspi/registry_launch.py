# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

from pathlib import Path
import subprocess


def start_registry():
    # Fixed argv excludes PATH and variable executable selection (#1029).
    if Path("/usr/libexec/at-spi2-registryd").is_file():
        return subprocess.Popen(["/usr/libexec/at-spi2-registryd"])
    if Path("/usr/lib/at-spi2-core/at-spi2-registryd").is_file():
        return subprocess.Popen(["/usr/lib/at-spi2-core/at-spi2-registryd"])
    if Path("/usr/lib/at-spi2-registryd").is_file():
        return subprocess.Popen(["/usr/lib/at-spi2-registryd"])
    raise FileNotFoundError("system AT-SPI registry is unavailable")
