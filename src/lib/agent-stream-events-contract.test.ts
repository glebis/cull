import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const pageSource = readFileSync(join(process.cwd(), 'src/routes/+page.svelte'), 'utf8');
const apiSource = readFileSync(join(process.cwd(), 'src/lib/api.ts'), 'utf8');
const runnerSource = readFileSync(join(process.cwd(), 'scripts/claude-agent-sdk-runner.mjs'), 'utf8');
const serviceSource = readFileSync(join(process.cwd(), 'src-tauri/src/services/claude_agent.rs'), 'utf8');

describe('Claude agent stream event bridge', () => {
    it('streams SDK iterator messages through a prefixed runner channel', () => {
        expect(runnerSource).toContain("const EVENT_PREFIX = 'CULL_AGENT_EVENT '");
        expect(runnerSource).toContain('includePartialMessages: true');
        expect(runnerSource).toContain('emitEvent(message)');
        expect(serviceSource).toContain('const SDK_EVENT_PREFIX: &str = "CULL_AGENT_EVENT "');
        expect(serviceSource).toContain('read_sdk_stderr_events');
        expect(serviceSource).toContain('emitter.emit_sdk_event(value)');
    });

    it('subscribes the app UI to scoped Claude stream events', () => {
        expect(apiSource).toContain('export interface ClaudeAgentStreamEvent');
        expect(pageSource).toContain("listen<ClaudeAgentStreamEvent>('claude-agent:stream-event'");
        expect(pageSource).toContain('activeAgentRequestId');
        expect(pageSource).toContain('request_id: requestId');
        expect(pageSource).toContain('streamEvents={agentStreamEvents}');
    });
});
