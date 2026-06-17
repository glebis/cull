# Cull Site Landing Page Design

## Goal

Build a minimal landing page for `cull.company` that turns LinkedIn traffic into confirmed launch-interest email signups.

## Decisions

- Store the site in this repository under `site/` as a separate Vercel app.
- Keep the desktop app package, Tauri config, and SvelteKit routes untouched.
- Build `/site` as a plain Vite static site with Vercel Node Functions, not a second SvelteKit app.
- Use Vercel Functions for the signup and confirmation endpoints.
- Use Vercel Blob for low-volume signup persistence.
- Use Resend only to send transactional confirmation emails.
- Do not add Neon or any other database service for v1.
- Use confirmed opt-in before treating an email as subscribed.

## Site Scope

The first version is a single page with plain, technical copy. It should make a limited number of claims:

- Cull is a local-first desktop workflow for image culling.
- Cull is keyboard-first for fast review.
- Cull is agent-first through CLI and MCP surfaces.
- Cull is planned as an open source release.

The page should avoid startup language, AGI claims, and long manifesto copy. It can include one short founder note adapted from the LinkedIn post: Cull came from learning enough engineering to make agents useful inside deterministic software, not from pure vibe coding.

## Visual Direction

Use the app's Tokyo Night-inspired style:

- dark background
- monospace typography
- restrained borders
- small radii
- blue, green, orange, purple, and red accents used sparingly

The first viewport should identify Cull immediately and include the email signup form. The page can reuse existing `site/public/images` assets for one quiet product proof panel, but screenshots are optional if they make the page feel less minimal.

## Signup Flow

1. User enters an email address on the landing page.
2. `POST /api/subscribe` validates and normalizes the email.
3. The API applies basic per-IP and per-email rate limits using Vercel Blob records.
4. The API creates a random token, stores a pending signup record in Vercel Blob, and sends a confirmation email with Resend.
5. The page shows a check-your-email state.
6. User clicks the confirmation link.
7. `GET /api/confirm?token=...` verifies the token, checks expiry, writes a confirmed record, and returns a confirmed page state.
8. Duplicate confirmed signups return a calm already-subscribed state.
9. Expired or invalid tokens return a page that lets the user request a fresh confirmation.

## Blob Storage Shape

Use object storage paths that avoid raw email addresses in filenames:

- pending records: `signups/pending/<token_hash>.json`
- confirmed records: `signups/confirmed/<email_hash>.json`
- rate-limit records: `signups/rate/<scope>/<bucket>.json`

Pending records may contain the normalized email because the confirmation flow needs it. Confirmed records may contain the normalized email, timestamps, source, and user agent metadata. Export can be implemented later by listing blobs.

## Environment

Required environment variables:

- `BLOB_READ_WRITE_TOKEN`
- `RESEND_API_KEY`
- `RESEND_FROM`
- `SIGNUP_SECRET`
- `PUBLIC_SITE_URL`

## Error Handling

- Invalid email: return a clear inline form error.
- Duplicate pending email: resend or refresh the confirmation email rather than creating many pending records.
- Duplicate confirmed email: return already subscribed.
- Rate-limited requests: return a calm temporary failure message.
- Resend failure: keep the pending record and return a temporary failure message.
- Blob failure: return a temporary failure message and do not send an email.
- Expired token: return expired state with a link back to the signup form.

## Testing

Add focused tests for:

- email normalization and validation
- deterministic hashing with `SIGNUP_SECRET`
- token expiry behavior
- subscribe rate limiting
- duplicate pending and confirmed handling
- subscribe and confirm API behavior using mocked Blob and Resend boundaries

Run the site build and a manual browser check before shipping.

## Deployment

Deploy `/site` as its own Vercel project attached to `cull.company`. Configure the Vercel project root directory to `site`, set the required environment variables in Vercel, and use an ignored build step so desktop-app-only commits do not redeploy the site. The site should not share the root app's Tauri build process.

Resend must have a verified sender domain before production launch. `SIGNUP_SECRET` should be treated as long-lived because rotating it changes email-hash lookup paths.
