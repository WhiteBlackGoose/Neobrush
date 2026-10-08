#!/usr/bin/env bash
# Builds a signed apt repository from the .deb files of the latest GitHub releases.
# Needs: APT_GPG_PRIVATE_KEY (armored secret key), GH_TOKEN, GITHUB_REPOSITORY; apt-utils, jq, gpg.
# Usage: web/build-apt.sh <output dir>
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=$1
POOL=$OUT/pool/main/n/neobrush
mkdir -p "$POOL" "$OUT/dists/stable/main/binary-amd64"
cp web/apt/neobrush.gpg web/apt/neobrush.asc web/apt/index.html "$OUT/"

AUTH=()
[ -n "${GH_TOKEN:-}" ] && AUTH=(-H "Authorization: Bearer $GH_TOKEN")
curl -fsSL "${AUTH[@]}" "https://api.github.com/repos/$GITHUB_REPOSITORY/releases?per_page=20" \
  | jq -r '[.[] | select(.draft | not)][:5][] | .assets[] | select(.name | endswith("_amd64.deb")) | .browser_download_url' \
  | while read -r url; do
      echo "fetching $url"
      curl -fsSL -o "$POOL/$(basename "$url")" "$url"
    done

cd "$OUT"
apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
gzip -9kf dists/stable/main/binary-amd64/Packages
apt-ftparchive \
  -o APT::FTPArchive::Release::Origin=Neobrush \
  -o APT::FTPArchive::Release::Label=Neobrush \
  -o APT::FTPArchive::Release::Suite=stable \
  -o APT::FTPArchive::Release::Codename=stable \
  -o APT::FTPArchive::Release::Architectures=amd64 \
  -o APT::FTPArchive::Release::Components=main \
  -o "APT::FTPArchive::Release::Description=Neobrush raster graphics editor" \
  release dists/stable > dists/stable/Release

export GNUPGHOME=$(mktemp -d)
echo "$APT_GPG_PRIVATE_KEY" | gpg --batch --import
gpg --batch --yes --pinentry-mode loopback --clearsign -o dists/stable/InRelease dists/stable/Release
gpg --batch --yes --pinentry-mode loopback -abs -o dists/stable/Release.gpg dists/stable/Release
rm -rf "$GNUPGHOME"
find . -type f | sort
