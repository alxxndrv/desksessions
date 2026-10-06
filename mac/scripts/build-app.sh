#!/usr/bin/env bash
# Builds DeskSessions.app into mac/build/, ad-hoc signed for this Mac.
#   scripts/build-app.sh            build
#   scripts/build-app.sh --install  also copy to ~/Applications and (re)launch it
set -euo pipefail
cd "$(dirname "$0")/.."

: "${DEVELOPER_DIR:=$(ls -d /Applications/Xcode*.app 2>/dev/null | grep -v Beta | sort | tail -1)/Contents/Developer}"
export DEVELOPER_DIR

swift build -c release --product DeskSessions
swift build -c release --product desksessions-hook
BIN=.build/release

APP=build/DeskSessions.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN/DeskSessions" "$BIN/desksessions-hook" "$APP/Contents/MacOS/"

VERSION=$(git describe --always --dirty 2>/dev/null || echo dev)
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.desksessions.app</string>
  <key>CFBundleName</key><string>DeskSessions</string>
  <key>CFBundleDisplayName</key><string>DeskSessions</string>
  <key>CFBundleExecutable</key><string>DeskSessions</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>26.0</string>
  <key>LSUIElement</key><true/>
  <key>NSBluetoothAlwaysUsageDescription</key>
  <string>DeskSessions talks to your desk clock over Bluetooth to show your Claude Code and Codex sessions.</string>
</dict>
</plist>
PLIST

codesign --force --sign - "$APP/Contents/MacOS/desksessions-hook"
codesign --force --sign - --identifier dev.desksessions.app "$APP"
echo "built $APP"

if [[ "${1:-}" == "--install" ]]; then
  mkdir -p ~/Applications
  pkill -x DeskSessions 2>/dev/null && sleep 1 || true
  rm -rf ~/Applications/DeskSessions.app
  cp -R "$APP" ~/Applications/
  open ~/Applications/DeskSessions.app
  echo "installed ~/Applications/DeskSessions.app"
fi
