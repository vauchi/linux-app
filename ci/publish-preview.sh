#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Signs the Linux preview downloads with this pipeline's GitLab identity
# (keyless cosign, owner decision 2026-11-09, vauchi/private#1190), proves
# the signatures verify and that a changed file does not, then uploads them
# to the generic package registry. Without a release tag it signs and
# verifies but uploads nothing, so a merge request can show the job works.
#
# Usage (from the repo root, after package:appimage and build:terminal):
#   sh ci/publish-preview.sh
set -eu

tag=${CI_COMMIT_TAG:-}
if [ -n "$tag" ]; then
    # The full tag, v included: release:preview links these names and
    # CI variables cannot strip a prefix.
    version=$tag
    upload=yes
else
    version="mr-${CI_MERGE_REQUEST_IID:-local}"
    upload=no
fi

: "${SIGSTORE_ID_TOKEN:?SIGSTORE_ID_TOKEN is missing: the job needs id_tokens with aud sigstore}"
: "${CI_CONFIG_REF_URI:?CI_CONFIG_REF_URI is missing}"
: "${CI_SERVER_URL:?CI_SERVER_URL is missing}"
identity="https://${CI_CONFIG_REF_URI}"

out=preview
rm -rf "$out"
mkdir -p "$out"

set -- gvauchi-*-x86_64.AppImage
[ "$#" -eq 1 ] && [ -f "$1" ] || {
    echo "FAIL: expected exactly one gvauchi AppImage from package:appimage, found: $*" >&2
    exit 1
}
cp "$1" "$out/gvauchi-${version}-x86_64.AppImage"

terminal="vauchi-terminal-${version}-linux-x86_64"
mkdir -p "$terminal"
cp target/release/vauchi target/release/vauchi-tui "$terminal/"
cp -R LICENSES "$terminal/"
tar -czf "$out/${terminal}.tar.gz" "$terminal"
rm -rf "$terminal"

cd "$out"
sha256sum -- * > SHA256SUMS

for f in *; do
    cosign sign-blob --yes --bundle "${f}.sigstore.json" "$f"
done

for f in *.AppImage *.tar.gz SHA256SUMS; do
    cosign verify-blob --bundle "${f}.sigstore.json" \
        --certificate-identity "$identity" \
        --certificate-oidc-issuer "$CI_SERVER_URL" "$f"
done

# CC-27: a verification that cannot fail proves nothing.
cp SHA256SUMS ../tampered
printf 'x' >> ../tampered
if cosign verify-blob --bundle SHA256SUMS.sigstore.json \
    --certificate-identity "$identity" \
    --certificate-oidc-issuer "$CI_SERVER_URL" ../tampered 2>/dev/null; then
    echo "FAIL: a changed file verified against the signature" >&2
    exit 1
fi
rm -f ../tampered
echo "Signatures verify for ${identity}; a changed file is rejected"

if [ "$upload" = no ]; then
    echo "No release tag: signed and verified, nothing uploaded"
    exit 0
fi

base="${CI_API_V4_URL}/projects/${CI_PROJECT_ID}/packages/generic/vauchi-linux-preview/${version}"
for f in *; do
    status=$(curl -s -o ../upload.out -w '%{http_code}' \
        --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
        --upload-file "$f" "${base}/${f}") || status=000
    case "$status" in
        200 | 201) echo "Uploaded ${f}" ;;
        *)
            echo "FAIL: upload of ${f} → HTTP ${status}" >&2
            cat ../upload.out >&2
            exit 1
            ;;
    esac
done
echo "Published vauchi-linux-preview ${version}"
