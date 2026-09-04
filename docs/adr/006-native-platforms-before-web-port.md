# ADR-006: Native Linux and Windows Before an Interactive Web Port

**Status:** Proposed planning direction
**Date:** 2026-08-27
**Author:** Gleb Kalinin with Codex assessment support

## Context

Cull is a local-first Tauri 2 application whose Svelte frontend depends heavily
on the Rust backend, local SQLite database, arbitrary filesystem access, image
decoding, thumbnails, ONNX inference, watchers, keychain, and desktop
integration. The assessment compared an interactive browser version with proper
Linux and Windows distributions.

The current frontend has approximately 231 Tauri invocations in `src/lib/api.ts`
and 39 production frontend files importing Tauri packages. The current release
workflow builds and verifies Apple Silicon artifacts only. Windows compilation
is blocked by unconditional Unix-socket MCP code; Linux can retain that
transport but still needs packaging, dependency, and desktop-integration work.

## Decision

Prioritize native Linux x86_64 and Windows x86_64 distributions before building
an interactive web product. Treat web as a separate product investment justified
by browser sharing or cross-device access, not as a shortcut to Windows support.

Recommended sequence:

1. Polish the existing read-only browser preview/static-publishing path.
2. Ship a Linux x86_64 preview as AppImage and `.deb`, then harden it and add
   Fedora testing/`.rpm` when stable.
3. Abstract MCP transport and ship a Windows x86_64 NSIS preview, then add
   signing, updater, and clean-machine verification.
4. Consider an interactive web client only after validating demand. If pursued,
   prefer a local Rust daemon plus browser UI so Cull can retain SQLite,
   filesystem, thumbnail, sidecar, and local-ML services.

## Resource Ranges

Ranges assume one senior engineer using coding agents, with part-time human QA.
Agent tokens are aggregate input/output budgets and have approximately 50%
uncertainty.

| Deliverable | Agent tokens | Engineering effort | Agent coding days | Calendar time |
| --- | ---: | ---: | ---: | ---: |
| Read-only browser publishing | 50k-200k | 0.5-2 days | under 1 | under 1 week |
| Proper Linux x86_64 release | 4-10M | 20-35 days | 6-10 | 4-7 weeks |
| Proper signed Windows x86_64 release | 4-10M | 20-35 days | 8-14 | 4-8 weeks |
| Linux + Windows combined | 8-18M | 40-70 days | 18-32 including shared release work | 8-14 weeks solo |
| Local-daemon interactive web MVP | 4-10M | 15-30 days | not separately calibrated | 4-8 weeks |
| Broad-parity local web product | 12-30M | 40-80 days | not separately calibrated | 3-5 months |
| Hosted web MVP | 10-25M | 30-60 days | not separately calibrated | 2-4 months |

A reasonable working budget for the combined native effort is 24 agent coding
days, 8-18M tokens, and about six calendar weeks with prompt human review and
parallel work. External certificate procurement and platform-store review are
not included.

## Key Constraints

- **Web:** browsers cannot directly reuse Cull's arbitrary path access, existing
  `cull.db`, recursive watchers, native decoders, Rust ONNX/PDFium stack, trash,
  clipboard, keychain, tray, file associations, or Tauri asset URLs. The E2E
  mock is test infrastructure, not a production backend.
- **Windows:** replace or conditionally disable Unix-socket MCP; implement or
  explicitly scope Recycle Bin/undo, clipboard, Open With, path handling,
  PDFium/ONNX DLLs, title bar, Node/Claude runtime resolution, NSIS signing,
  updater metadata, and Windows 10/11 install/upgrade tests.
- **Linux:** validate WebKitGTK/appindicator dependencies, Secret Service,
  AppImage/`.deb` and later `.rpm`, GNOME/KDE and Wayland/X11 behavior,
  freedesktop trash, clipboard/Open With, non-macOS decoding, PDFium, ONNX, and
  the updater/verifier path. Defer ARM64, Flatpak, and Snap initially.
- **Release maturity:** a successful compile or installer is a preview, not a
  supported distribution. Proper support includes signing where applicable,
  updater artifacts, provenance/checksums, clean install/uninstall/upgrade, and
  platform smoke tests.

## Provenance

- Completed Codex evaluation task: `01a0444f-5c29-7370-8928-26fa0ada3560`
- Assessment date: 2026-08-27, Europe/Berlin
- Evidence basis: read-only inspection of Cull's frontend API, Rust platform
  branches, Tauri configuration, release workflows, packaging documentation,
  and official Tauri 2 distribution prerequisites. No implementation was
  performed as part of the evaluation.
- Primary repository context: `docs/cross-platform-distribution.md`,
  `src/lib/api.ts`, `src-tauri/src/mcp/socket.rs`, `src-tauri/src/lib.rs`,
  `src-tauri/tauri.conf.json`, `.github/workflows/ci.yml`,
  `.github/workflows/release.yml`, and `release.config.json`.

## Consequences

- Windows users should be served by the native Tauri application rather than a
  premature web rewrite.
- Linux is the lower-risk first portability lane, but feature parity and release
  verification remain explicit work.
- A future web product can reuse the Svelte presentation layer and much of the
  Rust service logic only if it introduces a deliberate transport boundary.
- Product-specific macOS features may remain unavailable on other platforms if
  limitations are made explicit and core culling workflows remain reliable.
