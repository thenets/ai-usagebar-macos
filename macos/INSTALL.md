# Installing the AI Usage Bar menu bar app (macOS)

A step-by-step guide to get your Claude Code usage into the macOS menu bar. For
configuration and how it works, see [README.md](README.md).

## Prerequisites

| Need | How |
|---|---|
| **macOS 13+ (Ventura)** | `MenuBarExtra` + `Gauge` are macOS 13.0 APIs |
| **Command Line Tools** (for `swiftc`) | `xcode-select --install` |
| **Rust toolchain** (to build the engine) | [rustup.rs](https://rustup.rs) |
| **Claude logged in once** | run `claude` once — its OAuth creds go to the login **Keychain**, which the engine reads automatically |

## Step by step

### 1. Get the code

```bash
git clone git@github.com:thenets/ai-usagebar-macos.git
cd ai-usagebar-macos
# (already cloned? just `git pull`)
```

### 2. Build & install the usage engine (self-contained, from this repo)

```bash
cargo install --path .                     # installs ai-usagebar + ai-usagebar-tui to ~/.cargo/bin
ai-usagebar --vendor anthropic --pretty    # quick smoke test — should print your usage
```

### 3. Log in to Claude once (if you haven't)

```bash
claude        # authenticates; creds land in the login Keychain
```

### 4. Build the app

```bash
cd macos
./build.sh    # runs: swiftc -O -parse-as-library AIUsageBar.swift -o ai-usagebar-menubar
```

### 5. Run it

```bash
./ai-usagebar-menubar &
```

It appears in the menu bar next to the clock (no Dock icon), showing the 5-hour
percentage and time-to-reset (e.g. `42% · 3h 51m`). Click it for the per-window
dropdown (Session / Weekly / Sonnet / Extra).

### 6. Start automatically at login

Two options:

- **From the app** — build the `.app` bundle and use the **Open at Login** item
  in the dropdown (native Login Item via `SMAppService`):

  ```bash
  ./bundle.sh
  cp -R "AI Usage Bar.app" /Applications/
  open "/Applications/AI Usage Bar.app"     # then click "Open at Login"
  ```

- **LaunchAgent** — for the raw binary:

  ```bash
  ./install-agent.sh
  ```

  This installs a LaunchAgent at
  `~/Library/LaunchAgents/org.thenets.ai-usagebar-macos.plist` with `RunAtLoad`.

### 7. Verify it's running

```bash
launchctl list | grep ai-usagebar          # shows the agent (if using LaunchAgent)
pgrep -lf ai-usagebar-menubar               # shows the process
```

## Managing it

**Update to a newer version**

```bash
git pull
cargo install --path .                      # rebuild the engine if it changed
cd macos && ./build.sh                      # rebuild the app
# restart it (LaunchAgent users):
launchctl unload ~/Library/LaunchAgents/org.thenets.ai-usagebar-macos.plist
launchctl load   ~/Library/LaunchAgents/org.thenets.ai-usagebar-macos.plist
```

**Stop / uninstall the auto-start**

```bash
launchctl unload ~/Library/LaunchAgents/org.thenets.ai-usagebar-macos.plist
rm ~/Library/LaunchAgents/org.thenets.ai-usagebar-macos.plist
```

**Change settings** — settings persist in `UserDefaults` and apply live:

```bash
defaults write org.thenets.ai-usagebar-macos showExtra -bool true
defaults write org.thenets.ai-usagebar-macos interval  -int  60
```

See the settings table in [README.md](README.md#configuration).

## Troubleshooting

| Symptom | Fix |
|---|---|
| `swiftc: command not found` | `xcode-select --install`, then re-run `./build.sh` |
| Menu bar shows `⚠` | the engine wasn't found — `cargo install --path ..` from the repo root, or set `binaryPath`; it's searched in `~/.cargo/bin`, `/opt/homebrew/bin`, `/usr/local/bin`, then `PATH` |
| Menu bar shows `…` and never updates | run `claude` once so creds exist; test with `ai-usagebar --vendor anthropic --pretty` |
| macOS blocks the app (Gatekeeper) | it's your own local build — launching from Terminal / LaunchAgent is fine; if Finder blocks it, right-click → **Open** once |
| `'main' attribute cannot be used…` at build | you dropped the `-parse-as-library` flag; use `./build.sh`, which passes it |
| Gauge colors look off | tune the severity colors via `defaults write` (they apply live) |
