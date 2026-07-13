# Open Source Transition Audit

Last reviewed: 2026-07-13

Cull is now released under the Apache License 2.0. This note records the
transition checklist and the evidence used for the current release decision.

## Current Release Evidence

- v0.3.1 was published from production workflow run `29182947689` as an
  Apache-2.0 release.
- The public Apple Silicon DMG and updater archive were signed and notarized;
  the updater signature, checksums, codesign, Gatekeeper assessment, and
  notarization staple were verified before publication.
- Release gates include `npm run audit:licenses` and the supply-chain audit, and
  Homebrew promotion consumes the exact verified public DMG SHA-256.
- Cull's Apache-2.0 grant still excludes third-party model weights, fonts,
  artwork, and other assets except where their own recorded licences allow use.

## License Metadata

- `LICENSE`: conventional Apache License 2.0 text for GitHub and distribution tooling.
- `NOTICE`: project notice, AI-assistance note, and third-party model boundary.
- `package.json`: `Apache-2.0`.
- `package-lock.json`: root package `Apache-2.0`.
- `src-tauri/Cargo.toml`: `Apache-2.0`.
- README license section: Apache-2.0, with a pointer to this audit.

Verification:

```bash
rg -n "BUSL-1.1|Business Source License|Commercial use requires" \
  LICENSE README.md package.json package-lock.json src-tauri/Cargo.toml src/lib src-tauri/src
```

Expected result: no matches in the active license files or application source.

## Dependency License Audit

Run:

```bash
npm run audit:licenses
```

Last result on 2026-06-04: passed.

Summary from the last passing run:

- npm packages: 144 packages; licenses were MIT, Apache-2.0, MIT/Apache-2.0
  variants, ISC, BSD-3-Clause, MPL-2.0, 0BSD, and OFL-1.1.
- Cargo packages: 677 packages; no GPL, AGPL, or LGPL-only dependency was
  detected. Two `r-efi` versions expose `MIT OR Apache-2.0 OR
  LGPL-2.1-or-later`; Cull can use the MIT or Apache-2.0 option.
- Source GPL scan: 0 matches in application code, scripts, tests, and package
  manifests.
- Built-in incompatible model download URL scan: 0 matches.

The audit script treats missing dependency license metadata, GPL-family-only
licenses, and hardcoded incompatible model download URLs as failures.

## Supply Chain And SBOM Audit

Release supply-chain commands:

```bash
npm run audit:supply-chain
npm run audit:sbom
```

`scripts/supply-chain-audit.sh` runs RustSec advisory and policy tooling when
available:

- `cargo-deny` for RustSec advisories, license policy, bans, and source checks.
- `cargo-audit` for an additional RustSec advisory pass.
- `cargo-cyclonedx` and `@cyclonedx/cyclonedx-npm` for CycloneDX SBOM generation
  under `dist/sbom/`.

If a required tool is missing, the script fails with an explicit install hint
instead of silently treating the audit as passed.

## AI-Generated Code And Provenance

The release relies on the authorship record in `AUTHORSHIP.md`:

- human architecture, product decisions, data models, component boundaries, and
  review are documented as human-authored;
- AI-assisted implementation is disclosed;
- provider output terms are not treated as a substitute for human authorship,
  source provenance, or license compatibility review.

Contributor policy in `CONTRIBUTING.md` now requires contributors to avoid
unlicensed, source-available, non-commercial, GPL, AGPL, LGPL, or otherwise
incompatible copied code. AI-assisted contributions must be reviewed and must
not include generated output that matches public code unless the upstream
license is compatible and notices are preserved.

Provider output terms are recorded as supporting evidence, not as a substitute
for copyrightability or source-provenance review:

- OpenAI states that, as between the user and OpenAI and to the extent permitted
  by law, users `"own the Output"`:
  https://openai.com/policies/row-terms-of-use/
- Anthropic states that its Commercial Terms let customers `"retain ownership rights"`
  over generated outputs:
  https://www.anthropic.com/news/expanded-legal-protections-api-improvements

The release position remains that Cull is a human-authored project with
AI-assisted implementation under documented human architecture, selection,
arrangement, review, and integration.

## Model Weights

Cull's Apache-2.0 license applies to the application source. Model weights keep
their own licenses.

Current built-in embedding downloads:

| Model | Source | License note | Status |
| --- | --- | --- | --- |
| CLIP ViT-B/32 vision ONNX | `Qdrant/clip-ViT-B-32-vision` | Hugging Face model card is tagged `mit`; OpenAI CLIP repository license is MIT. | Allowed |
| DINOv2 ViT-S/14 ONNX | `sefaburak/dinov2-small-onnx` | Hugging Face model card is tagged `apache-2.0`; upstream DINOv2 is Apache-2.0. | Allowed |

Detection model changes made for this transition:

- Built-in Ultralytics YOLO11 downloads were disabled because Ultralytics
  documents YOLO11 models as AGPL-3.0 or Enterprise licensed.
- Built-in NudeNet ONNX downloads were disabled because the referenced
  Hugging Face model has no explicit license tag in the visible model card.
- Local detection still works when the user places a compatible ONNX file in the
  models directory. That user-supplied file is outside Cull's Apache-2.0 grant.

Sources used for this model decision:

- Qdrant CLIP model card: https://huggingface.co/Qdrant/clip-ViT-B-32-vision
- OpenAI CLIP license: https://github.com/openai/CLIP/blob/main/LICENSE
- DINOv2 ONNX model card: https://huggingface.co/sefaburak/dinov2-small-onnx
- DINOv2 upstream license: https://github.com/facebookresearch/dinov2/blob/main/LICENSE
- Ultralytics YOLO11 docs: https://docs.ultralytics.com/models/yolo11
- NudeNet ONNX model page: https://huggingface.co/vladmandic/nudenet/blob/main/nudenet.onnx

## Asset Inventory

Asset policy lives in `docs/ASSETS.md`.

Current release boundaries:

- Generated app icons and app/site mockups are part of the repository.
- Yulia Katan artwork may be used only where written permission covers the use;
  do not expand that permission to merchandise, paid advertising, model
  training, or derivative image generation without a separate grant.
- Fonts must remain under OFL or similarly permissive terms before bundling.
- Future model, artwork, or font additions must record source, license,
  attribution requirements, and allowed uses before release.

Tracked opaque fixture note:

- `src-tauri/tests/fixtures/db/v21.db` is a synthetic migration fixture. Current
  inspection shows it contains no image, path, token, audit-log, or user-content rows;
  it keeps only schema/migration/default-project state needed by tests.

## Remaining Release Discipline

Before publishing a public release:

1. Run `npm run audit:licenses`.
2. Run the normal quality gates (`npm run ci`, or the relevant frontend and Rust
   slices if the full gate is too slow).
3. Re-check that application metadata still says Apache-2.0.
4. Re-check that optional model download URLs remain compatible or disabled.
5. Keep `AUTHORSHIP.md`, `CONTRIBUTING.md`, and `docs/ASSETS.md` current when
   contributors, AI tools, model sources, or bundled assets change.
