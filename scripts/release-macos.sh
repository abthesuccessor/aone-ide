#!/usr/bin/env bash
# Never inherit xtrace into a release that may inspect credential variables.
# This must run before any environment value is expanded in this script.
set +x
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
MODE="${1:---public}"
TARGET="${AONE_MACOS_TARGET:-aarch64-apple-darwin}"
MAX_BYTES_INPUT="${AONE_MAX_DMG_BYTES:-314572800}"
readonly PRODUCT_MAX_DMG_BYTES=314572800
readonly EXPECTED_BUNDLE_ID="com.aone.ide"
readonly EXPECTED_EXECUTABLE="aone-ide"

# Retain credentials only in non-exported shell variables. Child processes get
# none of them unless they are the one signing/notarization operation.
SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY-}"
API_ISSUER="${APPLE_API_ISSUER-}"
API_KEY="${APPLE_API_KEY-}"
API_KEY_PATH="${APPLE_API_KEY_PATH-}"
APPLE_ACCOUNT="${APPLE_ID-}"
APPLE_APP_PASSWORD="${APPLE_PASSWORD-}"
APPLE_ACCOUNT_TEAM="${APPLE_TEAM_ID-}"
EXPECTED_TEAM_ID="${AONE_EXPECTED_TEAM_ID-}"
NOTARY_PROFILE="${AONE_NOTARY_PROFILE-}"
DOWNLOAD_ORIGIN="${AONE_DOWNLOAD_ORIGIN-}"
export -n SIGNING_IDENTITY API_ISSUER API_KEY API_KEY_PATH
export -n APPLE_ACCOUNT APPLE_APP_PASSWORD APPLE_ACCOUNT_TEAM
export -n EXPECTED_TEAM_ID NOTARY_PROFILE DOWNLOAD_ORIGIN
unset APPLE_SIGNING_IDENTITY APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
unset AONE_RELEASE_PROOF_KEY AONE_NOTARY_PROFILE AONE_EXPECTED_TEAM_ID
unset AONE_DOWNLOAD_ORIGIN AONE_MACOS_TARGET AONE_MAX_DMG_BYTES
unset NODE_OPTIONS NODE_PATH BASH_ENV ENV CDPATH GLOBIGNORE
unset DYLD_INSERT_LIBRARIES DYLD_LIBRARY_PATH DYLD_FRAMEWORK_PATH
unset DYLD_FALLBACK_LIBRARY_PATH DYLD_FALLBACK_FRAMEWORK_PATH
unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
unset NPM_CONFIG_SCRIPT_SHELL npm_config_script_shell
unset NPM_CONFIG_USERCONFIG npm_config_userconfig
unset NPM_CONFIG_GLOBALCONFIG npm_config_globalconfig

MOUNT_DIR=""
MOUNT_ATTACHED=0
CANONICAL_TEMP=""
PROOF_PATH=""
PROOF_DIR=""
PROOF_KEY=""
cleanup() {
  if (( MOUNT_ATTACHED == 1 )) && [[ -n "$MOUNT_DIR" ]]; then
    /usr/bin/hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
  fi
  [[ -z "$MOUNT_DIR" ]] || /bin/rmdir "$MOUNT_DIR" >/dev/null 2>&1 || true
  [[ -z "$CANONICAL_TEMP" ]] || /bin/rm -f -- "$CANONICAL_TEMP" >/dev/null 2>&1 || true
  [[ -z "$PROOF_PATH" ]] || /bin/rm -f -- "$PROOF_PATH" >/dev/null 2>&1 || true
  [[ -z "$PROOF_DIR" ]] || /bin/rmdir "$PROOF_DIR" >/dev/null 2>&1 || true
  API_ISSUER=""
  API_KEY=""
  API_KEY_PATH=""
  APPLE_ACCOUNT=""
  APPLE_APP_PASSWORD=""
  PROOF_KEY=""
}
trap cleanup EXIT

fail() {
  echo "$1" >&2
  exit 1
}

if [[ "$MODE" != "--public" && "$MODE" != "--local-test" ]]; then
  fail "usage: $0 [--public|--local-test]"
fi

case "$TARGET" in
  aarch64-apple-darwin) ARCHITECTURE="Apple Silicon"; ARCHIVE_ARCH="aarch64" ;;
  x86_64-apple-darwin) ARCHITECTURE="Intel"; ARCHIVE_ARCH="x64" ;;
  universal-apple-darwin) ARCHITECTURE="Universal"; ARCHIVE_ARCH="universal" ;;
  *) fail "unsupported macOS target: $TARGET" ;;
esac

if [[ ! "$MAX_BYTES_INPUT" =~ ^[0-9]+$ ]] || (( ${#MAX_BYTES_INPUT} > 9 )); then
  fail "AONE_MAX_DMG_BYTES must be an ASCII decimal integer from 1 through $PRODUCT_MAX_DMG_BYTES"
fi
MAX_BYTES=$((10#$MAX_BYTES_INPUT))
if (( MAX_BYTES < 1 || MAX_BYTES > PRODUCT_MAX_DMG_BYTES )); then
  fail "AONE_MAX_DMG_BYTES must be an ASCII decimal integer from 1 through $PRODUCT_MAX_DMG_BYTES"
fi

[[ "$(/usr/bin/uname -s)" == "Darwin" ]] || fail "macOS releases must be built on macOS"

resolve_tool() {
  local tool_path
  tool_path="$(command -v "$1" || true)"
  [[ "$tool_path" == /* && -x "$tool_path" ]] || \
    fail "required executable $1 must resolve to an absolute executable path"
  printf '%s\n' "$tool_path"
}

NODE_BIN="$(resolve_tool node)"
NPM_BIN="$(resolve_tool npm)"
RUSTUP_BIN="$(resolve_tool rustup)"
CARGO_BIN="$(resolve_tool cargo)"
SAFE_PATH="${NODE_BIN%/*}:${NPM_BIN%/*}:${RUSTUP_BIN%/*}:${CARGO_BIN%/*}:/usr/bin:/bin:/usr/sbin:/sbin"
SANITIZED_ENV_ARGS=(
  -u APPLE_SIGNING_IDENTITY -u APPLE_API_ISSUER -u APPLE_API_KEY
  -u APPLE_API_KEY_PATH -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID
  -u AONE_RELEASE_PROOF_KEY -u AONE_NOTARY_PROFILE -u AONE_EXPECTED_TEAM_ID
  -u AONE_DOWNLOAD_ORIGIN -u AONE_MACOS_TARGET -u AONE_MAX_DMG_BYTES
  -u NODE_OPTIONS -u NODE_PATH -u BASH_ENV -u ENV -u CDPATH -u GLOBIGNORE -u SHELLOPTS
  -u DYLD_INSERT_LIBRARIES -u DYLD_LIBRARY_PATH -u DYLD_FRAMEWORK_PATH
  -u DYLD_FALLBACK_LIBRARY_PATH -u DYLD_FALLBACK_FRAMEWORK_PATH
  -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER
  -u NPM_CONFIG_SCRIPT_SHELL -u npm_config_script_shell
  -u NPM_CONFIG_USERCONFIG -u npm_config_userconfig
  -u NPM_CONFIG_GLOBALCONFIG -u npm_config_globalconfig
)

run_clean() {
  /usr/bin/env "${SANITIZED_ENV_ARGS[@]}" "PATH=$SAFE_PATH" "$@"
}

read_json_value() {
  run_clean "$NODE_BIN" -e '
    const { readFileSync } = require("node:fs");
    const document = JSON.parse(readFileSync(process.argv[1], "utf8"));
    const value = process.argv[2].split(".").reduce((current, key) => current?.[key], document);
    if (typeof value !== "string" || value.length === 0) process.exit(2);
    process.stdout.write(value);
  ' "$1" "$2"
}

VERSION="$(read_json_value "$PROJECT_ROOT/package.json" version)"
MINIMUM_MACOS="$(read_json_value "$PROJECT_ROOT/src-tauri/tauri.conf.json" bundle.macOS.minimumSystemVersion)"
[[ "$VERSION" =~ ^[0-9A-Za-z][0-9A-Za-z.+-]*$ ]] || \
  fail "release version contains characters that are unsafe in an artifact filename"
[[ "$MINIMUM_MACOS" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]] || \
  fail "bundle.macOS.minimumSystemVersion must be a numeric macOS version"

INSTALLED_TARGETS="$(run_clean "$RUSTUP_BIN" target list --installed)"
if [[ "$TARGET" == "universal-apple-darwin" ]]; then
  for REQUIRED_TARGET in aarch64-apple-darwin x86_64-apple-darwin; do
    /usr/bin/grep -Fxq -- "$REQUIRED_TARGET" <<< "$INSTALLED_TARGETS" || \
      fail "Rust target $REQUIRED_TARGET is required for a universal build"
  done
elif ! /usr/bin/grep -Fxq -- "$TARGET" <<< "$INSTALLED_TARGETS"; then
  fail "Rust target $TARGET is not installed. Run: rustup target add $TARGET"
fi

if [[ "$MODE" == "--public" ]]; then
  [[ -z "$API_ISSUER$API_KEY$API_KEY_PATH$APPLE_ACCOUNT$APPLE_APP_PASSWORD$APPLE_ACCOUNT_TEAM" ]] || \
    fail "raw Apple notarization credentials are not accepted; store credentials in Keychain and set AONE_NOTARY_PROFILE"
  [[ -n "$NOTARY_PROFILE" ]] || \
    fail "AONE_NOTARY_PROFILE must name a notarytool Keychain profile"
  [[ "$NOTARY_PROFILE" != *$'\n'* && "$NOTARY_PROFILE" != *$'\r'* ]] || \
    fail "AONE_NOTARY_PROFILE must not contain control characters"
  [[ -n "$SIGNING_IDENTITY" && "$SIGNING_IDENTITY" != "-" ]] || \
    fail "APPLE_SIGNING_IDENTITY must name a Developer ID Application certificate"
  [[ "$SIGNING_IDENTITY" != *$'\n'* && "$SIGNING_IDENTITY" != *$'\r'* ]] || \
    fail "APPLE_SIGNING_IDENTITY must not contain control characters"
  [[ "$SIGNING_IDENTITY" == "Developer ID Application: "* ]] || \
    fail "APPLE_SIGNING_IDENTITY must name a Developer ID Application certificate"
  [[ "$EXPECTED_TEAM_ID" =~ ^[A-Z0-9]{10}$ ]] || \
    fail "AONE_EXPECTED_TEAM_ID must be the 10-character Apple Developer Team ID"
  [[ "$SIGNING_IDENTITY" == *" ($EXPECTED_TEAM_ID)" ]] || \
    fail "APPLE_SIGNING_IDENTITY does not match AONE_EXPECTED_TEAM_ID"
  SIGNING_IDENTITIES="$(run_clean /usr/bin/security find-identity -v -p codesigning)"
  /usr/bin/grep -Fq -- "\"$SIGNING_IDENTITY\"" <<< "$SIGNING_IDENTITIES" || \
    fail "APPLE_SIGNING_IDENTITY is not available in the current keychain"

  [[ -n "$DOWNLOAD_ORIGIN" ]] || fail "AONE_DOWNLOAD_ORIGIN must be the public HTTPS site origin"
  NORMALIZED_ORIGIN="$(run_clean "$NODE_BIN" "$PROJECT_ROOT/scripts/lib/release-common.mjs" validate-origin "$DOWNLOAD_ORIGIN")"
else
  NORMALIZED_ORIGIN=""
  SIGNING_IDENTITY=""
  API_ISSUER=""
  API_KEY=""
  API_KEY_PATH=""
  APPLE_ACCOUNT=""
  APPLE_APP_PASSWORD=""
  APPLE_ACCOUNT_TEAM=""
  EXPECTED_TEAM_ID=""
  NOTARY_PROFILE=""
fi

cd "$PROJECT_ROOT"
run_clean "$NPM_BIN" ci --ignore-scripts
run_clean "$NODE_BIN" scripts/check-release-version.mjs
run_clean "$NPM_BIN" run check

DMG_DIR="$PROJECT_ROOT/src-tauri/target/$TARGET/release/bundle/dmg"
DMG_PATH="$DMG_DIR/Aone IDE_${VERSION}_${ARCHIVE_ARCH}.dmg"
[[ ! -L "$DMG_DIR" ]] || fail "refusing to use a symlinked DMG output directory"
if [[ -d "$DMG_DIR" ]]; then
  DMG_DIR_REAL="$(cd -- "$DMG_DIR" && pwd -P)"
  [[ "$DMG_DIR_REAL" == "$DMG_DIR" ]] || fail "DMG output directory escapes the expected build path"
fi
[[ ! -L "$DMG_PATH" ]] || fail "refusing to replace a symlinked DMG build output"
[[ ! -e "$DMG_PATH" ]] || /bin/rm -f -- "$DMG_PATH"

BUILD_STARTED_EPOCH="$(/bin/date +%s)"
if [[ "$MODE" == "--public" ]]; then
  TAURI_ENV=("CI=true" "APPLE_SIGNING_IDENTITY=$SIGNING_IDENTITY")
  run_clean "${TAURI_ENV[@]}" "$NPM_BIN" run tauri -- build \
    --bundles dmg \
    --target "$TARGET" \
    --config '{"build":{"beforeBuildCommand":null,"beforeBundleCommand":null}}'
else
  run_clean CI=true "$NPM_BIN" run tauri -- build --bundles dmg --target "$TARGET"
fi

unset TAURI_ENV || true
API_ISSUER=""
API_KEY=""
API_KEY_PATH=""
APPLE_ACCOUNT=""
APPLE_APP_PASSWORD=""
APPLE_ACCOUNT_TEAM=""

[[ -f "$DMG_PATH" && ! -L "$DMG_PATH" ]] || \
  fail "expected regular, non-symlink DMG was not produced: $DMG_PATH"
if [[ "$MODE" == "--public" ]]; then
  run_clean /usr/bin/xcrun notarytool submit "$DMG_PATH" --keychain-profile "$NOTARY_PROFILE" --wait
  run_clean /usr/bin/xcrun stapler staple "$DMG_PATH"
fi

DMG_DIR_REAL="$(cd -- "$DMG_DIR" && pwd -P)"
[[ "$DMG_DIR_REAL" == "$DMG_DIR" ]] || fail "DMG output directory escapes the expected build path"
DMG_MTIME="$(/usr/bin/stat -f '%m' "$DMG_PATH")"
BYTES="$(/usr/bin/stat -f '%z' "$DMG_PATH")"
[[ "$DMG_MTIME" =~ ^[0-9]+$ && "$BYTES" =~ ^[0-9]+$ ]] || fail "DMG metadata is not numeric"
(( DMG_MTIME >= BUILD_STARTED_EPOCH )) || fail "refusing to release a DMG older than the current build"
(( BYTES < MAX_BYTES )) || fail "DMG is $BYTES bytes, exceeding the configured $MAX_BYTES byte limit"
run_clean /usr/bin/hdiutil verify "$DMG_PATH"

RELEASE_DIR="$PROJECT_ROOT/release"
/bin/mkdir -p "$RELEASE_DIR"
[[ -d "$RELEASE_DIR" && ! -L "$RELEASE_DIR" ]] || fail "release directory must be a regular directory"
RELEASE_DIR_REAL="$(cd -- "$RELEASE_DIR" && pwd -P)"
[[ "$RELEASE_DIR_REAL" == "$RELEASE_DIR" ]] || fail "release directory escapes the project root"
CANONICAL_NAME="Aone-IDE_${VERSION}_${ARCHIVE_ARCH}.dmg"
CANONICAL_PATH="$RELEASE_DIR/$CANONICAL_NAME"
CANONICAL_TEMP="$(/usr/bin/mktemp "$RELEASE_DIR/.Aone-IDE.XXXXXX")"
/bin/cp "$DMG_PATH" "$CANONICAL_TEMP"
/usr/bin/cmp -s "$DMG_PATH" "$CANONICAL_TEMP" || fail "atomic staging copy differs from the built DMG"
/bin/chmod 0644 "$CANONICAL_TEMP"
/bin/mv -f "$CANONICAL_TEMP" "$CANONICAL_PATH"
CANONICAL_TEMP=""
[[ -f "$CANONICAL_PATH" && ! -L "$CANONICAL_PATH" ]] || fail "canonical DMG must be a regular, non-symlink file"
SOURCE_HASH="$(run_clean /usr/bin/shasum -a 256 "$DMG_PATH")"
SOURCE_HASH="${SOURCE_HASH%% *}"
CANONICAL_HASH="$(run_clean /usr/bin/shasum -a 256 "$CANONICAL_PATH")"
CANONICAL_HASH="${CANONICAL_HASH%% *}"
[[ "$SOURCE_HASH" == "$CANONICAL_HASH" ]] || fail "canonical DMG digest differs from the built DMG"

read_plist_value() {
  run_clean /usr/bin/plutil -extract "$2" raw -o - "$1"
}

verify_public_artifact() {
  local artifact_path="$1"
  local app_path info_plist bundle_id short_version build_version executable_name executable_path
  local codesign_details requirement arch_output vtool_output platform minos expected_arch
  local -a actual_arches expected_arches

  [[ -f "$artifact_path" && ! -L "$artifact_path" ]] || fail "public artifact must be a regular, non-symlink DMG"
  run_clean /usr/bin/hdiutil verify "$artifact_path"
  run_clean /usr/bin/xcrun stapler validate "$artifact_path"
  run_clean /usr/sbin/spctl --assess --type open --context context:primary-signature -vv "$artifact_path"
  MOUNT_DIR="$(/usr/bin/mktemp -d "/tmp/aone-release.XXXXXX")"
  run_clean /usr/bin/hdiutil attach "$artifact_path" -readonly -nobrowse -noautoopen -mountpoint "$MOUNT_DIR" >/dev/null
  MOUNT_ATTACHED=1

  app_path="$MOUNT_DIR/Aone IDE.app"
  [[ -d "$app_path" && ! -L "$app_path" ]] || fail "expected non-symlink Aone IDE.app is missing from the DMG"
  [[ -d "$app_path/Contents" && ! -L "$app_path/Contents" ]] || fail "application Contents directory is invalid"
  info_plist="$app_path/Contents/Info.plist"
  [[ -f "$info_plist" && ! -L "$info_plist" ]] || fail "application Info.plist is invalid"
  bundle_id="$(read_plist_value "$info_plist" CFBundleIdentifier)"
  short_version="$(read_plist_value "$info_plist" CFBundleShortVersionString)"
  build_version="$(read_plist_value "$info_plist" CFBundleVersion)"
  executable_name="$(read_plist_value "$info_plist" CFBundleExecutable)"
  [[ "$bundle_id" == "$EXPECTED_BUNDLE_ID" ]] || fail "signed bundle identifier does not match $EXPECTED_BUNDLE_ID"
  [[ "$short_version" == "$VERSION" && "$build_version" == "$VERSION" ]] || fail "signed bundle version does not match $VERSION"
  [[ "$executable_name" == "$EXPECTED_EXECUTABLE" ]] || fail "signed bundle executable does not match $EXPECTED_EXECUTABLE"
  [[ "$(read_plist_value "$info_plist" LSMinimumSystemVersion)" == "$MINIMUM_MACOS" ]] || \
    fail "signed bundle minimum macOS does not match $MINIMUM_MACOS"
  [[ -d "$app_path/Contents/MacOS" && ! -L "$app_path/Contents/MacOS" ]] || fail "application executable directory is invalid"
  executable_path="$app_path/Contents/MacOS/$executable_name"
  [[ -f "$executable_path" && -x "$executable_path" && ! -L "$executable_path" ]] || \
    fail "signed bundle executable is not a regular executable file"

  requirement="=anchor apple generic and identifier \"$EXPECTED_BUNDLE_ID\" and certificate leaf[subject.OU] = \"$EXPECTED_TEAM_ID\" and certificate leaf[field.1.2.840.113635.100.6.1.13] exists"
  run_clean /usr/bin/codesign --verify --deep --strict --verbose=2 -R "$requirement" "$app_path"
  codesign_details="$(run_clean /usr/bin/codesign -d --verbose=4 "$app_path" 2>&1)"
  /usr/bin/grep -Fxq -- "Identifier=$EXPECTED_BUNDLE_ID" <<< "$codesign_details" || fail "codesign identifier evidence is missing"
  /usr/bin/grep -Fxq -- "TeamIdentifier=$EXPECTED_TEAM_ID" <<< "$codesign_details" || fail "codesign TeamIdentifier does not match"
  /usr/bin/grep -Fxq -- "Authority=$SIGNING_IDENTITY" <<< "$codesign_details" || fail "codesign authority does not match APPLE_SIGNING_IDENTITY"
  ! /usr/bin/grep -Fxq -- "Signature=adhoc" <<< "$codesign_details" || fail "public application must not use an ad-hoc signature"
  run_clean /usr/sbin/spctl --assess --type execute -vv "$app_path"

  arch_output="$(run_clean /usr/bin/lipo -archs "$executable_path")"
  read -r -a actual_arches <<< "$arch_output"
  case "$TARGET" in
    aarch64-apple-darwin) expected_arches=(arm64) ;;
    x86_64-apple-darwin) expected_arches=(x86_64) ;;
    universal-apple-darwin) expected_arches=(arm64 x86_64) ;;
  esac
  (( ${#actual_arches[@]} == ${#expected_arches[@]} )) || fail "signed executable architecture set does not match $TARGET"
  for expected_arch in "${expected_arches[@]}"; do
    [[ " ${actual_arches[*]} " == *" $expected_arch "* ]] || fail "signed executable architecture set does not match $TARGET"
    vtool_output="$(run_clean /usr/bin/xcrun vtool -arch "$expected_arch" -show-build "$executable_path")"
    platform="$(/usr/bin/awk '$1 == "platform" { print $2 }' <<< "$vtool_output")"
    minos="$(/usr/bin/awk '$1 == "minos" { print $2 }' <<< "$vtool_output")"
    [[ "$platform" == "MACOS" || "$platform" == "macOS" ]] || fail "signed executable does not target macOS"
    [[ "$minos" == "$MINIMUM_MACOS" ]] || fail "signed executable minimum macOS does not match $MINIMUM_MACOS"
  done

  run_clean /usr/bin/hdiutil detach "$MOUNT_DIR" >/dev/null
  MOUNT_ATTACHED=0
  /bin/rmdir "$MOUNT_DIR"
  MOUNT_DIR=""
}

if [[ "$MODE" == "--public" ]]; then
  [[ -x /usr/bin/openssl ]] || fail "/usr/bin/openssl is required to create the ephemeral release proof"
  PROOF_KEY="$(run_clean /usr/bin/openssl rand -hex 32)"
  [[ "$PROOF_KEY" =~ ^[a-f0-9]{64}$ ]] || fail "failed to create an ephemeral release proof key"
  PROOF_DIR="$(/usr/bin/mktemp -d "/tmp/aone-proof.XXXXXX")"
  /bin/chmod 0700 "$PROOF_DIR"
  PROOF_PATH="$PROOF_DIR/release-proof.json"
  run_clean "AONE_RELEASE_PROOF_KEY=$PROOF_KEY" "$NODE_BIN" scripts/lib/release-common.mjs issue-proof \
    --artifact "$CANONICAL_PATH" --version "$VERSION" --target "$TARGET" \
    --architecture "$ARCHITECTURE" --minimum-macos "$MINIMUM_MACOS" \
    --origin "$NORMALIZED_ORIGIN" --output "$PROOF_PATH"
  run_clean "AONE_RELEASE_PROOF_KEY=$PROOF_KEY" "$NODE_BIN" scripts/build-download-site.mjs \
    --artifact "$CANONICAL_PATH" --version "$VERSION" --target "$TARGET" \
    --architecture "$ARCHITECTURE" --minimum-macos "$MINIMUM_MACOS" \
    --channel public-candidate --origin "$NORMALIZED_ORIGIN" --release-proof "$PROOF_PATH"
  PROOF_KEY=""
  /bin/rm -f -- "$PROOF_PATH"
  PROOF_PATH=""
  /bin/rmdir "$PROOF_DIR"
  PROOF_DIR=""

  STAGED_DMG_PATH="$PROJECT_ROOT/release/site/downloads/$CANONICAL_NAME"
  [[ -f "$STAGED_DMG_PATH" && ! -L "$STAGED_DMG_PATH" ]] || fail "staged public DMG is missing or is a symlink"
  /usr/bin/cmp -s "$CANONICAL_PATH" "$STAGED_DMG_PATH" || fail "staged public DMG differs from the verified build output"
  verify_public_artifact "$STAGED_DMG_PATH"
  run_clean "$NODE_BIN" scripts/verify-release.mjs --public \
    --expected-version "$VERSION" --expected-target "$TARGET" --expected-architecture "$ARCHITECTURE" \
    --expected-minimum-macos "$MINIMUM_MACOS" --expected-team-id "$EXPECTED_TEAM_ID" \
    --expected-signing-identity "$SIGNING_IDENTITY" --expected-bundle-id "$EXPECTED_BUNDLE_ID" \
    --expected-executable "$EXPECTED_EXECUTABLE" --origin "$NORMALIZED_ORIGIN" --max-bytes "$MAX_BYTES"
else
  run_clean "$NODE_BIN" scripts/build-download-site.mjs \
    --artifact "$CANONICAL_PATH" --version "$VERSION" --target "$TARGET" \
    --architecture "$ARCHITECTURE" --minimum-macos "$MINIMUM_MACOS" --channel local-test
  STAGED_DMG_PATH="$PROJECT_ROOT/release/site/downloads/$CANONICAL_NAME"
  [[ -f "$STAGED_DMG_PATH" && ! -L "$STAGED_DMG_PATH" ]] || fail "staged local DMG is missing or is a symlink"
  /usr/bin/cmp -s "$CANONICAL_PATH" "$STAGED_DMG_PATH" || fail "staged local DMG differs from the built DMG"
  run_clean "$NODE_BIN" scripts/verify-release.mjs --max-bytes "$MAX_BYTES"
fi

echo "release site staged at $PROJECT_ROOT/release/site"
