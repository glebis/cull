# Preview Display

Preview Display is a separate presentation surface controlled by the main Cull window. Use it when the main app should stay private while a client, reviewer, or second screen sees only the current image and selected status information.

## Native Display Window

Open it from **View > Preview Display** or press `Cmd+Shift+D`.

The native window is titled **Cull Preview Display** and uses the `preview-display` window label. Reopening the menu item focuses the existing window instead of creating duplicates. The main Cull window remains the control surface: grid, loupe, compare, ratings, decisions, and focus changes drive the display.

Use **View > Move Preview Display to Display...** to place it on another monitor, including an iPad used as a macOS Sidecar display. Use **View > Fullscreen Preview Display** for presentation mode on the selected display.

## Presentation Controls

The View menu includes:

- **Freeze Preview Display**: hold the currently displayed image while navigating privately in the main window.
- **Blank Preview Display**: hide the image entirely without changing library state.
- **Image Only**, **Client Review**, and **Metadata Review** presets: choose whether the display shows only the image or includes filename, rating, decision, and metadata rail fields.
- Field toggles for filename, rating, decision, dimensions, format, source, prompt, tags, and histogram.
- Bounded rail controls for left/right placement, narrow/medium/wide width, and small/medium/large text.

These controls do not mutate images, ratings, selections, collections, or files. They only change presentation state.

The histogram panel uses the same real RGB/luma histogram path as Loupe. For formats that need a generated preview, including RAW and platform-only formats, Cull uses the same thumbnail fallback path documented for previews instead of inventing histogram data from color metrics.

## Web Stream

Use **View > Start Preview Display Web Stream** to create a live localhost browser URL. Use **View > Start Preview Display LAN Web Stream** when an iPad or other device on the same local network should connect. Cull copies the URL automatically. The URL contains a per-session token and remains valid until **View > Stop Preview Display Web Stream** is used or the app exits.

The stream is one-way: browser viewers follow the same focused image and Preview Display presets as the native window, but cannot curate, delete, move, or edit library data. The viewer polls Cull for JSON state and loads images through token-gated image endpoints.

Security notes:

- Treat the URL as a secret. Anyone on the reachable network with the full tokenized LAN URL can view the streamed preview, and clipboard-history tools may retain copied URLs.
- The stream uses `no-store`, `noindex`, and token checks, but it is not a replacement for a hosted proofing portal.
- Static Publishing remains localhost-only by default; Preview Display's default web stream is localhost-only, and the LAN web stream is a separate explicit local-network action.

## Limitations

- Sidecar, AirPlay, and local-network browser viewing can introduce latency, compression, disconnects, and non-reference color. Do not treat Preview Display as color-critical approval.
- The web stream serves browser-readable source files when possible and falls back to Cull thumbnails for unsupported formats such as RAW previews.
- Liveblocks-style collaboration, client comments, and multi-user feedback are future work, not part of the current one-way stream.
