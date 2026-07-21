#!/usr/bin/env bash
# Build Cubera .app + DMG for macOS (Apple Silicon).
# Uses single-threaded Cargo to avoid linker/dylib corruption on newer macOS betas.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

mkdir -p src-tauri/.cargo
cat > src-tauri/.cargo/config.toml <<'EOF'
[build]
jobs = 1
EOF

npm run build
(
  cd src-tauri
  cargo build --release --features tauri/custom-protocol
)
npm run tauri -- bundle --bundles app || true

APP="$ROOT/src-tauri/target/release/bundle/macos/Cubera.app"
if [[ ! -d "$APP" ]]; then
  echo "Cubera.app missing after bundle" >&2
  exit 1
fi

STAGE=/tmp/Cubera-dmg-stage
OUT_DIR="$ROOT/dist-installer"
DMG="$OUT_DIR/Cubera_0.1.0_aarch64.dmg"
rm -rf "$STAGE" "$OUT_DIR"
mkdir -p "$STAGE" "$OUT_DIR"
cp -R "$APP" "$STAGE/"
ln -sf /Applications "$STAGE/Applications"
hdiutil create -volname "Cubera" -srcfolder "$STAGE" -ov -format UDZO -imagekey zlib-level=9 "$DMG"
echo "Created $DMG"
ls -lah "$DMG"
