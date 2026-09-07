#!/bin/sh
# Installs or updates ZedSuite in the Applications folder from the latest
# GitHub release, from Terminal:
#
#   curl -fsSL https://raw.githubusercontent.com/LeZed97/ZedSuite/master/install-macos.sh | sh
#
# Files downloaded by curl carry no quarantine flag, so the app opens
# directly, without the "Open Anyway" step a browser download needs.
set -eu

REPO="LeZed97/ZedSuite"
API="https://api.github.com/repos/$REPO/releases/latest"

URL=$(curl -fsSL "$API" \
  | grep -o '"browser_download_url": *"[^"]*macos-universal\.app\.tar\.gz"' \
  | head -1 | sed 's/.*"\(https[^"]*\)"/\1/')
if [ -z "$URL" ]; then
  echo "The latest ZedSuite release has no macOS build yet: https://github.com/$REPO/releases/latest"
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
echo "Downloading $URL"
curl -fL --progress-bar "$URL" -o "$TMP/ZedSuite.app.tar.gz"
tar -xzf "$TMP/ZedSuite.app.tar.gz" -C "$TMP"

DEST="/Applications"
if [ ! -w "$DEST" ]; then
  DEST="$HOME/Applications"
  mkdir -p "$DEST"
fi

if pgrep -x ZedSuite >/dev/null 2>&1; then
  echo "Closing the running ZedSuite..."
  pkill -x ZedSuite || true
  sleep 1
fi
rm -rf "$DEST/ZedSuite.app"
mv "$TMP/ZedSuite.app" "$DEST/ZedSuite.app"
echo "ZedSuite installed in $DEST"
open "$DEST/ZedSuite.app"
