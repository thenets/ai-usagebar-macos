#!/usr/bin/env bash
# Full install: build Rust engine, bundle macOS app, deploy to /Applications, restart.
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$DIR")"

echo "=== 1/4 Installing Rust engine ==="
cargo install --path "$ROOT"

echo "=== 2/4 Bundling AI Usage Bar.app ==="
"$DIR/bundle.sh"

echo "=== 3/4 Deploying to /Applications ==="
rm -rf "/Applications/AI Usage Bar.app"
cp -R "$DIR/AI Usage Bar.app" /Applications/

echo "=== 4/4 Restarting ==="
pkill -f "ai-usagebar-menubar" 2>/dev/null || true
sleep 1
open "/Applications/AI Usage Bar.app"

echo "✓ Done"
