#!/usr/bin/env node
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const issuesPath = new URL('../.beads/issues.jsonl', import.meta.url);
const templatesDir = new URL('../.beads/templates/', import.meta.url);
const lines = readFileSync(issuesPath, 'utf8')
  .split('\n')
  .filter((line) => line.trim().length > 0);

const issuesById = new Map();

for (const [index, line] of lines.entries()) {
  let record;
  try {
    record = JSON.parse(line);
  } catch (error) {
    console.error(`Invalid JSON on .beads/issues.jsonl:${index + 1}`);
    throw error;
  }

  if (record._type !== 'issue' || record.status === 'closed') {
    continue;
  }

  issuesById.set(record.id, record);
}

const requiredTemplateFiles = ['bug.yaml', 'feature.yaml', 'task.yaml'];
const templateErrors = [];

for (const file of requiredTemplateFiles) {
  const templatePath = new URL(file, templatesDir);
  if (!existsSync(templatePath)) {
    templateErrors.push(`${file}: missing template file`);
    continue;
  }

  const template = readFileSync(templatePath, 'utf8');
  if (!template.includes('## Acceptance Criteria')) {
    templateErrors.push(`${file}: missing ## Acceptance Criteria section`);
  }
  if (!template.includes('acceptance_criteria:')) {
    templateErrors.push(`${file}: missing acceptance_criteria field`);
  }
}

const extraTemplates = readdirSync(templatesDir)
  .filter((file) => file.endsWith('.yaml') || file.endsWith('.yml'))
  .filter((file) => !requiredTemplateFiles.includes(file));

for (const file of extraTemplates) {
  const template = readFileSync(new URL(file, templatesDir), 'utf8');
  if (!template.includes('## Acceptance Criteria') || !template.includes('acceptance_criteria:')) {
    templateErrors.push(`${file}: missing acceptance criteria defaults`);
  }
}

const changedIssueIds = process.env.CULL_CHECK_ALL_BEADS_ACCEPTANCE === '1'
  ? new Set(issuesById.keys())
  : collectChangedIssueIds();

const missing = [];

for (const id of changedIssueIds) {
  const record = issuesById.get(id);
  if (!record) {
    continue;
  }

  const description = String(record.description ?? '');
  const acceptanceCriteria = String(record.acceptance_criteria ?? '').trim();

  if (!acceptanceCriteria || !description.includes('## Acceptance Criteria')) {
    missing.push({
      id: record.id,
      title: record.title,
      reason: [
        acceptanceCriteria ? null : 'missing acceptance_criteria field',
        description.includes('## Acceptance Criteria') ? null : 'missing ## Acceptance Criteria section',
      ].filter(Boolean).join('; '),
    });
  }
}

if (templateErrors.length > 0 || missing.length > 0) {
  if (templateErrors.length > 0) {
    console.error(`Beads templates missing acceptance criteria defaults: ${templateErrors.length}`);
    for (const error of templateErrors) {
      console.error(`- ${error}`);
    }
  }

  if (missing.length > 0) {
    console.error(`Changed open beads issues missing acceptance criteria: ${missing.length}`);
    for (const issue of missing) {
      console.error(`- ${issue.id}: ${issue.title} (${issue.reason})`);
    }
  }

  process.exit(1);
}

if (process.env.CULL_CHECK_ALL_BEADS_ACCEPTANCE === '1') {
  console.log('All open beads issues include acceptance criteria fields and ## Acceptance Criteria sections.');
} else if (changedIssueIds.size > 0) {
  console.log(`Changed open beads issues include acceptance criteria (${changedIssueIds.size} checked).`);
} else {
  console.log('No changed open beads issues to validate; beads templates include acceptance criteria defaults.');
}

function collectChangedIssueIds() {
  const ids = new Set();
  for (const args of diffCommands()) {
    let output = '';
    try {
      output = execFileSync('git', args, { encoding: 'utf8' });
    } catch {
      continue;
    }

    for (const line of output.split('\n')) {
      if (!line.startsWith('+') || line.startsWith('+++')) {
        continue;
      }

      try {
        const record = JSON.parse(line.slice(1));
        if (record._type === 'issue' && record.id) {
          ids.add(record.id);
        }
      } catch {
        // Ignore non-JSON diff context.
      }
    }
  }

  return ids;
}

function diffCommands() {
  const commands = [
    ['diff', '--unified=0', '--', '.beads/issues.jsonl'],
    ['diff', '--cached', '--unified=0', '--', '.beads/issues.jsonl'],
  ];

  try {
    const upstream = execFileSync('git', ['rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{upstream}'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
    if (upstream) {
      commands.push(['diff', '--unified=0', `${upstream}...HEAD`, '--', '.beads/issues.jsonl']);
    }
  } catch {
    // Repositories without an upstream still get working-tree and index checks.
  }

  return commands;
}
