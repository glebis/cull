P0 (Critical / must-fix)
[P0] Move Google API keys out of URL parameters

Category: Security

Evidence: docs/PRIVACY.md explicitly says Gemini embeddings send the API key as a URL parameter; src-tauri/src/db_core/gemini.rs builds ...?key={self.api_key} for embedding requests; src-tauri/src/commands/embeddings.rs also validates Google keys with models?key={key}; SECURITY.md says API keys must never be written to logs/config and secrets should stay behind the secret boundary. Tiny detail, apparently URLs are where secrets go to become everyone’s problem. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: API keys in URLs can leak through request logs, proxy logs, crash/error strings, debugging traces, history, or telemetry outside the app. The code has a redaction helper, but redaction is not a security boundary.

Recommendation: Use a header-based credential path for Google API calls wherever supported, preferably x-goog-api-key as already used elsewhere in generation code, and make URL construction fail tests if it contains key=.

Jobs To Be Done: "When I use BYOK cloud embeddings, I want my Google API key to stay out of URLs, so I can avoid accidental credential disclosure through logs or network tooling."

Acceptance Criteria:

 validate_api_key("google", ...) and GeminiEmbeddingProvider::generate_embedding send credentials via headers, not query parameters.

 A Rust unit test asserts no Google/Gemini request URL produced by the embeddings module contains key=.

 A regression test injects a sentinel API key and confirms logs, audit records, emitted errors, and returned errors never contain the sentinel.

[P0] Remove runtime asset-protocol expansion for pasted originals

Category: Security

Evidence: SECURITY.md promises the Tauri asset: protocol is scoped only to $APPDATA/thumbnails/**, $APPDATA/generated/**, and $HOME/.codex/generated_images/**, and says Cull does not add imported originals or user-selected clipboard capture folders at runtime. src-tauri/src/commands/files.rs violates that by calling app.asset_protocol_scope().allow_file(&target_str) after paste/import. The current regression test checks lib.rs, commands/import.rs, and commands/clipboard_monitor.rs, but not commands/files.rs, which is how the gremlin got in. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Pasted originals are dynamically exposed through the asset: protocol, contradicting the documented file-access boundary and bypassing the intended “app-owned/generated image folders only” model.

Recommendation: Remove the runtime allow_file call. Render pasted files through generated thumbnails, app-owned generated copies, or a narrowly scoped explicit IPC path that returns only the needed bytes.

Jobs To Be Done: "When I paste an image into a library folder, I want Cull to preserve the asset-protocol boundary, so I can trust that original files are not silently exposed to the renderer."

Acceptance Criteria:

 paste_image_from_clipboard no longer calls .asset_protocol_scope().allow_file.

 A repository-wide test scans every src-tauri/src/**/*.rs file and fails on runtime asset_protocol_scope, allow_file, or allow_directory usage outside an audited allowlist.

 Pasted images still render via generated thumbnails or app-owned preview assets without granting asset: access to the pasted original.

P1 (High)
[P1] Enforce MCP collection scopes consistently

Category: Security

Evidence: docs/mcp-remote-access.md documents collection, folder, and tag scopes with union semantics. src-tauri/src/mcp/tools.rs::check_image_id_scope loads the image but calls tokens::image_in_scope(&scope, &img.path, &[]), passing no collection membership. src-tauri/src/services/tokens.rs::image_in_scope only allows collection-scoped access when image_collections contains the scoped collection id. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: A collection-scoped remote token can enumerate some collection-derived results, but per-image MCP tools can reject the same image because membership is not loaded. That makes the authorization model inconsistent and brittle.

Recommendation: Centralize image-scope evaluation in one DB-backed helper that loads path, collection membership, and tags for every image id before authorizing any MCP tool.

Jobs To Be Done: "When I issue a collection-scoped MCP token, I want all allowed tools to agree on what images are in scope, so I can safely delegate curation without random permission failures."

Acceptance Criteria:

 A collection-scoped token can list_collections, list_collection_images, get_image, and set_rating for an image in the scoped collection.

 The same token is denied for an image outside the scoped collection.

 All MCP image-id tools call one shared scope-checking function that includes collection membership.

[P1] Implement MCP tag scopes or reject them

Category: Security

Evidence: docs/mcp-remote-access.md advertises token scopes with "tags": ["public"]; the frontend API type TokenScope also includes tags?: string[]. src-tauri/src/services/tokens.rs::image_in_scope checks folders and collections only, then returns false, with no tag logic at all. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Tag-scoped tokens are part of the documented and typed contract, but the authorization engine ignores tags. Depending on UI behavior, users either create broken tokens or trust a scope model that does not exist.

Recommendation: Either implement tag membership checks in the shared MCP scope helper or reject tag scopes at token creation with a clear error until implemented.

Jobs To Be Done: "When I restrict a remote token to a public tag, I want Cull to enforce that tag, so I can expose only deliberately shared images."

Acceptance Criteria:

 Creating a token with tags either succeeds with enforced tag filtering or fails with a specific “tag scopes unsupported” validation error.

 If implemented, tag-scoped tokens can access tagged images and cannot access untagged images.

 Unit tests cover folder-only, collection-only, tag-only, and mixed union scopes.

[P1] Push MCP scope filtering and pagination into SQL

Category: Scalability

Evidence: src-tauri/src/mcp/tools.rs::list_images fetches limit * 3, filters scoped images in memory, and then takes limit; scoped_images scans list_images(100_000, 0) for folder scopes; rescan_sources also hard-caps at list_images(100000, 0). For a 100k+ image app, this is what humans call “planning ahead” while standing in a swamp. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Sparse scopes can return short or empty pages even when matching images exist later, and libraries above 100k images can be silently truncated.

Recommendation: Add SQL-level scope filters with joins for folder, collection, and tag membership; use stable ordering and keyset or offset pagination without fixed global caps.

Jobs To Be Done: "When I query a large library through MCP, I want pagination to return the correct scoped images, so I can automate curation without missing data."

Acceptance Criteria:

 A test database with more than 150k images returns scoped images beyond offset 100k.

 A sparse folder or collection scope returns full pages without relying on limit * 3.

 EXPLAIN QUERY PLAN tests or perf tests confirm indexed lookups for scoped list operations.

[P1] Move blocking import, decode, and database work off async command paths

Category: Scalability

Evidence: Database is a single Arc<Mutex<Connection>>; import reads files, decodes images, generates thumbnails, source detection, sidecar detection, perceptual hashes, and color metrics synchronously; folder import recursively collects all matching entries before importing. Several Tauri commands are async wrappers around blocking work, because apparently async was invited but not allowed to speak. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Large imports and image analysis can block the async runtime, hold the DB mutex for long periods, freeze UI/MCP responsiveness, and serialize unrelated operations.

Recommendation: Move import/decode/thumbnail/analysis work into bounded background jobs using spawn_blocking or a dedicated worker pool, and use separate SQLite connections for read/write paths with WAL and busy_timeout.

Jobs To Be Done: "When I import a large folder, I want the app to stay responsive, so I can keep browsing and cancel or monitor progress."

Acceptance Criteria:

 Importing a synthetic 10k-image folder does not block lightweight commands such as get_library_stats or list_jobs.

 Thumbnail, perceptual hash, color metrics, and source detection run in cancellable background jobs.

 SQLite is configured with WAL and busy_timeout, with tests covering concurrent reads during import.

[P1] Verify migration state with schema invariants

Category: Logic

Evidence: src-tauri/src/db_core/db.rs maintains migrations 1 through 20 and uses PRAGMA user_version as the applied check. run_migration_step records a migration as succeeded when migration_already_applied(version) is true, and migration_already_applied only checks user_version >= version. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: A database with a high user_version but missing tables, columns, indexes, or migration rows can be treated as healthy and have success records backfilled. That risks runtime failures or incorrect data after partial prerelease migrations.

Recommendation: Treat schema_migrations plus schema invariant checks as authoritative after bootstrap. Validate required tables, columns, indexes, and compatibility markers before opening the app normally.

Jobs To Be Done: "When Cull opens an old or partially migrated database, I want it to verify the real schema, so I can avoid silent corruption or broken user data."

Acceptance Criteria:

 A fixture DB with user_version = 20 but missing image_tags fails preflight or is repaired explicitly.

 Migration success rows are not written merely because user_version is high.

 Post-migration tests assert all required tables, columns, indexes, and foreign keys exist.

[P1] Bound file import, hashing, decode, and embedding memory

Category: Scalability

Evidence: src-tauri/src/db_core/import.rs::sync_file reads the full file into memory before hashing, then decodes it; src-tauri/src/db_core/gemini.rs also reads the full image and base64-encodes it for embeddings. 

cull-audit-bundle

 

cull-audit-bundle

Problem: Very large TIFF/PSD/RAW/GIF or malicious image files can cause memory spikes, slow imports, or crashes during normal user input.

Recommendation: Stream SHA-256 hashing, reject or downsample files above configured byte/dimension limits before full decode, and apply provider-specific size limits before cloud embedding uploads.

Jobs To Be Done: "When I import huge or malformed images, I want Cull to fail safely, so I do not lose the session to a memory cliff."

Acceptance Criteria:

 Hashing uses streaming IO and never requires reading the entire file into a single Vec<u8>.

 Import tests cover oversized, malformed, and decompression-bomb-like image fixtures.

 Cloud embedding commands reject images above provider/app limits with a user-visible error before reading/base64-encoding the full file.

[P1] Replace O(N) folder counting with indexed SQL

Category: Scalability

Evidence: src-tauri/src/db_core/db.rs::list_folders selects every non-missing image_files path, builds parent folder counts in Rust, sorts in memory, and then returns the result. 

cull-audit-bundle

Problem: Listing folders becomes an O(N) full scan on every refresh. At 100k+ images, sidebar navigation and MCP folder listing will degrade badly.

Recommendation: Store a normalized parent folder column or derive folder counts with indexed SQL/grouping, and avoid loading every path into Rust for routine UI counts.

Jobs To Be Done: "When I open the sidebar for a large library, I want folders to appear quickly, so browsing does not feel like archaeology."

Acceptance Criteria:

 list_folders uses SQL grouping or indexed derived folder metadata instead of scanning all file rows in Rust.

 A perf test with 100k+ image_files rows completes folder listing under a defined budget.

 Index coverage is asserted for folder/path count queries.

[P1] Validate smart-collection filter edge cases before SQL generation

Category: Logic

Evidence: src-tauri/src/db_core/smart_collections.rs accepts filter JSON with nested groups, many fields, and untagged values; FilterValue::ClipImage uses image_id: i64 even though image ids elsewhere are strings; RuleOp::In and NotIn generate IN () or NOT IN () for empty arrays; LastNDays formats a number directly into SQL after casting to i64. 

cull-audit-bundle

 

cull-audit-bundle

Problem: Empty arrays, invalid numeric values, and mismatched image-id types can produce invalid SQL, wrong smart-collection results, or broken user-visible filters.

Recommendation: Add a validation layer before SQL generation: reject unsupported embedding fields, normalize id types, define empty-array semantics, and require finite non-negative date windows.

Jobs To Be Done: "When I save a smart collection, I want invalid rules rejected clearly, so my collection does not silently show the wrong images."

Acceptance Criteria:

 Empty in arrays compile to a deterministic false clause and empty not_in arrays compile to a deterministic true clause, or both are rejected with validation errors.

 last_n_days rejects negative, NaN, infinite, and fractional values outside the accepted policy.

 Smart-collection tests cover malformed JSON, unsupported CLIP fields, empty arrays, and UUID image ids.

[P1] Unify filesystem path policy across deeplinks, import, and clipboard

Category: Security

Evidence: src-tauri/src/commands/deeplink.rs canonicalizes paths, requires $HOME, blocks sensitive directories, and rejects hidden components. src-tauri/src/services/import.rs recursively walks the supplied folder and adds it as a library root. src-tauri/src/commands/files.rs accepts any existing paste destination, writes to it, and can add it as a library root. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Deeplinks enforce a strict path policy, but import and clipboard paths follow looser rules. This creates inconsistent security and privacy behavior depending on how the same folder enters the app.

Recommendation: Create one PathPolicy module with explicit modes for deeplink, user-picked folder, clipboard destination, MCP remote token, and local admin actions.

Jobs To Be Done: "When a path enters Cull from any surface, I want the same policy rules applied intentionally, so sensitive folders are not imported by accident."

Acceptance Criteria:

 Import, clipboard paste, deeplink, and MCP import all call a shared path-policy module.

 Tests cover symlinks, .., hidden folders, sensitive directories, paths outside $HOME, and explicit user overrides.

 UI errors explain whether a path was blocked because it was outside the library, hidden, sensitive, or outside token scope.

[P1] Restrict static preview serving to Cull-created packages

Category: Security

Evidence: serve_static_publish_package_inner accepts a site_dir string and only checks that index.html exists before serving it on localhost. The static package manifest says the access phrase is for host-level protection and “does not enforce server-side auth.” Request path traversal is blocked, but the served root itself is not verified as a Cull-created package. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: A caller with access to the command can serve any local directory containing index.html, not just Cull export packages. Localhost-only reduces exposure, but tunnels and browser-accessible localhost still make this a real boundary.

Recommendation: Require a Cull package marker, schema-valid data/canvas.json, and canonical export root ownership before serving. Treat the access phrase as UX guidance, not security.

Jobs To Be Done: "When I start the static preview server, I want it to serve only Cull-generated packages, so I do not accidentally expose unrelated local files."

Acceptance Criteria:

 Serving a directory without a valid Cull static-publishing manifest fails.

 Serving a symlinked or outside-export-root directory fails unless explicitly allowed by an audited local-only override.

 Tests verify localhost binding, package validation, traversal blocking, and “access phrase is not auth” messaging.

[P1] Route every trash path through shared confirmation and reliable undo

Category: UX

Evidence: ContextMenu.svelte::handleTrash calls trashImages([...ids]) directly; api.ts exposes both trash_images and delete_images_permanently; TrashConfirmDialog.svelte exists, but the context-menu path bypasses it; undo restores by guessing ~/.Trash/<filename>. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: A destructive context-menu action can skip confirmation, and undo is fragile for duplicate filenames, external volumes, Finder-renamed Trash entries, or files moved after deletion.

Recommendation: Use one destructive-action controller for all trash/permanent delete flows. Store the actual trash destination returned by the OS where possible, not a guessed filename.

Jobs To Be Done: "When I trash images from any UI surface, I want the same safety and undo behavior, so I do not lose work through a shortcut path."

Acceptance Criteria:

 Context menu, keyboard shortcut, menu bar, command palette, and bulk actions all use the same confirmation/suppression policy.

 Undo succeeds for duplicate filenames and external-volume trash cases in automated tests.

 Permanent delete always requires an explicit stronger confirmation than Move to Trash.

[P1] Make destructive dialogs and palette overlays fully modal

Category: Accessibility

Evidence: TrashConfirmDialog.svelte suppresses Svelte a11y warnings on static interactive elements, lacks role="dialog"/aria-modal, and confirms on Enter from the overlay/dialog. CommandPalette.svelte has a dialog and combobox/listbox semantics, but also nests a hotkey dialog and context menu inside the palette overlay. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Focus management and screen-reader semantics are incomplete for high-risk interactions, especially destructive confirmation and nested command-palette overlays.

Recommendation: Introduce a shared modal/dialog primitive with focus trap, focus restore, aria-labelledby, aria-describedby, inert background behavior, Escape handling, and safe default focus.

Jobs To Be Done: "When I use Cull with a keyboard or screen reader, I want modal dialogs to behave predictably, so I can avoid accidental destructive actions."

Acceptance Criteria:

 Trash confirmation uses role="dialog", aria-modal="true", aria-labelledby, and initial focus on Cancel or the least destructive action.

 Tab and Shift+Tab are trapped inside each active modal, including hotkey capture.

 Automated tests verify Escape closes, focus returns to the opener, and Enter does not trigger destructive action unless the destructive button is focused.

[P1] Expose thumbnail state through keyboard and screen-reader semantics

Category: Accessibility

Evidence: Thumbnail.svelte renders a div role="gridcell" with aria-label={filename}, aria-selected, Enter handling, and visual overlays for missing status, rating, source tag, and decision. Those overlays are not included in the accessible label, and Space does not select from the tile handler. 

cull-audit-bundle

 

cull-audit-bundle

Problem: Keyboard and screen-reader users cannot reliably perceive or operate the same state that sighted mouse users see.

Recommendation: Implement proper grid semantics with roving tabindex, Space selection, Enter open, aria-selected, and labels/descriptions that include filename, rating, decision, source, and missing-file state.

Jobs To Be Done: "When I review images without a mouse or with a screen reader, I want each tile’s state announced and operable, so I can curate at the same speed as visual users."

Acceptance Criteria:

 Space toggles selection and Enter opens/activates the focused thumbnail.

 Screen-reader labels include filename, rating, decision, source tag, selected state, and missing-file state.

 Automated accessibility tests cover grid navigation, selection, and state announcements.

P2 (Medium/Nice-to-have)
[P2] Replace direct Svelte prop mutation in thumbnails

Category: BestPractices

Evidence: Thumbnail.svelte::handleImgError regenerates a thumbnail and then mutates item.thumbnail_path = newPath directly on a prop-derived object. 

cull-audit-bundle

Problem: Directly mutating nested prop data is fragile in Svelte 5 and can produce stale UI state, especially when the parent store remains unchanged.

Recommendation: Emit a thumbnail-regenerated event or update the central images store by image id, preserving unidirectional data flow.

Jobs To Be Done: "When a thumbnail regenerates after load failure, I want the parent image state updated consistently, so all views show the new preview."

Acceptance Criteria:

 Thumbnail.svelte no longer mutates item or nested prop fields.

 A unit test verifies thumbnail regeneration updates the parent store and rerenders the thumbnail.

 Svelte check and tests pass with no prop mutation warnings or stale preview regressions.

[P2] Add design-token and contrast regression checks

Category: Accessibility

Evidence: AGENTS.md says all components must use Tokyo Night design tokens and never hardcode colors. Canvas.svelte contains hardcoded rgba(...) overlays and shadows. The design system claims --text-secondary is WCAG AA compliant, but there is no systematic contrast/token enforcement shown. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: Visual regressions and low-contrast states can creep in as hardcoded colors bypass tokens.

Recommendation: Add style linting or tests that allow hardcoded colors only in token definitions, generated static export templates, and documented exceptions.

Jobs To Be Done: "When the UI changes, I want color and contrast rules checked automatically, so the dark theme remains readable."

Acceptance Criteria:

 Component CSS hardcoded colors are rejected unless listed in an explicit allowlist.

 Contrast tests cover primary, secondary, disabled, warning, error, focus, and selected states.

 Canvas crop overlays are represented as named tokens or documented exceptions with contrast tests.

[P2] Narrow broad opener permissions

Category: Security

Evidence: tauri-command-contract.test.ts expects opener:allow-open-path to allow { path: '$HOME/**/*' } and { path: '$APPDATA/**/*' }, even though the test description says it is for opening generated local files from share results. 

cull-audit-bundle

Problem: The permission is broader than the described use case. If renderer code is compromised, opening arbitrary files under $HOME is a wider capability than opening Cull-generated exports.

Recommendation: Restrict opener permissions to Cull export/output directories and route broader “Reveal/Open” actions through validated Rust commands.

Jobs To Be Done: "When Cull opens generated files, I want the permission to cover only generated outputs, so a compromised UI cannot open arbitrary home-directory paths."

Acceptance Criteria:

 opener:allow-open-path no longer grants all $HOME/**/*.

 Generated export/share files still open successfully.

 Attempts to open unrelated home-directory paths through the opener permission fail in tests.

[P2] Add CVE and SBOM audits to release gates

Category: AuditNeeded

Evidence: package.json has audit:licenses but no dependency vulnerability or SBOM script. docs/OPEN_SOURCE_AUDIT.md records a license audit and model-license review, not a CVE audit. 

cull-audit-bundle

 

cull-audit-bundle

Problem: License compatibility is checked, but known-vulnerability exposure is not represented as a first-class release gate.

Recommendation: Add cargo audit or cargo deny, npm vulnerability audit policy, Dependabot enforcement, and SBOM generation for release artifacts.

Jobs To Be Done: "When I cut a release, I want dependency vulnerabilities and provenance captured, so users are not relying on vibes wearing a trench coat."

Acceptance Criteria:

 Release preflight runs Cargo and npm vulnerability checks with a documented severity policy.

 CI produces or validates an SBOM for packaged builds.

 Dependency exceptions require expiry dates and documented rationale.

[P2] Run a dedicated privacy and data-flow audit

Category: AuditNeeded

Evidence: docs/PRIVACY.md states cloud features are BYOK and lists image/prompt data sent to Gemini, OpenAI, OpenRouter, remote Ollama, and MCP HTTP. It also says Google/OpenAI/OpenRouter privacy terms require current review and lists exactly where external network calls live. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: The app handles private images, prompts, API keys, audit logs, thumbnails, and optional remote access. The current doc is useful, but it is not a formal DPIA/data-flow audit.

Recommendation: Produce a dedicated privacy audit covering consent gates, provider terms, retention, audit-log prompt storage, clipboard capture, thumbnails, MCP HTTP exposure, and GDPR/EU transfer posture.

Jobs To Be Done: "When I enable cloud or remote features, I want to understand exactly what leaves my machine, so I can make informed privacy decisions."

Acceptance Criteria:

 A data-flow diagram maps every image/prompt/API-key path from UI to storage/network.

 Cloud feature UI includes provider-specific consent and sensitive-image warnings.

 Audit-log retention, redaction, and export/delete behavior are documented and tested.

[P2] Run an MCP remote-access threat-model and penetration test

Category: AuditNeeded

Evidence: SECURITY.md says local MCP socket connections are full-admin based on filesystem permissions, while HTTP uses bearer tokens, roles, Origin validation, and localhost default binding. docs/mcp-remote-access.md documents Tailscale, Cloudflare Tunnel, and ngrok exposure patterns. src-tauri/src/mcp/http.rs implements body limits, rate limits, concurrency limits, and request timeouts. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: MCP is a high-power automation surface over local files, exports, AI runs, and token management. The design has controls, but it deserves adversarial testing before remote-use marketing.

Recommendation: Commission or perform a focused MCP threat model covering token theft, replay, brute force, role/scope bypass, Origin behavior behind tunnels, audit log integrity, and local-socket abuse by same-user malware.

Jobs To Be Done: "When I expose MCP beyond localhost, I want the remote-control plane tested adversarially, so I can avoid turning my image library into a LAN piñata."

Acceptance Criteria:

 Negative tests cover missing/invalid/expired/revoked tokens for every HTTP MCP route.

 Role and scope bypass attempts are tested against all MCP tools.

 A written threat model documents trust boundaries, assets, attackers, mitigations, and residual risk.

[P2] Add fuzz and property tests for untrusted parsers

Category: AuditNeeded

Evidence: Deeplink paths are parsed and canonicalized; smart-collection JSON is deserialized into a recursive filter tree and compiled to SQL; source detection parses filenames and metadata-like text with regexes. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: These are attacker- or user-controlled parsers. Unit examples exist, but fuzzing would catch weird path, JSON, Unicode, recursion, and SQL-generation cases humans generously fail to imagine.

Recommendation: Add cargo-fuzz or proptest targets for deeplinks, smart-collection filter JSON, sidecar metadata, source detection, and static export patch/manifest parsing.

Jobs To Be Done: "When Cull parses external strings or metadata, I want pathological input tested automatically, so malformed files and URLs cannot crash or corrupt state."

Acceptance Criteria:

 Fuzz/property targets exist for deeplink parsing, smart filters, source detection, and sidecar/manifest parsing.

 CI or nightly automation runs a bounded fuzz/property suite.

 Any parser panic on arbitrary input is treated as a failing test.

[P2] Promote keyboard and accessibility E2E coverage into automation

Category: BestPractices

Evidence: AGENTS.md says browser E2E runs through Chrome Beta/CDP but is classified as a manual pre-push gate, while package.json exposes test:e2e. 

cull-audit-bundle

 

cull-audit-bundle

Problem: Manual E2E gates are easy to skip, because human discipline has historically been a spectacularly lossy compression algorithm.

Recommendation: Add automated smoke coverage for command palette, grid keyboard navigation, destructive dialogs, import empty states, and core accessibility checks in CI or nightly builds.

Jobs To Be Done: "When UI behavior changes, I want keyboard and accessibility regressions caught automatically, so release quality does not depend on memory."

Acceptance Criteria:

 CI or nightly automation runs a headless/browser smoke suite for grid, command palette, dialogs, and static publishing.

 The suite includes keyboard-only flows for navigation, selection, rating, command execution, and dialog cancellation.

 Accessibility checks fail on missing dialog roles, unlabeled actionable controls, and broken focus return.

[P2] Keep model, asset, and AI-provenance audits current

Category: AuditNeeded

Evidence: docs/OPEN_SOURCE_AUDIT.md records Apache-2.0 metadata alignment, dependency license checks, AI-assisted authorship policy, model-weight license boundaries, disabled YOLO/NudeNet downloads, and asset inventory boundaries. 

cull-audit-bundle

 

cull-audit-bundle

 

cull-audit-bundle

Problem: The current open-source audit is good evidence for this snapshot, but model cards, dependency metadata, bundled assets, and AI-generated provenance assumptions can change.

Recommendation: Make license/provenance review a required release artifact, especially when adding model downloads, bundled images, fonts, generated assets, or AI-assisted code from new sources.

Jobs To Be Done: "When Cull ships publicly, I want source, model, and asset rights documented, so users and contributors know what they can legally rely on."

Acceptance Criteria:

 Release checklist requires updated license/provenance audit notes.

 New model downloads require source URL, SPDX/license terms, checksum, commercial-use status, and attribution notes.

 Asset additions require documented ownership or permission before merge.
