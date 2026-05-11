# ADR-005: BYOK (Bring Your Own Key) for All Cloud APIs

**Status:** Accepted
**Date:** 2025-06
**Author:** Gleb Kalinin

## Context

Some features benefit from cloud AI services: higher-quality embeddings (Gemini), image generation (OpenAI, OpenRouter), and vision analysis. Need to decide how to handle API access and billing.

## Options Considered

1. **Bundled API access** — App proxies requests through our server, we pay and bill users. Simplest UX, but requires server infra, billing system, and ongoing cost management
2. **BYOK (Bring Your Own Key)** — Users provide their own API keys. Zero server cost, users control their own billing and data
3. **Hybrid** — Bundled for basic, BYOK for power users

## Decision

BYOK for all cloud APIs. No server-side proxy.

## Rationale

- Privacy: user's API keys mean their data relationship is directly with the provider (OpenAI, Google), not with us
- No server cost: we don't run or pay for cloud infra
- No billing complexity: users manage their own spend
- GDPR clarity: we are not a data processor — the user has a direct relationship with the API provider
- Matches the local-first philosophy: cloud features are strictly opt-in
- Secrets stored locally via OS keychain (macOS Keychain, Windows Credential Manager)
- For a future paid App Store version, bundled API access could be the premium differentiator

## Consequences

- Worse onboarding UX: users must obtain and enter API keys themselves
- Can't control API costs or quality of service
- Users may hit rate limits or billing issues we can't debug
- Each provider has different ToS the user must accept independently
