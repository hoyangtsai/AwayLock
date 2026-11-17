#!/bin/bash
# 把發布版 App 打包成 Homebrew Cask 可用的 zip，並產生 cask 檔。
# 用法：scripts/package-macos.sh
# 產出（都在 dist/，不進版控）：
#   awaylock-<版本>-<架構>.zip   上傳到 GitHub Release（tag：v<版本>）
#   awaylock.rb                  複製到 tap 倉庫的 Casks/awaylock.rb
set -euo pipefail
cd "$(dirname "$0")/.."

scripts/bundle-macos.sh release

APP="dist/awaylock.app"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
ARCH="$(uname -m)"
ZIP="dist/awaylock-$VERSION-$ARCH.zip"

rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
SHA256="$(shasum -a 256 "$ZIP" | awk '{print $1}')"

sed -e "s/@VERSION@/$VERSION/" -e "s/@SHA256@/$SHA256/" \
  packaging/homebrew/awaylock.rb.in > dist/awaylock.rb

echo
echo "版本：$VERSION  架構：$ARCH"
echo "壓縮檔：$ZIP"
echo "sha256：$SHA256"
echo "Cask：dist/awaylock.rb"
[ "$ARCH" = "arm64" ] || echo "注意：cask 範本寫死 arm64，目前架構是 $ARCH，請調整 url 與 depends_on。"
