#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

checker=$(cd "$(dirname "$0")/.." && pwd)/check-release-versions.sh
[ -f "$checker" ] || { echo 'FAIL: release version checker is missing' >&2; exit 1; }
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
fixture="$scratch/release fixture"
mkdir -p "$fixture/debian" "$fixture/packaging"

write_fixture() {
    printf '[package]\nname = "vauchi-linux-app"\nversion = "%s"\n' "$1" > "$fixture/Cargo.toml"
    printf 'gvauchi (%s-1) unstable; urgency=medium\n' "$2" > "$fixture/debian/changelog"
    printf 'Name: gvauchi\nVersion: %s\n' "$3" > "$fixture/packaging/gvauchi.spec"
    printf 'Name: qvauchi\nVersion: %s\n' "${4:-$3}" > "$fixture/packaging/qvauchi.spec"
}

expect_failure() {
    expected=$1
    if sh "$checker" "$fixture" > "$scratch/output" 2>&1; then
        echo "FAIL: accepted invalid metadata: $expected" >&2
        exit 1
    fi
    if ! grep -F "$expected" "$scratch/output" > /dev/null; then
        cat "$scratch/output" >&2
        echo "FAIL: missing diagnostic: $expected" >&2
        exit 1
    fi
}

write_fixture 0.10.1 0.10.1 0.10.1
actual=$(sh "$checker" "$fixture")
[ "$actual" = 'release versions agree: 0.10.1' ] || {
    echo "FAIL: unexpected valid-release result: $actual" >&2
    exit 1
}

write_fixture 0.10.1 0.8.1 0.10.1
expect_failure 'Debian package version 0.8.1 differs from Cargo 0.10.1'
write_fixture 0.10.1 0.10.1 0.8.1
expect_failure 'RPM package version 0.8.1 differs from Cargo 0.10.1'
write_fixture 0.10.1 0.8.1 0.8.1
expect_failure 'Debian package version 0.8.1 differs from Cargo 0.10.1'
write_fixture '' 0.10.1 0.10.1
expect_failure 'Cargo version must be X.Y.Z'
write_fixture 0.10.1-rc1 0.10.1 0.10.1
expect_failure 'Cargo version must be X.Y.Z'
write_fixture 0.10.1 0.10.1 0.10.1
printf 'gvauchi (0.10.1) unstable; urgency=medium\n' > "$fixture/debian/changelog"
expect_failure 'Cannot read Debian package version'
write_fixture 0.10.1 0.10.1 0.10.1
rm "$fixture/packaging/gvauchi.spec"
expect_failure 'Missing packaging/gvauchi.spec'
write_fixture 0.10.1 0.10.1 0.10.1 0.8.1
expect_failure 'RPM package version 0.8.1 (qvauchi) differs from Cargo 0.10.1'
write_fixture 0.10.1 0.10.1 0.10.1
rm "$fixture/packaging/qvauchi.spec"
expect_failure 'Missing packaging/qvauchi.spec'

echo 'PASS: release metadata agrees or fails before tagging (10 cases)'
