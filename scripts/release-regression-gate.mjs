#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function regressionError(code, message, details) {
  const error = new Error(message);
  error.code = code;
  error.details = details;
  return error;
}

export function loadRegressionContracts(repoRoot = process.cwd()) {
  let config;
  try {
    config = JSON.parse(readFileSync(resolve(repoRoot, 'release.config.json'), 'utf8'));
  } catch (cause) {
    throw regressionError('REGRESSION_CONFIG_INVALID', 'Unable to read release.config.json', {
      cause: cause.message,
    });
  }
  if (!Array.isArray(config.regressionGate?.contracts)) {
    throw regressionError(
      'REGRESSION_CONFIG_INVALID',
      'release.config.json must define regressionGate.contracts',
    );
  }
  return config.regressionGate.contracts;
}

export const regressionContracts = loadRegressionContracts(resolve(import.meta.dirname, '..'));

export function validateRegressionContracts(contracts, pathExists = existsSync) {
  const ids = new Set();
  for (const contract of contracts) {
    if (!contract || typeof contract.id !== 'string' || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(contract.id)
      || !Array.isArray(contract.tests) || contract.tests.length === 0
      || contract.tests.some((path) => typeof path !== 'string' || !path.endsWith('.test.ts'))) {
      throw regressionError('REGRESSION_CONFIG_INVALID', 'Every regression contract needs a stable id and behavior test files', {
        contract,
      });
    }
    if (ids.has(contract.id)) {
      throw regressionError('REGRESSION_CONFIG_INVALID', `Duplicate regression contract ${contract.id}`);
    }
    ids.add(contract.id);
    const missingTests = contract.tests.filter((path) => !pathExists(path));
    if (missingTests.length > 0) {
      throw regressionError(
        'REGRESSION_CONTRACT_MISSING',
        `Release regression contract ${contract.id} is missing required behavior coverage`,
        { contract: contract.id, missingTests },
      );
    }
  }
  return contracts;
}

export function runRegressionContracts(contracts, execute) {
  const passed = [];
  for (const contract of contracts) {
    process.stdout.write(`\n[release-regression] ${contract.id}\n`);
    const result = execute(contract);
    if (result.error || result.status !== 0) {
      throw regressionError(
        'REGRESSION_CONTRACT_FAILED',
        `Release regression contract ${contract.id} failed; run npm run test:release-regressions -- --contract ${contract.id}`,
        { contract: contract.id, tests: contract.tests, status: result.status, signal: result.signal },
      );
    }
    passed.push({ id: contract.id, status: 'passed' });
  }
  return { ok: true, contracts: passed };
}

function parseContract(argv) {
  if (argv.length === 0) return null;
  if (argv.length === 2 && argv[0] === '--contract' && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(argv[1])) {
    return argv[1];
  }
  throw regressionError('INPUT_INVALID', 'Usage: npm run test:release-regressions -- [--contract <id>]');
}

function main() {
  try {
    const repoRoot = resolve(process.cwd());
    const requested = parseContract(process.argv.slice(2));
    let contracts = loadRegressionContracts(repoRoot);
    if (requested !== null) {
      contracts = contracts.filter((contract) => contract.id === requested);
      if (contracts.length === 0) {
        throw regressionError('INPUT_INVALID', `Unknown release regression contract ${requested}`);
      }
    }
    validateRegressionContracts(contracts, (path) => existsSync(resolve(repoRoot, path)));
    const report = runRegressionContracts(contracts, (contract) => spawnSync(
      'npm', ['exec', '--', 'vitest', 'run', ...contract.tests],
      { cwd: repoRoot, stdio: 'inherit' },
    ));
    process.stdout.write(`\n${JSON.stringify(report)}\n`);
  } catch (cause) {
    const failure = {
      ok: false,
      code: cause.code ?? 'INTERNAL_ERROR',
      message: cause.message,
      ...(cause.details === undefined ? {} : { details: cause.details }),
    };
    if (process.env.GITHUB_ACTIONS === 'true') {
      process.stderr.write(`::error title=Cull release regression gate::${failure.code}: ${failure.message}\n`);
    }
    process.stderr.write(`${JSON.stringify(failure)}\n`);
    process.exitCode = cause.code ? 2 : 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) main();
