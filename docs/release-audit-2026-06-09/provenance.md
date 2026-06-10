# Build Provenance — Cull 0.2.1 (local dev build, 2026-06-10)

**This is a LOCAL ad-hoc-signed build for verification, NOT the public release artifact.**
The public DMG must be produced by the signed + notarized GitHub Release pipeline (bd imageview-dkz.2 / HYG-001).

## Toolchain
- commit: a3e07727ede76467302600bab27ce0d5e57110c9
- rustc: rustc 1.89.0 (29483883e 2025-08-04)
- node: v22.22.3
- tauri-cli: tauri-cli 2.11.2
- command: npm run tauri build
- target: aarch64-apple-darwin

## Artifacts
- Cull.app (ad-hoc signed — Signature=adhoc, no Developer ID)
- Cull_0.2.1_aarch64.dmg

## Trust chain (local build — expected failures until CI signing)
- codesign --verify --deep --strict: ad-hoc (no Developer ID resources)
- spctl --assess --type execute: FAIL (not notarized locally)
- xcrun stapler validate: no ticket (notarization is a CI/Release step)

## Checksums (SHA-256)
```
41e8b0192e883797ac336cb89707d496cc3414dc0da98993d7ff3f70fa53ca37  src-tauri/target/release/bundle/dmg/Cull_0.2.1_aarch64.dmg
```

On a properly signed CI build, all three trust-chain commands must pass before the DMG is published publicly.
