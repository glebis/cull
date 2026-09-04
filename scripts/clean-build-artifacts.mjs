#!/usr/bin/env node
// Removes OS junk files from the SvelteKit build output. `static/` is copied
// verbatim into `build/`, so Finder-created .DS_Store files would otherwise be
// embedded into the shipped Tauri bundle.
import { readdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';

const JUNK = new Set(['.DS_Store', 'Thumbs.db', 'desktop.ini']);
const root = new URL('../build', import.meta.url).pathname;

let removed = 0;

function clean(dir) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory()) {
      clean(path);
    } else if (JUNK.has(entry.name)) {
      rmSync(path);
      removed += 1;
    }
  }
}

clean(root);
if (removed > 0) {
  console.log(`clean-build-artifacts: removed ${removed} junk file(s) from build/`);
}