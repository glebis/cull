# Tauri WebDriver E2E Testing: Evaluation

**Date:** 2026-05-24

## Projects Evaluated

### 1. Choochmeque/tauri-plugin-webdriver
- **Repo:** https://github.com/Choochmeque/tauri-plugin-webdriver
- **Crate:** Not published under that exact name; related crate `tauri-plugin-webdriver-automation` exists (see below)
- **Approach:** Embeds a W3C WebDriver server (port 4445) directly inside the Tauri app as a plugin. Single-crate, cross-platform (macOS/Windows/Linux).
- **Tauri 2:** Plugin version 0.2 listed; author also maintains Tauri v2 plugins (sharekit, iap, notifications), strongly suggesting Tauri 2 compatibility.
- **Status:** Active as of Feb 2026. Low community adoption (few stars/downloads).

### 2. danielraffel/tauri-webdriver
- **Repo:** https://github.com/danielraffel/tauri-webdriver
- **Blog:** https://danielraffel.me/2026/02/14/i-built-a-webdriver-for-wkwebview-tauri-apps-on-macos/
- **Crate:** `tauri-plugin-webdriver-automation` v0.1.3 on crates.io (~2,344 downloads)
- **Approach:** Two components: a Tauri plugin (debug builds only) that starts an HTTP server + JS bridge inside WKWebView, plus a CLI binary (`tauri-wd`) on port 4444. Covers element finding, clicks, screenshots, actions, cookies, iframes, shadow DOM, alerts, print-to-PDF, computed ARIA.
- **Tauri 2:** Built for Tauri 2 on macOS. macOS-only by design (fills the WKWebView gap).
- **Bonus:** Includes MCP integration for AI agent testing.
- **Status:** Published Feb 2026. Solo-maintainer project, built with Claude Code.

## Cull App Context

Cull uses `tauri = "2"` with protocol-asset and tray-icon features. macOS-only development. Current E2E is browser-only against localhost:1420 with Tauri mock layer.

## Integration (either project)

```toml
# Cargo.toml — add under [dependencies]
tauri-plugin-webdriver-automation = "0.1.3"  # Raffel
# or via git for Choochmeque
tauri-plugin-webdriver = { git = "https://github.com/Choochmeque/tauri-plugin-webdriver" }
```

```rust
// main.rs — register plugin (debug only)
#[cfg(debug_assertions)]
app.plugin(tauri_plugin_webdriver::init())?;
```

Test scripts would use WebdriverIO or Selenium against the local WebDriver port.

## Dealbreakers

- **Both are solo-maintainer, low-adoption projects** (<2,500 downloads combined). Risk of abandonment.
- **Neither tests Tauri-native features** (tray icon, native menus, system dialogs). They only automate the WebView content, same as current browser-based E2E.
- **Choochmeque:** No published crate under that exact name; unclear versioning story.
- **Raffel:** macOS-only (fine for now, blocks future cross-platform CI).

## Recommendation: WAIT

Neither project solves the core problem -- testing Tauri-native features like tray menus, file import dialogs, and system menu items. They only add WebView automation, which our current browser-based E2E with mocks already covers adequately.

**Short-term:** Keep current browser E2E + Tauri mock layer. It is cheaper and more stable than depending on pre-1.0 solo-maintainer crates.

**Revisit when:** (a) official `tauri-driver` gains macOS support, or (b) either project reaches 1.0 with multi-maintainer governance, or (c) we need WebView automation that must run inside the real Tauri shell (e.g., testing protocol-asset URLs or deep-link handling that mocks cannot cover).
