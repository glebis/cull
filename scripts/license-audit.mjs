#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = process.cwd();
const GPL_PATTERN = /\b(?:A?GPL|L?GPL|GNU\s+(?:Affero\s+)?General\s+Public\s+License|GNU\s+Lesser\s+General\s+Public\s+License|GNU\s+Library\s+General\s+Public\s+License)\b/i;
const COPYLEFT_LICENSE_PATTERN = /\b(?:AGPL|GPL|LGPL)(?:[-+\w. ]*)/i;
const PERMISSIVE_LICENSE_PATTERN = /\b(?:0BSD|Apache-2\.0|BSD-\d-Clause|ISC|MIT|Unlicense|Zlib|Unicode-3\.0)\b/i;
const IGNORED_DIRS = new Set([
  '.git',
  '.svelte-kit',
  'build',
  'node_modules',
  'src-tauri/target',
  'target',
]);
const SOURCE_EXTENSIONS = new Set([
  '.cjs',
  '.css',
  '.html',
  '.js',
  '.json',
  '.mjs',
  '.rs',
  '.sh',
  '.svelte',
  '.toml',
  '.ts',
  '.yaml',
  '.yml',
]);
const GPL_SCAN_ROOTS = [
  'src',
  'src-tauri/src',
  'scripts',
  'tests',
  'package.json',
  'package-lock.json',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
];

function fail(message, details = []) {
  console.error(`license audit failed: ${message}`);
  for (const detail of details) {
    console.error(`  - ${detail}`);
  }
  process.exitCode = 1;
}

function hasCopyleftRisk(license) {
  if (!COPYLEFT_LICENSE_PATTERN.test(license)) return false;
  if (/\bOR\b/i.test(license) && PERMISSIVE_LICENSE_PATTERN.test(license)) return false;
  return true;
}

function extension(path) {
  const idx = path.lastIndexOf('.');
  return idx === -1 ? '' : path.slice(idx);
}

function walk(dir, files = []) {
  for (const entry of readdirSync(dir)) {
    const absolute = join(dir, entry);
    const rel = relative(ROOT, absolute);
    const normalized = rel.split('\\').join('/');
    if ([...IGNORED_DIRS].some((ignored) => normalized === ignored || normalized.startsWith(`${ignored}/`))) {
      continue;
    }
    const info = statSync(absolute);
    if (info.isDirectory()) {
      walk(absolute, files);
      continue;
    }
    files.push({ absolute, normalized });
  }
  return files;
}

function auditPackageJson() {
  const packageJson = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
  const packageLock = JSON.parse(readFileSync(join(ROOT, 'package-lock.json'), 'utf8'));
  const rootPackage = packageLock.packages?.[''];
  const issues = [];

  if (packageJson.license !== 'Apache-2.0') {
    issues.push(`package.json license is ${packageJson.license}`);
  }
  if (rootPackage?.license !== 'Apache-2.0') {
    issues.push(`package-lock root license is ${rootPackage?.license ?? 'missing'}`);
  }

  const dependencyIssues = [];
  const counts = new Map();
  for (const [path, pkg] of Object.entries(packageLock.packages ?? {})) {
    if (!path) continue;
    const license = String(pkg.license ?? 'NOASSERTION');
    counts.set(license, (counts.get(license) ?? 0) + 1);
    if (license === 'NOASSERTION' || hasCopyleftRisk(license)) {
      dependencyIssues.push(`${path}: ${license}`);
    }
  }

  if (issues.length) {
    fail('npm package metadata is not Apache-2.0', issues);
  }
  if (dependencyIssues.length) {
    fail('npm dependency license issues found', dependencyIssues);
  }

  return {
    total: [...Object.values(packageLock.packages ?? {})].length - 1,
    counts: Object.fromEntries([...counts.entries()].sort()),
  };
}

function auditCargo() {
  const manifestPath = join(ROOT, 'src-tauri', 'Cargo.toml');
  const cargoToml = readFileSync(manifestPath, 'utf8');
  const metadata = JSON.parse(
    execFileSync('cargo', ['metadata', '--locked', '--format-version', '1', '--manifest-path', manifestPath], {
      cwd: ROOT,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'pipe'],
    }),
  );
  const issues = [];

  if (!/^license\s*=\s*"Apache-2\.0"/m.test(cargoToml)) {
    issues.push('src-tauri/Cargo.toml license is not Apache-2.0');
  }

  const rootId = metadata.resolve?.root;
  const counts = new Map();
  const dependencyIssues = [];

  for (const pkg of metadata.packages ?? []) {
    if (pkg.id === rootId) continue;
    const license = pkg.license || (pkg.license_file ? `LicenseRef-file:${pkg.license_file}` : 'NOASSERTION');
    counts.set(license, (counts.get(license) ?? 0) + 1);
    if (license === 'NOASSERTION' || hasCopyleftRisk(license)) {
      dependencyIssues.push(`${pkg.name}@${pkg.version}: ${license}`);
    }
  }

  if (issues.length) {
    fail('Cargo package metadata is not Apache-2.0', issues);
  }
  if (dependencyIssues.length) {
    fail('Cargo dependency license issues found', dependencyIssues.slice(0, 80));
  }

  return {
    total: (metadata.packages ?? []).filter((pkg) => pkg.id !== rootId).length,
    counts: Object.fromEntries([...counts.entries()].sort()),
  };
}

function auditSourceForCopyleftHeaders() {
  const matches = [];
  for (const file of walk(ROOT)) {
    if (file.normalized === 'scripts/license-audit.mjs') continue;
    const inScope = GPL_SCAN_ROOTS.some((scope) => file.normalized === scope || file.normalized.startsWith(`${scope}/`));
    if (!inScope) continue;
    if (!SOURCE_EXTENSIONS.has(extension(file.normalized))) continue;
    const text = readFileSync(file.absolute, 'utf8');
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (GPL_PATTERN.test(line)) {
        matches.push(`${file.normalized}:${index + 1}: ${line.trim().slice(0, 180)}`);
      }
    });
  }

  if (matches.length) {
    fail('GPL-family text found in source files', matches.slice(0, 80));
  }
  return { matches: 0 };
}

function auditModelDownloadPolicy() {
  const files = [
    join(ROOT, 'src-tauri', 'src', 'db_core', 'detection.rs'),
    join(ROOT, 'src-tauri', 'src', 'commands', 'detection.rs'),
  ];
  const forbidden = [
    'github.com/ultralytics/assets/releases',
    'huggingface.co/vladmandic/nudenet',
  ];
  const matches = [];
  for (const file of files) {
    if (!existsSync(file)) continue;
    const rel = relative(ROOT, file);
    const text = readFileSync(file, 'utf8');
    for (const token of forbidden) {
      if (text.includes(token)) {
        matches.push(`${rel}: contains ${token}`);
      }
    }
  }
  if (matches.length) {
    fail('incompatible or unverified built-in model download URL found', matches);
  }
  return { blockedBuiltInDownloads: 0 };
}

const npm = auditPackageJson();
const cargo = auditCargo();
const source = auditSourceForCopyleftHeaders();
const models = auditModelDownloadPolicy();

if (process.exitCode) {
  process.exit(process.exitCode);
}

console.log('license audit passed');
console.log(JSON.stringify({ npm, cargo, source, models }, null, 2));
