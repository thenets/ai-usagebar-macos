# AI Usage Bar — macOS menu bar app

The native macOS menu bar app of
[`ai-usagebar-macos`](../README.md). It shows your **Claude Code** 5-hour usage
next to the clock — the percentage and time-to-reset — with a native SwiftUI
dropdown of per-window gauges.

Built with **native macOS UI components**: SwiftUI
[`MenuBarExtra`](https://developer.apple.com/documentation/swiftui/menubarextra)
with a `.window` popover, and per-window
[`Gauge`](https://developer.apple.com/documentation/swiftui/gauge)s in the
`.accessoryLinearCapacity` style — real system controls, not text bars. A single
Swift file (`AIUsageBar.swift`); no Xcode project.

> **Installing?** Follow the step-by-step in **[INSTALL.md](INSTALL.md)**.

## Requirements

- **macOS 13+ (Ventura)** — `MenuBarExtra` and `Gauge` are macOS 13.0 APIs.
- **Command Line Tools** (`xcode-select --install`) for `swiftc`.
- The `ai-usagebar` engine binary on the Mac. Build it from this repo:
  `cargo install --path ..` (lands in `~/.cargo/bin`) — see the
  [main README](../README.md).
- Run `claude` once so its OAuth creds are in the login **Keychain**; the engine
  reads them there automatically (no env vars).

## Build & run

```bash
cd macos
./build.sh                 # swiftc -O -parse-as-library AIUsageBar.swift → ./ai-usagebar-menubar
./ai-usagebar-menubar &    # appears in the menu bar (no Dock icon)
```

### As a proper `.app` bundle

```bash
./bundle.sh                # → "AI Usage Bar.app" (generated icon + Info.plist, ad-hoc signed)
open "AI Usage Bar.app"
cp -R "AI Usage Bar.app" /Applications/    # optional — then toggle "Open at Login"
```

Start at login: use the **Open at Login** item in the dropdown (native
`SMAppService` Login Item — works from the `.app` bundle), or the LaunchAgent:

```bash
./install-agent.sh         # installs a LaunchAgent (RunAtLoad)
```

> Not code-signed with a Developer ID. It's a local binary you built yourself,
> so Gatekeeper doesn't block it when launched from the terminal / LaunchAgent.
> If macOS ever complains, right-click the app in Finder → **Open** once.

## Configuration

Settings persist in `UserDefaults` under `org.thenets.ai-usagebar-macos` and
apply **live** — gauges re-tint and windows show/hide the moment you change a
setting:

```bash
defaults write org.thenets.ai-usagebar-macos showExtra -bool true
defaults write org.thenets.ai-usagebar-macos interval  -int  60
```

| Key | Default | Notes |
|---|---|---|
| `showSession` / `showWeekly` / `showSonnet` / `showExtra` | on / on / on / off | which windows appear as gauges |
| `colorLow` / `colorMid` / `colorHigh` / `colorCritical` | One Dark | gauge tint per severity (≥90 / ≥75 / ≥50 / else) |
| `interval` | 30 | refresh seconds (5–3600) |
| `vendor` | anthropic | only Anthropic has the 5h + weekly windows |
| `binaryPath` | auto | empty = `~/.cargo/bin`, Homebrew, then `PATH` |

The menu-bar label is a system SF Symbol + text, so it adapts to a light or dark
menu bar automatically; the configurable colors tint the gauge fills in the
dropdown.

## How it works

Runs `ai-usagebar --vendor <v> --format '{plan};;{session_pct};;…'`, parses the
JSON (`{text, …}`), and renders each usage window as a native SwiftUI `Gauge`
(`.accessoryLinearCapacity`) tinted by severity. The subprocess runs **off the
main thread** (`DispatchQueue.global` → back to `@MainActor` for UI), so the UI
never blocks. The app is a `MenuBarExtra` scene with
`.setActivationPolicy(.accessory)` — a menu-bar agent with no Dock icon.
