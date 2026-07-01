# Changelog

All notable changes to **ai-usagebar-macos** are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Release pages: <https://github.com/thenets/ai-usagebar-macos/releases>.

## [Unreleased]

### Added
- **Native macOS menu bar app** (`macos/AIUsageBar.swift`) built with SwiftUI
  `MenuBarExtra` + `Gauge` (`.accessoryLinearCapacity`). Replaces the previous
  Unicode text-bar renderer with genuine native controls.
- Menu bar label shows the **5-hour usage percentage and time-to-reset**
  (e.g. `42% · 3h 51m`).
- Dropdown with per-window gauges (Session / Weekly / Sonnet / Extra) tinted by
  severity, plus **Refresh now**, **Open TUI**, **Open at Login**, and **Quit**.
- **Open at Login** via `SMAppService` (native macOS Login Item).
- `macos/bundle.sh` — packages a self-contained `AI Usage Bar.app` (generated
  icon, `Info.plist`, `LSUIElement`, ad-hoc code-signed) with no Xcode project.

### Changed
- Project renamed to **ai-usagebar-macos** (`org.thenets.ai-usagebar-macos`) and
  refocused as a **macOS-only, self-contained** project: the cross-platform Rust
  usage engine stays in-repo and builds locally (`cargo install --path .`).
- Removed all Linux-specific content (GNOME Shell extension, Waybar/Hyprland/
  Omarchy docs, Arch AUR packaging, Linux release CI).

### Attribution
- The Rust usage engine is adapted from
  [`ai-usagebar`](https://github.com/akitaonrails/ai-usagebar) by Fabio Akita
  (AkitaOnRails), itself a port of
  [`claudebar`](https://github.com/mryll/claudebar) / `codexbar` by mryll.

[Unreleased]: https://github.com/thenets/ai-usagebar-macos/commits/main
