#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/src-tauri/target/release/bundle/macos/Fairway XMP.app"
ARTIFACTS="$ROOT/artifacts"
STAGE="$ROOT/.runtime-work/pkg-root"
COMPONENTS="$ROOT/.runtime-work/fairway-components.plist"
[ -d "$APP" ] || { echo 'Build the Tauri application bundle first.' >&2; exit 1; }
mkdir -p "$ARTIFACTS"
rm -rf "$STAGE"
mkdir -p "$STAGE/Applications"
ditto "$APP" "$STAGE/Applications/Fairway XMP.app"
cp "$ROOT/packaging/macos/Uninstall Fairway XMP.command" "$STAGE/Applications/Fairway XMP.app/Contents/Resources/Uninstall Fairway XMP.command"
chmod +x "$ROOT/packaging/macos/scripts/preinstall" "$ROOT/packaging/macos/scripts/postinstall" "$STAGE/Applications/Fairway XMP.app/Contents/Resources/Uninstall Fairway XMP.command"
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  # Sign innermost binaries first; never use --deep as a substitute for signing dependencies.
  python3 "$ROOT/scripts/sign_macos.py" "$STAGE/Applications/Fairway XMP.app" "$APPLE_SIGNING_IDENTITY"
fi
VERSION=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1])).version)' "$ROOT/package.json")
pkgbuild --analyze --root "$STAGE" "$COMPONENTS"
# Keep the app in /Applications even when macOS finds a build copy with the same bundle ID.
plutil -replace '0.BundleIsRelocatable' -bool NO "$COMPONENTS"
pkgbuild --root "$STAGE" --component-plist "$COMPONENTS" --identifier dev.fairway.environment --version "$VERSION" --install-location / --scripts "$ROOT/packaging/macos/scripts" "$ARTIFACTS/Fairway-XMP-$VERSION-arm64-unsigned.pkg"
rm -rf "$STAGE"
rm -f "$COMPONENTS"
if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ]; then
  productsign --sign "$APPLE_INSTALLER_IDENTITY" "$ARTIFACTS/Fairway-XMP-$VERSION-arm64-unsigned.pkg" "$ARTIFACTS/Fairway-XMP-$VERSION-arm64.pkg"
fi
if [ -n "${APPLE_NOTARY_PROFILE:-}" ]; then
  xcrun notarytool submit "$ARTIFACTS/Fairway-XMP-$VERSION-arm64.pkg" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait
  xcrun stapler staple "$ARTIFACTS/Fairway-XMP-$VERSION-arm64.pkg"
fi
