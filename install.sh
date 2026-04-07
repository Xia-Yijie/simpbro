#!/bin/sh
set -e

REPO="Xia-Yijie/simpbro"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

OS="$(uname -s)"
case "$OS" in
    Linux)  OS="linux" ;;
    Darwin) OS="darwin" ;;
    *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)  ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TAG="$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | head -1 | cut -d'"' -f4)"
if [ -z "$TAG" ]; then
    echo "Failed to get latest release"
    exit 1
fi

URL="https://github.com/$REPO/releases/download/$TAG/simpbro-${OS}-${ARCH}"
echo "Installing simpbro $TAG (${OS}/${ARCH})..."

mkdir -p "$INSTALL_DIR"
curl -sL "$URL" -o "$INSTALL_DIR/simpbro"
chmod +x "$INSTALL_DIR/simpbro"
# macOS: clear quarantine/provenance attributes that block execution
if [ "$OS" = "darwin" ]; then
    xattr -c "$INSTALL_DIR/simpbro" 2>/dev/null || true
fi

echo "Installed to $INSTALL_DIR/simpbro"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "Add $INSTALL_DIR to your PATH:"; echo "  export PATH=\"\$PATH:$INSTALL_DIR\"" ;;
esac
