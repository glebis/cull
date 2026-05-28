# Static Publishing Workflow Review - 2026-05-28

## Scope

Static Publishing is a module-gated publishing workflow, not a Settings sub-page. Settings only enables or disables the module. When enabled, Publish appears immediately before Export in app navigation.

## Scenarios

- Local preview: build a portable static site and serve it on loopback.
- Client review link: set a share URL for QR/link handoff and keep indexing disabled by default.
- Agent handoff: generate the site plus `instructions/CLAUDE.md` for Vercel, S3-compatible storage, or manual edits.
- Static host package: produce `site/` assets that can be uploaded without a writable backend.

## Site Customization

- Site title and description are stored in the manifest and reflected in `index.html`.
- Related links are label/URL pairs shown in the generated page header.
- Search indexing is explicit: private packages write `noindex,nofollow` and `robots.txt` with `Disallow: /`; indexable packages can be crawled by static hosts that respect `robots.txt`.
- The site remains read-only; comments, moderation, and upload endpoints are outside this slice.
