#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The Debian packages linux-app builds: gvauchi (GTK) and qvauchi (Qt),
# one or the other (maintainer, 2026-11-11, private#1029). Both apps use
# the same data directory, so the packages conflict; qvauchi takes over
# from the legacy vauchi-qt; each package's desktop entry starts its own
# binary, and only GTK is D-Bus activatable.
#
# usage: packages.test.sh <directory with the built .deb files>
set -eu

dir=${1:?usage: packages.test.sh <deb directory>}
command -v dpkg-deb >/dev/null || { echo "packages.test.sh: dpkg-deb is missing"; exit 2; }
failures=0

fail() {
    echo "FAIL: $1"
    failures=$((failures + 1))
}

deb_of() {
    find "$dir" -maxdepth 1 -name "$1_*.deb" | head -n 1
}

field() {
    dpkg-deb --field "$2" "$1" 2>/dev/null || true
}

ships() {
    dpkg-deb --contents "$1" | grep -E " \\./$2\$" >/dev/null
}

entry() {
    dpkg-deb --fsys-tarfile "$1" | tar -xOf - ./usr/share/applications/app.vauchi.Vauchi.desktop 2>/dev/null || true
}

gtk=$(deb_of gvauchi)
qt=$(deb_of qvauchi)
[ -n "$gtk" ] || fail "no gvauchi package in $dir"
[ -n "$qt" ] || fail "no qvauchi package in $dir"

if [ -n "$gtk" ] && [ -n "$qt" ]; then
    field Conflicts "$gtk" | grep -qw qvauchi || fail "gvauchi does not conflict with qvauchi"
    field Conflicts "$qt" | grep -qw gvauchi || fail "qvauchi does not conflict with gvauchi"
    for relation in Replaces Breaks Provides; do
        field "$relation" "$qt" | grep -qw vauchi-qt || fail "qvauchi lacks $relation: vauchi-qt"
    done
    ships "$qt" usr/bin/qvauchi || fail "qvauchi does not ship usr/bin/qvauchi"
    ships "$gtk" usr/bin/gvauchi || fail "gvauchi does not ship usr/bin/gvauchi"
    entry "$qt" | grep -qx 'Exec=qvauchi %u' || fail "qvauchi's desktop entry does not start qvauchi"
    entry "$gtk" | grep -qx 'Exec=gvauchi %u' || fail "gvauchi's desktop entry does not start gvauchi"
    if entry "$qt" | grep -q '^DBusActivatable=true'; then
        fail "qvauchi's desktop entry claims D-Bus activation"
    fi
    if dpkg-deb --contents "$qt" | grep 'dbus-1/services/' >/dev/null; then
        fail "qvauchi ships a D-Bus service file"
    fi
fi

if [ "$failures" -ne 0 ]; then
    echo "packages.test.sh: $failures failure(s)"
    exit 1
fi
echo "packages.test.sh: OK"
