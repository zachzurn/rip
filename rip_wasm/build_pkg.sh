#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# 1. Build WASM
echo "Building WASM..."
wasm-pack build . --target web --out-name rip_wasm --release

# 2. Copy hand-written wrapper into pkg/
echo "Copying JS wrapper..."
cp js/rip.mjs pkg/rip.mjs
cp js/rip.d.ts pkg/rip.d.ts

# 3. Replace wasm-pack's package.json with ours
cp package.json pkg/package.json

# 4. Clean up wasm-pack artifacts we don't need
rm -f pkg/.gitignore

echo ""
echo "Package ready in pkg/"
echo "To publish:  cd pkg && npm publish"
