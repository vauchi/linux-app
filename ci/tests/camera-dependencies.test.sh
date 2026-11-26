#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

tree=$(mktemp)
trap 'rm -f "$tree"' EXIT
features=${1:-gtk}
cargo tree --locked --no-default-features --features "$features" --edges normal --prefix none > "$tree"
if grep -E '^(pipewire|pipewire-sys|libspa|libspa-sys|nokhwa[^[:space:]]*|v4l[^[:space:]]*)[[:space:]]' "$tree"; then
    echo 'FAIL: the camera-disabled app pulls in a native camera backend' >&2
    exit 1
fi
echo "PASS: Linux app features '$features' require no PipeWire or direct-camera library"
