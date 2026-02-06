#!/bin/sh
set -e

REPO="FrozenTear/solder"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

echo "Installing solder..."

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"

echo "  Downloading binary..."
curl -fSL "https://github.com/${REPO}/releases/latest/download/solder" -o "${BIN_DIR}/solder"
chmod +x "${BIN_DIR}/solder"

echo "  Installing desktop entry and icon..."
curl -fSL "https://raw.githubusercontent.com/${REPO}/master/assets/solder.svg" -o "${ICON_DIR}/solder.svg"
curl -fSL "https://raw.githubusercontent.com/${REPO}/master/assets/solder.desktop" -o /tmp/solder.desktop
sed -e "s|Exec=solder|Exec=${BIN_DIR}/solder|" \
    -e "s|Icon=solder|Icon=${ICON_DIR}/solder.svg|" \
    /tmp/solder.desktop > "${APP_DIR}/solder.desktop"
rm /tmp/solder.desktop

echo "Done!"
