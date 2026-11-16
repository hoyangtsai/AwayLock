#!/bin/bash
# 打包 macOS App：Rust 核心（動態函式庫）+ Swift 原生介面，ad-hoc 簽章。
# 用法：scripts/bundle-macos.sh [release|dev]
#   release（預設）：發布版，不含任何 Debug 功能  → dist/AwayLock.app
#   dev            ：開發版，含 Debug 模式         → dist/AwayLock Dev.app
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"

MODE="${1:-release}"
case "$MODE" in
  release) CARGO_FLAGS="--release"; PROFILE=release; SWIFT_CONFIG=release; APP="dist/AwayLock.app";     BUNDLE_ID="com.awaylock.app";     NAME="AwayLock" ;;
  dev)     CARGO_FLAGS="";          PROFILE=debug;   SWIFT_CONFIG=debug;   APP="dist/AwayLock Dev.app"; BUNDLE_ID="com.awaylock.app.dev"; NAME="AwayLock Dev" ;;
  *) echo "用法：$0 [release|dev]"; exit 2 ;;
esac

# 1. Rust 核心 → libawaylock_ffi.dylib
# shellcheck disable=SC2086
cargo build -p awaylock-ffi $CARGO_FLAGS
DYLIB="$ROOT/target/$PROFILE/libawaylock_ffi.dylib"

# 2. Swift 原生介面，連結上面的動態函式庫
SDK_VERSION="$(xcrun --sdk macosx --show-sdk-version)"   # 使用目前安裝的最新 macOS SDK，系統才會套用最新的介面樣式
(
  cd apps/macos
  swift build -c "$SWIFT_CONFIG" \
    -Xlinker -L"$ROOT/target/$PROFILE" -Xlinker -lawaylock_ffi \
    -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
    -Xlinker -platform_version -Xlinker macos -Xlinker 13.0 -Xlinker "$SDK_VERSION"
)
SWIFT_BIN="$(cd apps/macos && swift build -c "$SWIFT_CONFIG" --show-bin-path)/AwayLock"

# 3. 組成 .app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Frameworks" "$APP/Contents/Resources"
cp "$SWIFT_BIN" "$APP/Contents/MacOS/AwayLock"
cp "$DYLIB" "$APP/Contents/Frameworks/libawaylock_ffi.dylib"
cp packaging/macos/Info.plist "$APP/Contents/Info.plist"
cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

# 開發版使用不同的 bundle id 與名稱，避免和發布版的設定、開機啟動項目混在一起
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $BUNDLE_ID" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName $NAME" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $NAME" "$APP/Contents/Info.plist"

codesign --force --sign - "$APP/Contents/Frameworks/libawaylock_ffi.dylib"
codesign --force --sign - "$APP"
echo "完成（$MODE）：$ROOT/$APP"
