#!/usr/bin/env bash
# Build Cubera .app + DMG for macOS (Apple Silicon).
# Uses single-threaded Cargo to avoid linker/dylib corruption on newer macOS betas.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p "require('./package.json').version")"

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

# Ad-hoc sign so macOS does not treat the bundle as corrupt/unsigned junk.
# (Full Developer ID + notarization needs an Apple Developer certificate.)
codesign --force --deep --sign - \
  --options runtime \
  --timestamp=none \
  "$APP"
codesign --verify --deep --strict --verbose=2 "$APP" || true
xattr -cr "$APP" || true

STAGE=/tmp/Cubera-dmg-stage
OUT_DIR="$ROOT/dist-installer"
DMG="$OUT_DIR/Cubera_${VERSION}_aarch64.dmg"
rm -rf "$STAGE" "$OUT_DIR"
mkdir -p "$STAGE" "$OUT_DIR"

# ditto preserves app bundle structure better than cp -R
ditto "$APP" "$STAGE/Cubera.app"
ln -sf /Applications "$STAGE/Applications"

# Double-click helper for Gatekeeper quarantine ("app is damaged")
cat > "$STAGE/Fix & Open Cubera.command" <<'EOF'
#!/bin/bash
set -euo pipefail
APP="/Applications/Cubera.app"
if [[ ! -d "$APP" ]]; then
  APP="$(cd "$(dirname "$0")" && pwd)/Cubera.app"
fi
if [[ ! -d "$APP" ]]; then
  osascript -e 'display alert "Cubera not found" message "Drag Cubera into Applications first, then run this again." as critical'
  exit 1
fi
xattr -cr "$APP" 2>/dev/null || true
codesign --force --deep --sign - "$APP" 2>/dev/null || true
open "$APP"
EOF
chmod +x "$STAGE/Fix & Open Cubera.command"

hdiutil create -volname "Cubera" -srcfolder "$STAGE" -ov -format UDZO -imagekey zlib-level=9 "$DMG"
echo "Created $DMG"
ls -lah "$DMG"
