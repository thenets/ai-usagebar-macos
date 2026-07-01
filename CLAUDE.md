# CLAUDE.md

Notes for Claude Code (and humans) working in **ai-usagebar-macos**. Keep tight:
these are invariants and layout, not a project tour.

This is a **macOS-only, self-contained** project: a native SwiftUI menu bar app
(`macos/`) over an in-repo cross-platform Rust **usage engine** (`src/`). The
engine is adapted from [`ai-usagebar`](https://github.com/akitaonrails/ai-usagebar)
by Fabio Akita — keep that attribution intact (README Acknowledgements, LICENSE).

## Build & run

```bash
cargo install --path .                 # build + install the engine (ai-usagebar, ai-usagebar-tui)
cd macos && ./build.sh                 # build the native app  → ai-usagebar-menubar
cd macos && ./bundle.sh                # package "AI Usage Bar.app" (icon, Info.plist, ad-hoc signed)
```

- The Swift app is a **single file** (`macos/AIUsageBar.swift`) compiled with
  `swiftc -O -parse-as-library` (the flag is required for a single-file SwiftUI
  `@main`; without it, `swiftc` errors `'main' attribute cannot be used…`).
- The bundle id is **`org.thenets.ai-usagebar-macos`** — it must match in
  `bundle.sh` (Info.plist) and `install-agent.sh` (LaunchAgent label), and is
  what `SMAppService` (Open at Login) and `defaults write` key off.

## Hard invariants — never break these

- **The engine always exits 0** and emits a fallback `⚠` JSON on error — the
  Swift app parses `{text}` and shows the raw text when it can't parse. See
  `widget::run::fallback`.
- **The macOS app resolves the engine by the name `ai-usagebar`** (and
  `ai-usagebar-tui`). Do **not** rename the Rust crate/binaries — `resolveBinary`
  in `AIUsageBar.swift` and every doc depend on those exact names.
- **Cache writes are atomic** (tempfile + persist), with per-vendor `flock`.
- **No secrets in tracked files.** `.gitignore` covers `.env`,
  `*.credentials.json`, and `.claude/`. Never commit a real API key/token.
- **Tests are hermetic.** A `#[test]` must never read/write a real
  `$HOME`/`$XDG` path or branch on an ambient env var — inject via the test seam
  (`Cache::at`, `creds::read_from`, `App::with_theme`, …). Live API tests stay
  behind `#[ignore]` (see `tests/live.rs`).

## Secret-discipline rules

- **Never `cat` a config/credential file** that could hold `api_key`/`token`/
  OAuth blobs. Use `jq 'keys'` for structure.
- **Never `env | grep …`** without a tight filter. Prefer `printenv VAR`.
- OAuth files (`~/.claude/.credentials.json`, `~/.codex/auth.json`): `jq 'keys'`
  only.

## What lives where

- `macos/AIUsageBar.swift` — the native SwiftUI menu bar app (MenuBarExtra +
  Gauge, Settings, SMAppService Login Item). Shells out to the engine.
- `macos/build.sh` / `bundle.sh` / `install-agent.sh` — build the binary, the
  `.app` bundle, and the login LaunchAgent.
- `src/active.rs` — scroll-cycle active-vendor state file.
- `src/anthropic/`, `src/openai/`, `src/openrouter/`, `src/zai/`,
  `src/deepseek/` — per-vendor types + fetch + render.
- `src/anthropic/keychain.rs` — macOS `security(1)` fallback when
  `~/.claude/.credentials.json` is absent (Claude Code stores the OAuth blob in
  the login Keychain). `#[cfg(target_os = "macos")]`.
- `src/cache.rs` — atomic per-vendor cache writes + flock, plus cross-platform
  path resolvers (`xdg_cache_dir`, `home_dir`).
- `src/tui/` — the `ai-usagebar-tui` interactive terminal app (ratatui).
- `src/widget/` — the CLI/JSON output the Swift app consumes (`{text, tooltip,
  class}`).
- `src/tooltip.rs` — shared bordered-box tooltip renderer.
- `tests/anthropic_e2e.rs` — mockito + insta snapshot tests.
- `tests/live.rs` — `#[ignore]`d smoke tests against real APIs (`make smoke`).

## Live API smoke discipline

`make smoke` exercises real undocumented endpoints (Anthropic OAuth, OpenAI
Codex OAuth, Z.AI monitor). If it fails after a vendor's response shape drifts:
capture the actual response, update the matching `types.rs` in
`src/{anthropic,openai,zai,openrouter,deepseek}/`, and re-run until green.

## Gate before committing

```bash
cargo test                                  # unit + integration
cargo clippy --all-targets -- -D warnings   # clean
cd macos && ./build.sh                      # the app still compiles
```
