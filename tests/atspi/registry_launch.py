# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

from pathlib import Path
import os
import subprocess


def start_registry():
    inherited = (
        "DBUS_SESSION_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS", "DISPLAY", "XAUTHORITY",
        "XDG_RUNTIME_DIR", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME",
        "XDG_CURRENT_DESKTOP", "GTK_A11Y", "LANG", "LC_ALL",
    )
    environment = {key: os.environ[key] for key in inherited if key in os.environ}
    environment["PATH"] = "/usr/bin:/bin"
    # Fixed argv excludes PATH and variable executable selection (#1029).
    if Path("/usr/libexec/at-spi2-registryd").is_file():
        return subprocess.Popen(["/usr/libexec/at-spi2-registryd"], env=environment)
    if Path("/usr/lib/at-spi2-core/at-spi2-registryd").is_file():
        return subprocess.Popen(["/usr/lib/at-spi2-core/at-spi2-registryd"], env=environment)
    if Path("/usr/lib/at-spi2-registryd").is_file():
        return subprocess.Popen(["/usr/lib/at-spi2-registryd"], env=environment)
    raise FileNotFoundError("system AT-SPI registry is unavailable")
