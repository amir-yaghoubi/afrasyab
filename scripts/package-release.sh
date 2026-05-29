#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?version}"
TARGET="${2:?target triple}"
BIN_PATH="${3:?path to built binary}"

NAME="afrasyab-${VERSION}-${TARGET}"
STAGING="dist/${NAME}"
mkdir -p "$STAGING"

case "$TARGET" in
  *windows*) EXE="afrasyab.exe" ;;
  *) EXE="afrasyab" ;;
esac

cp "$BIN_PATH" "$STAGING/$EXE"
cat > "$STAGING/INSTALL.txt" <<'EOF'
Afrasyab release binary

Requires on PATH (not included in this archive):
  - yt-dlp (with JS runtime support for YouTube)
  - ffmpeg
  - deno

Configure via .env — see README in the repository.
EOF

mkdir -p dist
case "$TARGET" in
  *windows*)
    (cd dist && zip -r "${NAME}.zip" "${NAME}")
    echo "dist/${NAME}.zip"
    ;;
  *)
    tar -czf "dist/${NAME}.tar.gz" -C dist "${NAME}"
    echo "dist/${NAME}.tar.gz"
    ;;
esac
