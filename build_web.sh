#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/neon_city.wasm web/
echo "Built. Serve with:  python3 -m http.server 8080 -d web"
