# Cull Assets And Licensing Guide

Status: working notes for brand and site assets.

## App Icons

Icon source and generated files live in:

- `design/icons/tahoe/`
- `src-tauri/icons/`
- `src-tauri/icons/variants/`
- `static/icon-variants/`

The active app icons are flat, minimal, and masked for Tahoe-style display. Icon Composer renders are reference-only because Apple's `ictool` adds gradients and Liquid Glass shading that currently conflicts with the Cull identity.

## Image Rights

Yulia Katan artwork may be used for Cull brand/site explorations because the project owner has written permission.

Track provenance for every imported artwork:

- artist name
- source URL
- permission status
- date saved
- intended usage

Do not assume this permission extends to merchandise, paid advertising, model training, or derivative image generation unless the written permission explicitly says so.

## Fonts

Font licensing must be checked before production use.

Current preferred directions:

- EB Garamond or similar open-license editorial serif for large quotes.
- A refined monospace for system language, terminal sections, captions, and UI references.

Before shipping a site or app bundle, confirm:

- the font license allows web embedding
- the font license allows app bundling if included in the desktop app
- attribution requirements are met
- the license is stored or linked in this repo

Prefer OFL or similarly permissive fonts for public source distribution.

## Model Weights

Cull's Apache-2.0 license covers the application source, not third-party model
weights.

Current policy:

- CLIP and DINOv2 embedding downloads must point to model repositories with a
  compatible license recorded in the open-source audit.
- YOLO and NudeNet-compatible detection can load user-supplied local ONNX files,
  but Cull does not ship or automatically download those weights until their
  licenses are verified as compatible with the release goal.
- Any future model downloader must document the model source, license,
  attribution, checksum, and commercial-use terms before it is exposed in the UI
  or CLI.

## Screenshots And Mockups

Cull screenshots should show real product capability:

- local archive browsing
- visual reference collections
- MCP/agent control
- output composition
- publishing/social layouts
- privacy/local-first settings

Avoid fake data that implies cloud sync, analytics, or subscriptions unless the feature exists.

## Claims

Do not make unverified legal or privacy claims.

Allowed now:

- local-first
- privacy-first as a product principle
- open source
- no hidden data collection as an intended standard
- made in Europe
- designed to respect European expectations

Needs audit before stronger claims:

- GDPR compliant
- no data collection
- fully offline
- no third-party dependency
- certified private

## Asset Style Rules

- Keep the flat icon family as the default.
- Use gradients only when intentionally exploring a separate direction.
- Use full-bleed image sections without borders where the art should lead.
- Use framed UI only when showing actual app screens or controlled outputs.
- Do not overdecorate with generic AI visual motifs.
