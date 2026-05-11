# Liveblocks Research for ImageView

**Date:** 2026-05-10
**Context:** Evaluating Liveblocks as a real-time collaboration layer for an open-source Tauri + Svelte 5 image viewer.

---

## 1. Svelte Support

Liveblocks has **no dedicated Svelte SDK**. Integration uses the framework-agnostic `@liveblocks/client` package directly. Liveblocks provides an official [Svelte quickstart guide](https://liveblocks.io/docs/get-started/svelte) and [Svelte examples](https://liveblocks.io/examples/browse/all/svelte) including Yjs-based editors (Tiptap, Quill, Monaco, CodeMirror). SvelteKit authentication helpers exist for both [access tokens](https://liveblocks.io/docs/authentication/access-token/sveltekit) and [ID tokens](https://liveblocks.io/docs/authentication/id-token/sveltekit).

**Practical impact:** No reactive Svelte hooks like the React SDK offers (`useMyPresence`, `useOthers`). You call the client API imperatively and wire results into Svelte stores/runes yourself. This is extra work but fully viable -- the client API is well-documented.

## 2. Pricing Model

| Plan | Cost | Monthly Active Rooms | Key Limits |
|------|------|---------------------|------------|
| Free | $0 | 500 | 256 MB realtime storage, 3 dashboard seats, paused on overage |
| Pro | $29/mo | 1,000 (then pay-as-you-go) | Removes branding, $0.02/additional MAU |
| Enterprise | Custom | Custom | SLA, SSO, dedicated support |

**Unlimited MAUs on Free** -- Liveblocks charges by rooms, not by connected users. The "bring your own API key" model works: each user creates their own Liveblocks account, enters their key in app settings, and gets their own 500 free rooms. No per-seat cost for the open-source project itself.

**Caveat:** Free tier pauses when limits are exceeded (not hard-blocked). The "Powered by Liveblocks" badge is required on Free.

## 3. Core Capabilities

**Conflict resolution:** Property-level CRDT with Last-Write-Wins (LWW) on same-property conflicts. Built on three primitives: `LiveObject`, `LiveMap`, `LiveList`. Not character-level (no native rich-text CRDT), but Liveblocks offers a [Yjs integration](https://liveblocks.io/blog/introducing-liveblocks-yjs) for text editing scenarios.

**Relevant features for ImageView:**
- **Presence** -- ephemeral per-user state (cursor position, selected image, current viewport). Built-in, low-latency.
- **Storage** -- persistent shared state (image ratings, annotations, collection metadata). Synced via CRDT.
- **Broadcast** -- fire-and-forget events (e.g., "user X started a slideshow").
- **Comments & Notifications** -- built-in threads and notification system, could work for image annotations.

All of these map well to shared cursors, selection state, ratings, and annotations.

## 4. Tauri Compatibility

No known issues. Liveblocks communicates over **standard WebSockets** from the browser context. Tauri's webview (WebKit on macOS/Linux, WebView2 on Windows) supports WebSockets natively. The `@liveblocks/client` package is pure JavaScript with no browser-specific APIs beyond `WebSocket` and `fetch`.

**Potential concern:** If a Tauri app restricts outbound network access via its security config, you need to allowlist `*.liveblocks.io` in the CSP/allowlist. Otherwise, no Tauri-specific blockers were found in documentation or community discussions.

## 5. Alternatives

| Alternative | Tradeoff vs Liveblocks |
|-------------|----------------------|
| **Y.js (self-hosted)** | Full control, no vendor lock-in, free at any scale -- but requires hosting your own WebSocket server (y-websocket or Hocuspocus), managing persistence, and handling scaling yourself. |
| **PartyKit (Cloudflare)** | Serverless WebSocket rooms on Cloudflare Workers, good DX -- but acquired by Cloudflare, less collaboration-specific tooling (no built-in presence/storage primitives), you build more from scratch. |
| **Supabase Realtime** | Piggybacks on Postgres changes, great if you already use Supabase -- but not CRDT-based, not designed for fine-grained collaborative state, weaker conflict resolution for simultaneous edits. |

---

## Recommendation

Liveblocks is a strong fit for ImageView's collaboration needs. The BYOK model works for open source, the free tier is generous, and the feature set (presence + storage + broadcast) covers shared cursors, ratings, and annotations without building infrastructure. The main cost is the lack of a Svelte SDK -- you write thin wrappers around `@liveblocks/client` yourself.

If avoiding vendor dependency is a priority, Y.js with a self-hosted backend (e.g., Hocuspocus) is the strongest alternative, at the cost of significantly more infrastructure work.

---

Sources:
- [Liveblocks Svelte Quickstart](https://liveblocks.io/docs/get-started/svelte)
- [Liveblocks Pricing](https://liveblocks.io/pricing)
- [Liveblocks Plans Documentation](https://liveblocks.io/docs/pricing/plans)
- [Liveblocks Presence Tutorial](https://liveblocks.io/docs/tutorial/react/getting-started/presence)
- [Liveblocks Yjs Integration](https://liveblocks.io/blog/introducing-liveblocks-yjs)
- [Liveblocks Architecture Blog](https://liveblocks.io/blog/understanding-sync-engines-how-figma-linear-and-google-docs-work)
- [Liveblocks Svelte Examples](https://liveblocks.io/examples/browse/all/svelte)
- [SvelteKit Authentication](https://liveblocks.io/docs/authentication/id-token/sveltekit)
- [Liveblocks vs Supabase Comparison](https://ably.com/compare/liveblocks-broadcast-vs-supabase)
