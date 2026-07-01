#!/usr/bin/env bash
# Package the menu bar app into a proper "AI Usage Bar.app" bundle — no Xcode.
# The bundle can be dragged to /Applications and toggled from the app's own
# "Open at Login" item (SMAppService) or System Settings › Login Items.
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

APP_NAME="AI Usage Bar"
BUNDLE_ID="org.thenets.ai-usagebar-macos"
EXEC="ai-usagebar-menubar"
VERSION="1.0.0"
APP="$DIR/$APP_NAME.app"

# 1. Build the executable.
"$DIR/build.sh"

# 2. Fresh bundle skeleton.
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$DIR/$EXEC" "$APP/Contents/MacOS/$EXEC"

# 3. Render an app icon from an SF Symbol (rounded card + tinted gauge glyph),
#    then pack it into AppIcon.icns via iconutil. Pure AppKit, no assets.
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
GEN="$(mktemp -d)/mkicon.swift"
cat > "$GEN" <<'SWIFT'
import AppKit

let outDir = CommandLine.arguments[1]
// (filename, pixel size) — the standard macOS iconset matrix.
let variants: [(String, Int)] = [
    ("icon_16x16", 16), ("icon_16x16@2x", 32),
    ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256),
    ("icon_256x256", 256), ("icon_256x256@2x", 512),
    ("icon_512x512", 512), ("icon_512x512@2x", 1024),
]
let green = NSColor(srgbRed: 0.596, green: 0.765, blue: 0.475, alpha: 1) // #98c379

func tinted(_ symbol: NSImage, _ color: NSColor) -> NSImage {
    let out = NSImage(size: symbol.size)
    out.lockFocus()
    color.set()
    let r = NSRect(origin: .zero, size: symbol.size)
    r.fill()
    symbol.draw(in: r, from: .zero, operation: .destinationIn, fraction: 1)
    out.unlockFocus()
    return out
}

func makePNG(_ px: Int) -> Data? {
    let side = CGFloat(px)
    let img = NSImage(size: NSSize(width: side, height: side))
    img.lockFocus()
    // Rounded "card" background with a subtle vertical gradient.
    let inset = side * 0.06
    let rect = NSRect(x: inset, y: inset, width: side - 2 * inset, height: side - 2 * inset)
    let path = NSBezierPath(roundedRect: rect, xRadius: side * 0.22, yRadius: side * 0.22)
    let grad = NSGradient(colors: [
        NSColor(srgbRed: 0.17, green: 0.19, blue: 0.23, alpha: 1),
        NSColor(srgbRed: 0.10, green: 0.11, blue: 0.13, alpha: 1),
    ])!
    grad.draw(in: path, angle: -90)
    // Centered, tinted gauge glyph.
    let cfg = NSImage.SymbolConfiguration(pointSize: side * 0.52, weight: .semibold)
    if let base = NSImage(systemSymbolName: "gauge.with.dots.needle.50percent",
                          accessibilityDescription: nil)?.withSymbolConfiguration(cfg) {
        let glyph = tinted(base, green)
        let gs = glyph.size
        glyph.draw(in: NSRect(x: (side - gs.width) / 2, y: (side - gs.height) / 2,
                              width: gs.width, height: gs.height))
    }
    img.unlockFocus()
    guard let tiff = img.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) else { return nil }
    return rep.representation(using: .png, properties: [:])
}

for (name, px) in variants {
    guard let data = makePNG(px) else { continue }
    try? data.write(to: URL(fileURLWithPath: "\(outDir)/\(name).png"))
}
SWIFT

if swiftc -O "$GEN" -o "$GEN.bin" 2>/dev/null && "$GEN.bin" "$ICONSET" \
   && iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns" 2>/dev/null; then
    ICON_KEY='<key>CFBundleIconFile</key><string>AppIcon</string>'
    echo "› Icon generated"
else
    ICON_KEY=''
    echo "! Icon generation skipped (bundle still works, just no custom icon)"
fi

# 4. Info.plist — LSUIElement makes it a menu-bar agent (no Dock icon), matching
#    setActivationPolicy(.accessory).
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>$EXEC</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    $ICON_KEY
</dict>
</plist>
EOF

# 5. PkgInfo + ad-hoc code signature (required for SMAppService login items).
printf 'APPL????' > "$APP/Contents/PkgInfo"
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 \
    && echo "› Ad-hoc signed" || echo "! codesign unavailable — Login Items may not persist"

echo "✓ Built: $APP"
echo
echo "Run now:            open \"$APP\""
echo "Install:            cp -R \"$APP\" /Applications/   # then 'Open at Login' from the dropdown"
