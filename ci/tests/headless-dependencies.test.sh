#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

tree=$(mktemp)
trap 'rm -f "$tree"' EXIT
features=${1:-}
cargo tree --locked --no-default-features --features "$features" --edges normal --prefix none > "$tree"
if grep -E '^(gtk4|libadwaita|qt-display|qmetaobject|qt6)[[:space:]]' "$tree"; then
    echo 'FAIL: shared Linux app depends on a graphical toolkit' >&2
    exit 1
fi
echo "PASS: Linux app features '${features:-none}' have no GTK, libadwaita or Qt dependency"
