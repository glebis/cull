# Cull Site

Minimal landing page for `cull.company`.

## Stack

- Vite static site
- Vercel Node Functions in `api/`
- Vercel Blob for low-volume signup records
- Resend for confirmation email

This app is intentionally separate from the desktop app. In Vercel, set the project root directory to `site`.

## Environment Variables

Set these in Vercel for Production. Use matching values in Preview only if preview signups should send real email.

- `BLOB_READ_WRITE_TOKEN`: Vercel Blob read/write token.
- `RESEND_API_KEY`: Resend API key.
- `RESEND_FROM`: verified sender, for example `Cull <hello@cull.company>`.
- `SIGNUP_SECRET`: long random string, at least 24 characters. Treat as long-lived because it determines hashed Blob paths.
- `PUBLIC_SITE_URL`: production origin, `https://cull.company`.

## Resend

Verify the sending domain before production launch. SPF, DKIM, and DMARC should be configured for the domain or subdomain used by `RESEND_FROM`.

## Vercel

Project settings:

- Root Directory: `site`
- Build Command: `npm run build`
- Output Directory: `dist`
- Ignored Build Step: `git diff --quiet HEAD^ HEAD .`

The ignored build step prevents desktop-app-only commits from redeploying the site project.

## Signup Storage

Signup records are stored as private Vercel Blob JSON files:

- pending token records under `signups/pending/`
- pending email index records under `signups/pending-email/`
- confirmed records under `signups/confirmed/`
- rate-limit records under `signups/rate/`

This is not a newsletter database. It is a simple confirmed opt-in capture system. Export later by listing blobs and reading confirmed records.

## Local Commands

```bash
npm install
npm run check
npm test -- --run
npm run build
npm run dev
```
