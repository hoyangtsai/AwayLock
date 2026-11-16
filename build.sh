#!/bin/bash
# 用法：./build.sh [release|dev]
#   release（預設）：發布版，不含任何 Debug 功能  → build/AwayLock.app
#   dev            ：開發版，含 Debug 模式         → build/AwayLock Dev.app
set -euo pipefail
cd "$(dirname "$0")"

MODE="${1:-release}"
case "$MODE" in
  release) CONFIG=release; APP="build/AwayLock.app";     BUNDLE_ID="com.awaylock.app";     NAME="AwayLock" ;;
  dev)     CONFIG=debug;   APP="build/AwayLock Dev.app"; BUNDLE_ID="com.awaylock.app.dev"; NAME="AwayLock Dev" ;;
  *) echo "用法：$0 [release|dev]"; exit 2 ;;
esac

swift build -c "$CONFIG"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$(swift build -c "$CONFIG" --show-bin-path)/AwayLock" "$APP/Contents/MacOS/AwayLock"
cp Resources/Info.plist "$APP/Contents/Info.plist"
[ -f Resources/AppIcon.icns ] && cp Resources/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

# 開發版使用不同的 bundle id 與名稱，避免和發布版的設定、開機啟動項目混在一起
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $BUNDLE_ID" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName $NAME" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $NAME" "$APP/Contents/Info.plist"

codesign --force --sign - "$APP"
echo "完成（$MODE）：$PWD/$APP"
