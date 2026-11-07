#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail
tree=$(mktemp)
trap 'rm -f "$tree"' EXIT
features=${1-qt}
cargo tree --locked --no-default-features --features "$features" --edges normal --prefix none > "$tree"
if grep -E '^(gtk4|libadwaita)[[:space:]]' "$tree"; then
    echo 'FAIL: Qt pairing depends on GTK or libadwaita' >&2
    exit 1
fi
if ! grep -qE '^qt-display[[:space:]]' "$tree"; then
    echo 'FAIL: Qt pairing did not select the Qt display' >&2
    exit 1
fi
echo 'PASS: Qt pairing selects qt-display without GTK or libadwaita'
