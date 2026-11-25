#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/src-tauri/target/release/bundle/macos/Fairway XMP.app"
ARTIFACTS="$ROOT/artifacts"
STAGE="$ROOT/.runtime-work/pkg-root"
COMPONENTS="$ROOT/.runtime-work/fairway-components.plist"
[ -d "$APP" ] || { echo 'Build the Tauri application bundle first.' >&2; exit 1; }
# CI signs on a fresh runner. Execute bundled code only before credentials are
# imported or after they are removed; local packaging retains its usual checks.
if [ "${FAIRWAY_DEFER_RUNTIME_CHECKS:-0}" != 1 ]; then
  python3 "$ROOT/scripts/verify_macos_permissions.py" "$APP"
fi
mkdir -p "$ARTIFACTS" "$ROOT/.runtime-work"
chmod +x "$APP/Contents/Resources/Uninstall Fairway XMP.command"
VERSION=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1])).version)' "$ROOT/package.json")
if [ "${FAIRWAY_SIGNED_RELEASE:-0}" = 1 ]; then
  : "${APPLE_SIGNING_IDENTITY:?Developer ID Application identity is required}"
  : "${APPLE_INSTALLER_IDENTITY:?Developer ID Installer identity is required}"
  : "${APPLE_NOTARY_PROFILE:?A validated notarization profile is required}"
  if [ "${FAIRWAY_DEFER_UPDATER_SIGNATURE:-0}" != 1 ]; then
    : "${TAURI_SIGNING_PRIVATE_KEY:?Updater signing key is required}"
  fi
fi
notarize() {
  if [ -n "${APPLE_NOTARY_KEYCHAIN:-}" ]; then
    xcrun notarytool submit "$1" --keychain-profile "$APPLE_NOTARY_PROFILE" --keychain "$APPLE_NOTARY_KEYCHAIN" --wait --timeout 30m
  else
    xcrun notarytool submit "$1" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait --timeout 30m
  fi
}
# Sign/notarize the final app itself. Both the pkg and updater archive must
# contain these exact stapled bytes; notarizing only the pkg is insufficient.
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  python3 "$ROOT/scripts/sign_macos.py" "$APP" "$APPLE_SIGNING_IDENTITY"
  if [ "${FAIRWAY_DEFER_RUNTIME_CHECKS:-0}" != 1 ]; then
    python3 "$ROOT/scripts/check_signed_php.py" "$APP"
    python3 "$ROOT/scripts/verify_macos_permissions.py" "$APP" --team-id "${FAIRWAY_APPLE_TEAM_ID:?Set your Apple Developer team ID}"
  fi
fi
if [ -n "${APPLE_NOTARY_PROFILE:-}" ]; then
  ZIP="$ROOT/.runtime-work/notarize-app.zip"
  ditto -c -k --keepParent "$APP" "$ZIP"
  notarize "$ZIP"
  python3 "$ROOT/scripts/staple_macos.py" "$APP"
  codesign --verify --deep --strict "$APP"
  spctl --assess --type execute --verbose=2 "$APP"
  # Exact distribution app: register and unregister without approval, Settings,
  # XPC or hosts writes. Approved-helper migration remains a device check.
  if [ "${FAIRWAY_DEFER_RUNTIME_CHECKS:-0}" != 1 ]; then
    "$APP/Contents/MacOS/fairway" --test-native-hosts-registration
  fi
  rm -f "$ZIP"
fi
rm -rf "$STAGE"
mkdir -p "$STAGE/Applications"
ditto "$APP" "$STAGE/Applications/Fairway XMP.app"
pkgbuild --analyze --root "$STAGE" "$COMPONENTS"
plutil -replace '0.BundleIsRelocatable' -bool NO "$COMPONENTS"
UNSIGNED="$ARTIFACTS/fairway-xmp-$VERSION-macos-arm64-unsigned.pkg"
PKG="$ARTIFACTS/fairway-xmp-$VERSION-macos-arm64.pkg"
pkgbuild --root "$STAGE" --component-plist "$COMPONENTS" --identifier dev.fairway.environment --version "$VERSION" --install-location / --scripts "$ROOT/packaging/macos/scripts" "$UNSIGNED"
rm -rf "$STAGE"
rm -f "$COMPONENTS"
if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ]; then
  productsign --sign "$APPLE_INSTALLER_IDENTITY" "$UNSIGNED" "$PKG"
  pkgutil --check-signature "$PKG"
  rm -f "$UNSIGNED"
fi
if [ -n "${APPLE_NOTARY_PROFILE:-}" ]; then
  notarize "$PKG"
  python3 "$ROOT/scripts/staple_macos.py" "$PKG"
  spctl --assess --type install --verbose=2 "$PKG"
fi
if [ "${FAIRWAY_SIGNED_RELEASE:-0}" = 1 ]; then
  ARCHIVE="$ARTIFACTS/fairway-xmp-$VERSION-macos-arm64.app.tar.gz"
  COPYFILE_DISABLE=1 tar -czf "$ARCHIVE" -C "$(dirname "$APP")" 'Fairway XMP.app'
  # Pin the signature to the version as well as the archive bytes.
  if [ "${FAIRWAY_DEFER_UPDATER_SIGNATURE:-0}" != 1 ]; then
    node "$ROOT/node_modules/@tauri-apps/cli/tauri.js" signer sign --app-version "$VERSION" "$ARCHIVE"
  fi
fi
