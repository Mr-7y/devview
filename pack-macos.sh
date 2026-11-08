#!/usr/bin/env bash
# DevView — macOS 打包成 .app
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release

APP="dist/DevView.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp target/release/devview "$APP/Contents/MacOS/DevView"
chmod +x "$APP/Contents/MacOS/DevView"

if [ -f icons/icon.icns ]; then
  cp icons/icon.icns "$APP/Contents/Resources/icon.icns"
fi

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>DevView</string>
    <key>CFBundleDisplayName</key><string>DevView</string>
    <key>CFBundleIdentifier</key><string>com.devview.app</string>
    <key>CFBundleVersion</key><string>1.1.0</string>
    <key>CFBundleShortVersionString</key><string>1.1.0</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleExecutable</key><string>DevView</string>
    <key>CFBundleIconFile</key><string>icon.icns</string>
    <key>LSMinimumSystemVersion</key><string>10.15</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

# 本机使用做一次 ad-hoc 签名，避免「已损坏」提示
codesign --force --deep --sign - "$APP" 2>/dev/null || true

printf '\n产物: %s\n' "$APP"
printf '拖动到「应用程序」即可安装；分发给他人需要 Apple 开发者证书签名+公证。\n'
