#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

echo "Building Deckhand release binary..."
cd "$REPO_ROOT"
cargo build --release

mkdir -p "$INSTALL_DIR"
cp "$REPO_ROOT/target/release/deckhand" "$INSTALL_DIR/deckhand"

if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "Warning: $INSTALL_DIR is not on your PATH."
    echo "Add this to your shell profile:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo "Installed deckhand to $INSTALL_DIR/deckhand"
