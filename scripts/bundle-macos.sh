#!/bin/bash
# 打包 macOS（Apple silicon）：Simple Live.app + dmg
# 依赖：Homebrew 的 mpv（提供 libmpv）和 dylibbundler（把 libmpv 及其依赖拷进包内并改写加载路径）
# 用法：scripts/bundle-macos.sh [版本号]   产物在 dist/
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' app/Cargo.toml | cut -d'"' -f2)}"
VERSION="${VERSION#v}"
APP="dist/Simple Live.app"
DMG="dist/SimpleLive-macOS-arm64-v${VERSION}.dmg"

cargo build --release -p simple_live_gpui

rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp target/release/simple_live_gpui "$APP/Contents/MacOS/"
cp app/assets/AppIcon.icns "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Simple Live</string>
  <key>CFBundleDisplayName</key><string>Simple Live</string>
  <key>CFBundleIdentifier</key><string>io.github.feb17.simplelive</string>
  <key>CFBundleExecutable</key><string>simple_live_gpui</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# 把 libmpv 及其全部依赖拷进 Frameworks，并把加载路径改成包内相对路径
# -ns：不让它逐个签名（带扩展属性的文件会签名失败），下面统一处理
dylibbundler -od -b -cd -ns -x "$APP/Contents/MacOS/simple_live_gpui" -d "$APP/Contents/Frameworks" -p @executable_path/../Frameworks/

# dylibbundler 会重复写入同一条 LC_RPATH，新版 dyld 遇到重复 rpath 直接拒绝加载，删到只剩一条
rpaths() { otool -l "$1" | awk '/cmd LC_RPATH/{getline; getline; print $2}'; }
for f in "$APP/Contents/MacOS/simple_live_gpui" "$APP"/Contents/Frameworks/*.dylib; do
  for rp in $(rpaths "$f" | sort | uniq -d); do
    while [ "$(rpaths "$f" | grep -cxF "$rp")" -gt 1 ]; do install_name_tool -delete_rpath "$rp" "$f"; done
  done
done

# 改写过的二进制签名已失效；Apple silicon 上未签名的代码无法运行。
# 先清掉扩展属性（否则 codesign 报 “resource fork … not allowed”），再统一做 ad-hoc 签名
xattr -cr "$APP"
codesign --force --deep --sign - "$APP"

hdiutil create -volname "Simple Live" -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "已生成：$DMG"
