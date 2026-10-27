#!/bin/bash
# 打包 macOS（Apple silicon）：Livo.app + dmg
# libmpv 用 media-kit 的精简版（原项目 macOS 也用它）：只编了直播用到的解码器和协议，
# 比 Homebrew 的 mpv 连同依赖小一半多（包内库 58MB → 20MB）。首次运行会下载并校验
# 用法：scripts/bundle-macos.sh [版本号]   产物在 dist/
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' app/Cargo.toml | cut -d'"' -f2)}"
VERSION="${VERSION#v}"
APP="dist/Livo.app"
DMG="dist/Livo-macOS-arm64-v${VERSION}.dmg"

LIBMPV_VER=v0.7.3
LIBMPV_PKG="libmpv-libs_${LIBMPV_VER}_macos-arm64-video-default"
LIBMPV_SHA256=9137a736c97dd8429a51b2dbbf061b69e879b0b69dc88b9d3a200d2354166af5
LIBMPV_DIR="$PWD/target/libmpv/$LIBMPV_PKG"
if [ ! -d "$LIBMPV_DIR" ]; then
  mkdir -p target/libmpv
  curl -fsSL -o "target/libmpv/$LIBMPV_PKG.tar.gz" "https://github.com/media-kit/libmpv-darwin-build/releases/download/$LIBMPV_VER/$LIBMPV_PKG.tar.gz"
  echo "$LIBMPV_SHA256  target/libmpv/$LIBMPV_PKG.tar.gz" | shasum -a 256 -c -
  # 先解压到临时目录，完整解压后才改名成正式目录；中途中断不会留下残缺目录，下次会重新下载校验
  tmp=$(mktemp -d target/libmpv/.extract.XXXXXX)
  tar -xzf "target/libmpv/$LIBMPV_PKG.tar.gz" -C "$tmp"
  mv "$tmp/$LIBMPV_PKG" "$LIBMPV_DIR"
  rmdir "$tmp"
fi

MPV_LIB_DIR="$LIBMPV_DIR" cargo build --release -p livo

rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp target/release/livo "$APP/Contents/MacOS/"
cp app/assets/AppIcon.icns "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Livo</string>
  <key>CFBundleDisplayName</key><string>Livo</string>
  <key>CFBundleIdentifier</key><string>io.github.feb17.livo</string>
  <key>CFBundleExecutable</key><string>livo</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# 这套库之间都用 @rpath 互相引用，拷进 Frameworks 再给主程序加一条 rpath 即可
cp "$LIBMPV_DIR"/*.dylib "$APP/Contents/Frameworks/"
install_name_tool -add_rpath @executable_path/../Frameworks "$APP/Contents/MacOS/livo"

# 加 rpath 后二进制签名已失效；Apple silicon 上未签名的代码无法运行。
# 先清掉扩展属性（否则 codesign 报 “resource fork … not allowed”），再统一做 ad-hoc 签名
xattr -cr "$APP"
codesign --force --deep --sign - "$APP"

hdiutil create -volname "Livo" -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "已生成：$DMG"
