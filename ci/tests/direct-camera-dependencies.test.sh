#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu
tree=$(mktemp)
trap 'rm -f "$tree"' EXIT
cargo tree --locked --no-default-features --features "${1:-capture-v4l2}" --edges normal --prefix none > "$tree"
if grep -E '^(pipewire|pipewire-sys|libspa|libspa-sys)[[:space:]]' "$tree"; then
    echo 'FAIL: the direct camera provider pulls in PipeWire' >&2
    exit 1
fi
if ! grep -E '^nokhwa-bindings-linux[[:space:]]' "$tree" >/dev/null; then
    echo 'FAIL: the direct camera provider has no V4L2 capture backend' >&2
    exit 1
fi
echo 'PASS: direct camera capture requires no PipeWire library'
