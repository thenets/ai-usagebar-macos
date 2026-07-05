# ai-usagebar-macos

A native **macOS menu bar app** that shows your **Claude Code** (and other AI
plan) usage next to the clock — the 5-hour session window, the weekly window,
and more — rendered with genuine native UI (SwiftUI `MenuBarExtra` + `Gauge`).

The menu bar shows the 5-hour percentage and time-to-reset at a glance:

```
⏲ 42% · 3h 51m
```

Click it for a native dropdown with per-window gauges (Session, Weekly, and
optionally Sonnet and extra-usage $).

This project is **self-contained**: it bundles a small cross-platform Rust
**usage engine** (`ai-usagebar`) that talks to each vendor's API, plus the
native Swift menu bar app that renders it. Everything builds from this repo — no
external services.

> **Attribution.** The usage engine is a lightly-adapted copy of
> **[`ai-usagebar`](https://github.com/akitaonrails/ai-usagebar) by Fabio Akita
> (AkitaOnRails)** — a Rust Waybar widget + TUI for AI plan usage. This project
> keeps his engine as the data source and adds a first-class native macOS menu
> bar frontend. All the vendor/OAuth/endpoint work is his; see
> [Acknowledgements](#acknowledgements). The original engine, in turn, is a port
> of [`claudebar`](https://github.com/mryll/claudebar)/[`codexbar`](https://github.com/mryll/codexbar)
> by mryll.

## What you see

- **Menu bar label** — the 5-hour usage percentage and time left until it
  resets, e.g. `42% · 3h 51m`.
- **Dropdown** — native `Gauge` bars for each window, tinted by severity
  (green → yellow → orange → red at 50/75/90%), with reset countdowns. Plus
  **Refresh now**, **Open TUI**, **Open at Login**, and **Quit**.
- **Open at Login** — a native macOS Login Item toggle via `SMAppService`.

## Requirements

| Need | How |
|---|---|
| **macOS 13+ (Ventura)** | `MenuBarExtra` + `Gauge` are macOS 13.0 APIs |
| **Xcode Command Line Tools** (for `swiftc`) | `xcode-select --install` |
| **Rust toolchain** (to build the engine) | [rustup.rs](https://rustup.rs) |
| **Claude logged in once** | run `claude` once — its OAuth creds go to the login **Keychain**, which the engine reads automatically |

## Install (self-contained)

Everything is built from this repo — no `cargo install` from crates.io, no
prebuilt downloads.

```bash
git clone git@github.com:thenets/ai-usagebar-macos.git
cd ai-usagebar-macos

# 1. Build the usage engine (Rust) and install the two binaries to ~/.cargo/bin.
cargo install --path .          # installs ai-usagebar + ai-usagebar-tui

# 2. Log in to Claude once so creds land in the login Keychain.
claude

# 3. Build the native menu bar app and run it.
cd macos
./build.sh                      # swiftc -O -parse-as-library → ai-usagebar-menubar
./ai-usagebar-menubar &
```

Prefer a real `.app` bundle you can drop in `/Applications` and toggle from
Login Items?

```bash
cd macos
./bundle.sh                     # → "AI Usage Bar.app" (icon + Info.plist, ad-hoc signed)
open "AI Usage Bar.app"
```

See **[macos/INSTALL.md](macos/INSTALL.md)** for the step-by-step, and
**[macos/README.md](macos/README.md)** for how the app works and its settings.

## Configuration

Settings live in `UserDefaults` under the app's bundle id
(`org.thenets.ai-usagebar-macos`) and apply live:

```bash
defaults write org.thenets.ai-usagebar-macos showExtra  -bool true   # show the extra-usage ($) gauge
defaults write org.thenets.ai-usagebar-macos showWeekly -bool true   # weekly gauge in the dropdown
defaults write org.thenets.ai-usagebar-macos interval   -int  60     # refresh seconds (5–3600)
defaults write org.thenets.ai-usagebar-macos vendor -string anthropic
```

| Key | Default | Notes |
|---|---|---|
| `showSession` / `showWeekly` / `showSonnet` / `showExtra` | on / on / on / off | which windows appear as gauges in the dropdown |
| `colorLow` / `colorMid` / `colorHigh` / `colorCritical` | One Dark hexes | gauge tint per severity (≥90 / ≥75 / ≥50 / else) |
| `interval` | 30 | refresh seconds |
| `vendor` | anthropic | only Anthropic has the 5h + weekly windows |
| `binaryPath` | auto | empty = `~/.cargo/bin`, Homebrew, then `PATH` |

## Authentication

The engine reads credentials the official CLIs already wrote — no env vars for
the OAuth vendors.

| Vendor | Method | Action |
|---|---|---|
| **Anthropic (Claude)** | OAuth in the login **Keychain** (service `Claude Code-credentials`), or `~/.claude/.credentials.json` | run `claude` once |
| OpenAI (Codex) | OAuth in `~/.codex/auth.json` | run `codex login` once |
| Z.AI / OpenRouter / DeepSeek | API key (env var or `~/.config/ai-usagebar/config.toml`) | set the key |

> On recent Claude Code builds macOS stores the OAuth blob in the login
> Keychain rather than a file; the engine reads (and writes refreshed tokens
> back to) it via the built-in `security` tool automatically.

## How it works

The Swift app runs `ai-usagebar --vendor <v> --format '{plan};;{session_pct};;…'`
off the main thread, parses the JSON it returns, and renders each usage window
as a native SwiftUI `Gauge`. The engine handles OAuth refresh, the Keychain, and
the (undocumented) vendor endpoints. See [macos/README.md](macos/README.md) for
the rendering details and [CLAUDE.md](CLAUDE.md) for repo layout and invariants.

## Command-line tools

The engine binaries also work standalone on macOS:

```bash
ai-usagebar --vendor anthropic --pretty    # human-readable usage in the terminal
ai-usagebar --vendor anthropic --json      # structured usage JSON
ai-usagebar-tui                            # interactive multi-vendor TUI
```

## Acknowledgements

- **[Fabio Akita / AkitaOnRails](https://github.com/akitaonrails/ai-usagebar)** —
  the `ai-usagebar` Rust usage engine this project is built on (all vendor,
  OAuth, and endpoint work). This macOS app is a native frontend for it.
- **[mryll](https://github.com/mryll/claudebar)** — the original `claudebar` /
  `codexbar`, whose endpoint references, severity colors, and pacing math the
  engine ports.

## License

MIT — see [LICENSE](LICENSE). The original engine is © AkitaOnRails; the macOS
app is © thenets.org.
