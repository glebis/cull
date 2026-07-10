# Canvas Editing Contract

Canvas editing is non-destructive. Canvas item geometry, display resize, crop, rotation, and notes are saved in the Canvas v1 document stored in `canvases.layout_json`; they do not mutate source library files.

Destructive source-image edits, such as file-level crop commands outside Canvas, remain separate workflows and must not be inferred from Canvas transforms.

Canvas save paths must also follow the user data safety checklist in `docs/user-data-safety-checklist.md`.

## Saved Data

- Display position and display resize live on `items[].x`, `items[].y`, `items[].width`, and `items[].height`.
- Non-destructive crop and rotation live on `items[].transform.crop` and `items[].transform.rotationDegrees`.
- Item notes/comments live in `annotations[]` with an item target.
- Folder/search/collection scope is a render filter, not a delete signal. Saving a visible subset must preserve off-scope `items[]`, groups, connectors, annotations, viewport, and export settings.

## Export And Agents

- Static Canvas export keeps the full Canvas document in `data/canvas.json`.
- When notes exist, export also writes a read-only `data/annotations.json` sidecar.
- Agents should resolve Canvas IDs with `list_session_canvases` and read transforms plus annotations with `get_canvas_layout`.
