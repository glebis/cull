import { execFile } from 'node:child_process';
import { chmod, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

function execFileAsync(
    file: string,
    args: string[],
    options: {
        cwd: string;
        env: Record<string, string | undefined>;
        timeout: number;
        maxBuffer: number;
    },
): Promise<{ stdout: string; stderr: string }> {
    return new Promise((resolve, reject) => {
        execFile(file, args, options, (error, stdout, stderr) => {
            if (error) {
                reject(error);
            } else {
                resolve({ stdout, stderr });
            }
        });
    });
}

function listenOnRandomPort(): Promise<{ server: http.Server; port: number }> {
    return new Promise((resolve, reject) => {
        const server = http.createServer((_req, res) => {
            res.writeHead(200, { 'content-type': 'text/html' });
            res.end('<html><body>not cull mock</body></html>');
        });
        server.once('error', reject);
        server.listen(0, '127.0.0.1', () => {
            const address = server.address();
            if (!address || typeof address === 'string') {
                reject(new Error('server did not expose a TCP port'));
                return;
            }
            resolve({ server, port: address.port });
        });
    });
}

describe('e2e runner', () => {
    it('does not attach to an existing server unless reuse is explicit', async () => {
        const tempDir = await mkdtemp(path.join(os.tmpdir(), 'cull-e2e-runner-'));
        const binDir = path.join(tempDir, 'bin');
        const shotsDir = path.join(tempDir, 'shots');
        const logPath = path.join(tempDir, 'events.log');
        const viteLog = path.join(tempDir, 'vite.log');
        const fakeServer = path.join(tempDir, 'fake-vite.mjs');
        await mkdir(binDir);
        await mkdir(shotsDir);

        await writeFile(fakeServer, `
import http from 'node:http';

const port = Number(process.argv[2]);
const server = http.createServer((_req, res) => {
  res.writeHead(200, { 'content-type': 'text/html' });
  res.end('<html><body>mock vite</body></html>');
});
server.listen(port, '127.0.0.1');
process.on('SIGTERM', () => server.close(() => process.exit(0)));
process.on('SIGINT', () => server.close(() => process.exit(0)));
`);

        const fakeNpx = path.join(binDir, 'npx');
        await writeFile(fakeNpx, `#!/usr/bin/env bash
set -euo pipefail
port=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --port)
      shift
      port="$1"
      ;;
  esac
  shift || true
done
echo "npx:$port" >> "$FAKE_RUNNER_LOG"
exec /usr/bin/env node "$FAKE_VITE_SERVER" "$port"
`);
        await chmod(fakeNpx, 0o755);

        const fakePython = path.join(binDir, 'python3');
        await writeFile(fakePython, `#!/usr/bin/env bash
set -euo pipefail
echo "python:$CULL_E2E_URL" >> "$FAKE_RUNNER_LOG"
exit 0
`);
        await chmod(fakePython, 0o755);

        const occupied = await listenOnRandomPort();
        try {
            const { stdout } = await execFileAsync('bash', ['tests/e2e/run-e2e.sh'], {
                cwd: repoRoot,
                env: {
                    ...process.env,
                    PATH: `${binDir}:${process.env.PATH ?? ''}`,
                    CULL_E2E_PORT: String(occupied.port),
                    CULL_E2E_LOG: viteLog,
                    CULL_E2E_SHOTS: shotsDir,
                    FAKE_RUNNER_LOG: logPath,
                    FAKE_VITE_SERVER: fakeServer,
                },
                timeout: 10_000,
                maxBuffer: 1024 * 1024,
            });

            const log = await readFile(logPath, 'utf8');
            expect(stdout).toContain('Starting Vite');
            expect(log).toMatch(/npx:\d+/);
            expect(log).not.toContain(`npx:${occupied.port}`);
            expect(log).not.toContain(`python:http://127.0.0.1:${occupied.port}`);
        } finally {
            await new Promise<void>((resolve, reject) => {
                occupied.server.close((err) => (err ? reject(err) : resolve()));
            });
        }
    }, 15_000);
});
