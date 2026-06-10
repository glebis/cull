# Cull Release Audit 2026-06-09 — Decision Sheet

Machine-parseable taste calls. One row per decision: the identity call, every non-KEEP triage verdict, and the plugin scope. `user_decision` is pre-filled with the audit recommendation — edit it (and fill `override_reason`) to override. `evidence_ids` reference ids in `findings.json` and `inventory.json`.

Triage rows re-generated 2026-06-10 against the user-chosen identity "agent-native image tool" per the override rework rule (only Phase 3 re-ran; inventory, findings, and plugin mechanism stand).

| item_id | type | audit_recommendation | user_decision | override_reason | evidence_ids |
|---|---|---|---|---|---|
| identity | identity | AI art library — browse, CLIP semantic search, collections, generation metadata | agent-native image tool — MCP surface as headline differentiator | user decision at gate 2026-06-10 | mcp-server, grid_view, similarity_search, collection_management, lineage_view, PERF-07 |
| grid_view | triage | CORE | CORE | | grid_view, PERF-07, CQ-4 |
| loupe_view | triage | CORE | CORE | | loupe_view, PERF-04, UX-10 |
| command_palette | triage | CORE | CORE | | command_palette, UX-07 |
| sidebar | triage | CORE | CORE | | sidebar, UX-03, UX-04 |
| collection_management | triage | CORE | CORE | | collection_management, browse-curate-group |
| smart_collections | triage | CORE | CORE | | smart_collections, color_analysis |
| import_banner | triage | CORE | CORE | | import_banner, UX-02, UX-04, PERF-02 |
| similarity_search | triage | CORE | CORE | | similarity_search, PERF-07 |
| image_search | triage | CORE | CORE | | image_search, PERF-07 |
| command_bar | triage | CORE | CORE | | command_bar, UX-06 |
| keyboard_shortcuts_ui | triage | CORE | CORE | | keyboard_shortcuts_ui, UX-01, UX-07 |
| image_loading | triage | CORE | CORE | | image_loading, PERF-03, PERF-05 |
| mcp-server | triage | CORE | CORE | | mcp-server, SEC-001, SEC-004 |
| browse-curate-group | triage | CORE | CORE | | browse-curate-group |
| search-quality-group | triage | CORE | CORE | | search-quality-group, CQ-1 |
| import-export-group | triage | CORE | CORE | | import-export-group, SEC-001 |
| ai-processing-group | triage | CORE | CORE | | ai-processing-group, PERF-01 |
| file_watcher | triage | CORE | CORE | | file_watcher, PERF-05 |
| file_operations | triage | CORE | CORE | | file_operations, CQ-2 |
| mcp_settings | triage | CORE (was KEEP — headline config surface: tokens, modules, privacy) | CORE (was KEEP — headline config surface: tokens, modules, privacy) | | mcp_settings, SEC-004 |
| agent_snapshots | triage | CORE (was KEEP — category-unique agent surface) | CORE (was KEEP — category-unique agent surface) | | agent_snapshots, display-ui-group |
| privacy_dashboard | triage | CORE (was KEEP — trust surface for the scoped/audited/revocable claim) | CORE (was KEEP — trust surface for the scoped/audited/revocable claim) | | privacy_dashboard, SEC-004 |
| display-ui-group | triage | CORE (was KEEP — agent view-driving is the headline demo) | CORE (was KEEP — agent view-driving is the headline demo) | | display-ui-group |
| publish-clipboard-group | triage | CORE (was DEMOTE — "publish" is in the identity sentence; module gate stays as consent step) | CORE (was DEMOTE — "publish" is in the identity sentence; module gate stays as consent step) | | publish-clipboard-group, publish_view |
| tokens-management-group | triage | CORE (was KEEP — SEC-004 expiry/audit gaps promoted to polish budget) | CORE (was KEEP — SEC-004 expiry/audit gaps promoted to polish budget) | | tokens-management-group, SEC-004 |
| audit-logging-group | triage | CORE (was KEEP — audit trail is the trust half of the headline) | CORE (was KEEP — audit trail is the trust half of the headline) | | audit-logging-group, SEC-004 |
| headless_cli | triage | CORE (was KEEP — proves the tool layer is the product API; polish = documentation) | CORE (was KEEP — proves the tool layer is the product API; polish = documentation) | | headless_cli, mcp-server |
| publish_view | triage | PLUGIN (extract via Track C; Day-4 fallback = ship module-gated as today; MCP publish tools stay in core) | PLUGIN (extract via Track C; Day-4 fallback = ship module-gated as today; MCP publish tools stay in core) | | publish_view, publish-clipboard-group |
| delivery_csv | triage | DEMOTE (client-tools settings toggle, default off) | DEMOTE (client-tools settings toggle, default off) | | delivery_csv, client_feedback |
| client_feedback | triage | DEMOTE (client-tools settings toggle, default off) | KEEP (visible in v1) | user decision at gate 2026-06-10 | client_feedback |
| preview_display | triage | DEMOTE (client-tools settings toggle, default off) | KEEP (visible in v1) | user decision at gate 2026-06-10 | preview_display |
| prompt_resubmit | triage | DEMOTE (gate behind settings until stale-cost bug and CSP fixed) | KEEP (visible in v1; consequence: CQ-6 stale-cost bug and SEC-002 CSP scoping move to Track A pre-launch) | user decision at gate 2026-06-10 | prompt_resubmit, CQ-6, SEC-002 |
| voice_dictation | triage | DEMOTE (settings toggle, default off) | DEMOTE (settings toggle, default off) | | voice_dictation |
| raw_support | triage | DEMOTE (ratify existing module_raw default-off; label experimental) | KEEP (visible in v1; un-gate module_raw) | user decision at gate 2026-06-10 | raw_support |
| preview_web_stream | triage | DEMOTE (default off behind client-tools toggle) | KEEP (visible in v1) | user decision at gate 2026-06-10 | preview_web_stream |
| session_timeline | triage | CUT (delete dead component) | CUT (delete dead component) | | session_timeline, CQ-5 |
| ocr_text_extraction | triage | CUT (unwind command registration + JobProgressPanel row; check search_text consumers) | KEEP (dormant code ships as-is) | user decision at gate 2026-06-10 | ocr_text_extraction |
| near_duplicate_detection | triage | CUT (remove dormant commands + api wrappers) | KEEP (dormant code ships as-is) | user decision at gate 2026-06-10 | near_duplicate_detection |
| color_analysis | triage | CUT (remove dormant pipeline + orphan color_label filter in RuleBuilder) | KEEP (dormant code ships as-is) | user decision at gate 2026-06-10 | color_analysis, smart_collections |
| audit-doc-2026-06-03 | content-pass | (deferred by fresh-eyes rule) | ARCHIVE INTERNAL — read and assessed 2026-06-10: no personal data, internal URLs, or secrets; its security findings are already public via the bd tracker mirror (`.beads/issues.jsonl`), so the raw ChatGPT export adds no information while being an internal working artifact; moved to gitignored `docs/internal/`, AGENTS.md reference updated; no history rewrite | executed for bd imageview-dkz.9 (HYG-004/SEC-005) | HYG-004 |
| beads-issues-jsonl | content-pass | untrack or scrub | KEEP TRACKED — it is the public tracker mirror; owner email and security-issue detail equal what git commit metadata and the tracker already disclose publicly; scrubbing would be reverted by every bd re-export | executed for bd imageview-dkz.9 | HYG-004 |
| beads-interactions-jsonl | content-pass | untrack or scrub | UNTRACK — agent/user interaction trace log (status changes with actor + close reasons); `git rm --cached`, kept locally, covered by existing `.beads/` gitignore; no history rewrite | executed for bd imageview-dkz.9 | HYG-004 |
| personal-paths-docs | content-pass | replace with placeholders pre-launch | SCRUBBED — release-skill plan paths → `$CULL_REPO`/`$CULL_LANDING_WORKTREE`; clipboard-monitor spec capture_dir → `~/Pictures/...`; release-audit findings.json/report.md evidence quotes → `$HOME`; enforced by open-source-release-contract.test.ts | executed for bd imageview-dkz.9 | SEC-005 |
| agents-md-personal-refs | content-pass | trim to project-relevant entries | TRIMMED — personal skills repo, Obsidian vault, Linear CLI, and load-on-demand ref entries removed from AGENTS.md Reference Paths; beads references kept | executed for bd imageview-dkz.9 | HYG-004 |
| internal-working-artifacts | content-pass | private archive or rewrite | ARCHIVE INTERNAL — vision-brainstorm-raw, dev-workflow-audit-2026-06-02, tooling-research-2026-06-03, settings-mockup-draft.json, settings-mockup-v2.json, oss-strategy-explorer.html moved to gitignored `docs/internal/`; vision-brainstorm-processed (curated) stays tracked | executed for bd imageview-dkz.9 | HYG-004 |
| plugin-scope | plugin-scope | Hybrid frontend-ESM plugins over a Rust-enforced permission bridge reusing MCP capability vocabulary; static registry.json v1 (tag-pinned, SHA-256); proof plugin = cull-publish (publish_view extraction); 12.0h committed scope (uninstall/update UX pre-cut); Day-4 fallback valve ships publish_view module-gated as today | Hybrid frontend-ESM plugins over a Rust-enforced permission bridge reusing MCP capability vocabulary; static registry.json v1 (tag-pinned, SHA-256); proof plugin = cull-publish (publish_view extraction); 12.0h committed scope (uninstall/update UX pre-cut); Day-4 fallback valve ships publish_view module-gated as today | | publish_view, mcp-server, tokens-management-group, audit-logging-group |
