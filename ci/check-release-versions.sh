#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

root=${1:-.}
for file in Cargo.toml debian/changelog packaging/gvauchi.spec packaging/qvauchi.spec; do
    [ -f "$root/$file" ] || { echo "Missing $file" >&2; exit 1; }
done

cargo_version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$root/Cargo.toml")
if ! printf '%s\n' "$cargo_version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo 'Cargo version must be X.Y.Z' >&2
    exit 1
fi
debian_version=$(sed -n '1s/^gvauchi (\([0-9][0-9.]*\)-[^)]*) .*$/\1/p' "$root/debian/changelog")
rpm_version=$(sed -n 's/^Version:[[:space:]]*\([^[:space:]]*\)[[:space:]]*$/\1/p' "$root/packaging/gvauchi.spec")
[ -n "$debian_version" ] || { echo 'Cannot read Debian package version' >&2; exit 1; }
[ -n "$rpm_version" ] || { echo 'Cannot read RPM package version' >&2; exit 1; }
[ "$debian_version" = "$cargo_version" ] || {
    echo "Debian package version $debian_version differs from Cargo $cargo_version" >&2
    exit 1
}
[ "$rpm_version" = "$cargo_version" ] || {
    echo "RPM package version $rpm_version differs from Cargo $cargo_version" >&2
    exit 1
}
qt_rpm_version=$(sed -n 's/^Version:[[:space:]]*\([^[:space:]]*\)[[:space:]]*$/\1/p' "$root/packaging/qvauchi.spec")
[ "$qt_rpm_version" = "$cargo_version" ] || {
    echo "RPM package version ${qt_rpm_version:-(none)} (qvauchi) differs from Cargo $cargo_version" >&2
    exit 1
}
echo "release versions agree: $cargo_version"
