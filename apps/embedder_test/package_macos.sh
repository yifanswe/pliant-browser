#!/bin/bash
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
: "${PLIANT_CHROMIUM_LIB_DIR:?Set this to the Chromium output directory containing PliantContent.framework}"
native_out="$(cd "$PLIANT_CHROMIUM_LIB_DIR" && pwd)"
framework_source="$native_out/PliantContent.framework"
framework_binary="$framework_source/PliantContent"

package_only=false
app=""
bundle_id=""
bundle_name=""
example=""
runtime_args=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --package-only)
      package_only=true
      shift
      ;;
    --app-path)
      [[ $# -ge 2 ]] || { printf '%s\n' '--app-path requires a value' >&2; exit 2; }
      app="$2"
      shift 2
      ;;
    --bundle-id)
      [[ $# -ge 2 ]] || { printf '%s\n' '--bundle-id requires a value' >&2; exit 2; }
      bundle_id="$2"
      shift 2
      ;;
    --bundle-name)
      [[ $# -ge 2 ]] || { printf '%s\n' '--bundle-name requires a value' >&2; exit 2; }
      bundle_name="$2"
      shift 2
      ;;
    --example)
      [[ $# -ge 2 ]] || { printf '%s\n' '--example requires a value' >&2; exit 2; }
      example="$2"
      shift 2
      ;;
    --)
      shift
      runtime_args=("$@")
      break
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      exit 2
      ;;
  esac
done

[[ -n "$app" ]] || { printf '%s\n' '--app-path is required' >&2; exit 2; }
[[ -n "$bundle_id" ]] || { printf '%s\n' '--bundle-id is required' >&2; exit 2; }
[[ -n "$bundle_name" ]] || { printf '%s\n' '--bundle-name is required' >&2; exit 2; }
[[ "$bundle_id" =~ ^[A-Za-z0-9][A-Za-z0-9.-]*$ ]] || {
  printf '%s\n' '--bundle-id contains unsupported characters' >&2
  exit 2
}
case "$example" in
  ""|navigation|threading|window) ;;
  *) printf '%s\n' '--example must be navigation, threading, or window' >&2; exit 2 ;;
esac
[[ "$app" == *.app ]] || { printf '%s\n' '--app-path must end in .app' >&2; exit 2; }
app_parent="$(cd "$(dirname "$app")" && pwd)"
[[ "$app_parent" == "$native_out" ]] || {
  printf '%s\n' 'the test bundle must be a direct child of PLIANT_CHROMIUM_LIB_DIR' >&2
  exit 2
}
case "$(basename "$app")" in
  PliantMvp.app|PliantBrowser.app)
    printf '%s\n' 'refusing to replace a production or existing MVP bundle' >&2
    exit 2
    ;;
esac
[[ ! -e "$app" ]] || { printf 'refusing to overwrite existing bundle: %s\n' "$app" >&2; exit 2; }
[[ -f "$framework_binary" ]] || { printf 'missing framework binary: %s\n' "$framework_binary" >&2; exit 1; }
nm -gU "$framework_binary" | grep '_pliant_page_create_in$' >/dev/null || {
  printf '%s\n' 'PliantContent.framework lacks the public host-container API' >&2
  exit 1
}

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$native_out/rust-embedder-test}"
export PLIANT_CHROMIUM_LIB_DIR="$native_out"
if [[ -n "$example" ]]; then
  cargo build --offline --locked \
    --manifest-path "$root/apps/embedder_test/Cargo.toml" \
    --example "$example"
  source_binary="$CARGO_TARGET_DIR/debug/examples/$example"
else
  cargo build --offline --locked \
    --manifest-path "$root/apps/embedder_test/Cargo.toml" \
    --bin pliant-embedder-test
  source_binary="$CARGO_TARGET_DIR/debug/pliant-embedder-test"
fi

staging="$native_out/.$(basename "$app").packaging.$$"
trap 'rm -rf "$staging"' EXIT
mkdir -p "$staging/Contents/MacOS" "$staging/Contents/Frameworks"
cp "$root/apps/embedder_test/native/app.plist" "$staging/Contents/Info.plist"
plutil -replace CFBundleIdentifier -string "$bundle_id" "$staging/Contents/Info.plist"
plutil -replace CFBundleName -string "$bundle_name" "$staging/Contents/Info.plist"
cp "$source_binary" "$staging/Contents/MacOS/pliant-embedder-test"
otool -l "$staging/Contents/MacOS/pliant-embedder-test" |
  grep '@executable_path/../Frameworks' >/dev/null || {
    printf '%s\n' 'test executable is missing its bundle Frameworks LC_RPATH' >&2
    exit 1
  }
ditto "$framework_source" "$staging/Contents/Frameworks/PliantContent.framework"
framework="$staging/Contents/Frameworks/PliantContent.framework"
for suffix in '' ' (Renderer)' ' (GPU)'; do
  helper="$framework/Helpers/PliantHelper$suffix.app"
  if [[ "$suffix" == ' (Renderer)' ]]; then
    codesign --force --sign - \
      --entitlements "$root/embedder/chromium/renderer-entitlements.plist" \
      "$helper"
  else
    codesign --force --sign - "$helper"
  fi
done
codesign --force --sign - "$framework"
codesign --force --sign - "$staging"
mv "$staging" "$app"
trap - EXIT

if [[ "$package_only" == true ]]; then
  printf 'Packaged without launching: %s (%s)\n' "$app" "$bundle_id"
  exit 0
fi

exec "$app/Contents/MacOS/pliant-embedder-test" "${runtime_args[@]}"
