# Cull Release Audit 2026-06-09 — Decision Sheet

Machine-parseable taste calls. One row per decision: the identity call, every non-KEEP triage verdict, and the plugin scope. `user_decision` is pre-filled with the audit recommendation — edit it (and fill `override_reason`) to override. `evidence_ids` reference ids in `findings.json` and `inventory.json`.

| item_id | type | audit_recommendation | user_decision | override_reason | evidence_ids |
|---|---|---|---|---|---|
| identity | identity | AI art library — browse, CLIP semantic search, collections, generation metadata | AI art library — browse, CLIP semantic search, collections, generation metadata | | mcp-server, grid_view, similarity_search, collection_management, lineage_view, PERF-07 |
| grid_view | triage | CORE | CORE | | grid_view, PERF-07, CQ-4 |
| loupe_view | triage | CORE | CORE | | loupe_view, PERF-04, UX-10 |
| lineage_view | triage | CORE | CORE | | lineage_view |
| command_palette | triage | CORE | CORE | | command_palette, UX-07 |
| sidebar | triage | CORE | CORE | | sidebar, UX-03, UX-04 |
| collection_management | triage | CORE | CORE | | collection_management |
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
| publish_view | triage | PLUGIN (extract via Track C; Day-4 fallback = ship module-gated as today) | PLUGIN (extract via Track C; Day-4 fallback = ship module-gated as today) | | publish_view |
| delivery_csv | triage | DEMOTE (client-tools settings toggle, default off) | DEMOTE (client-tools settings toggle, default off) | | delivery_csv, client_feedback |
| client_feedback | triage | DEMOTE (client-tools settings toggle, default off) | DEMOTE (client-tools settings toggle, default off) | | client_feedback |
| preview_display | triage | DEMOTE (client-tools settings toggle, default off) | DEMOTE (client-tools settings toggle, default off) | | preview_display |
| prompt_resubmit | triage | DEMOTE (gate behind settings until stale-cost bug and CSP fixed) | DEMOTE (gate behind settings until stale-cost bug and CSP fixed) | | prompt_resubmit, CQ-6, SEC-002 |
| publish-clipboard-group | triage | DEMOTE (keep module_static_publishing off by default) | DEMOTE (keep module_static_publishing off by default) | | publish-clipboard-group |
| voice_dictation | triage | DEMOTE (settings toggle, default off) | DEMOTE (settings toggle, default off) | | voice_dictation |
| raw_support | triage | DEMOTE (ratify existing module_raw default-off; label experimental) | DEMOTE (ratify existing module_raw default-off; label experimental) | | raw_support |
| preview_web_stream | triage | DEMOTE (default off behind client-tools toggle) | DEMOTE (default off behind client-tools toggle) | | preview_web_stream |
| session_timeline | triage | CUT (delete dead component) | CUT (delete dead component) | | session_timeline, CQ-5 |
| ocr_text_extraction | triage | CUT (unwind command registration + JobProgressPanel row; check search_text consumers) | CUT (unwind command registration + JobProgressPanel row; check search_text consumers) | | ocr_text_extraction |
| near_duplicate_detection | triage | CUT (remove dormant commands + api wrappers) | CUT (remove dormant commands + api wrappers) | | near_duplicate_detection |
| color_analysis | triage | CUT (remove dormant pipeline + orphan color_label filter in RuleBuilder) | CUT (remove dormant pipeline + orphan color_label filter in RuleBuilder) | | color_analysis, smart_collections |
| plugin-scope | plugin-scope | Hybrid frontend-ESM plugins over a Rust-enforced permission bridge reusing MCP capability vocabulary; static registry.json v1 (tag-pinned, SHA-256); proof plugin = cull-publish (publish_view extraction); 12.0h committed scope (uninstall/update UX pre-cut); Day-4 fallback valve ships publish_view module-gated as today | Hybrid frontend-ESM plugins over a Rust-enforced permission bridge reusing MCP capability vocabulary; static registry.json v1 (tag-pinned, SHA-256); proof plugin = cull-publish (publish_view extraction); 12.0h committed scope (uninstall/update UX pre-cut); Day-4 fallback valve ships publish_view module-gated as today | | publish_view, mcp-server, tokens-management-group, audit-logging-group |
