import { describe, expect, it, vi } from 'vitest';

import {
  buildOptions,
  createMessageSummarizer,
  normalizeResult,
  parseStructuredOutput,
  streamEventSummary,
} from './claude-agent-sdk-runner.mjs';

describe('claude agent sdk runner helpers', () => {
  it('passes schema output format and subscription auth env to the SDK', () => {
    vi.stubEnv('ANTHROPIC_API_KEY', 'should-not-leak');
    const schema = { type: 'object', properties: { operation: { type: 'string' } } };
    const options = buildOptions({
      model: 'opus',
      max_budget_usd: 1.25,
      visual_level: 'tiny',
      allowed_dirs: ['/tmp/cull/thumbs'],
      schema,
      prompt: 'select images',
    });

    expect(options.outputFormat).toEqual({ type: 'json_schema', schema });
    expect(options.allowedTools).toEqual(['Read']);
    expect(options.additionalDirectories).toEqual(['/tmp/cull/thumbs']);
    expect(options.env.ANTHROPIC_API_KEY).toBeUndefined();
    expect(options.env.CLAUDE_AGENT_SDK_CLIENT_APP).toBe('cull/agent-chat');
    vi.unstubAllEnvs();
  });

  it('hard-disables tools for text-only requests', () => {
    const options = buildOptions({
      model: 'haiku',
      visual_level: 'text',
      allowed_dirs: ['/tmp/cull/thumbs'],
    });

    expect(options.tools).toEqual([]);
    expect(options.disallowedTools).toEqual(['*']);
    expect(options.allowedTools).toBeUndefined();
  });

  it('summarizes streaming text with accumulated assistant content', () => {
    const summarize = createMessageSummarizer();

    expect(summarize({
      type: 'stream_event',
      event: { type: 'content_block_delta', delta: { type: 'text_delta', text: 'Selected ' } },
    }).message).toBe('Selected');

    expect(summarize({
      type: 'stream_event',
      event: { type: 'content_block_delta', delta: { type: 'text_delta', text: 'the strongest image.' } },
    }).message).toBe('Selected the strongest image.');
  });

  it('keeps useful status details for retry and tool stream events', () => {
    expect(streamEventSummary({
      type: 'content_block_start',
      content_block: { type: 'tool_use', name: 'Read' },
    }, 'content_block_start')).toMatchObject({
      phase: 'sdk_tool',
      message: 'Using Read',
      details: { tool_name: 'Read' },
    });
  });

  it('parses structured output from direct, fenced, and trailing JSON results', () => {
    expect(parseStructuredOutput({ structured_output: { operation: 'answer' } })).toEqual({ operation: 'answer' });
    expect(parseStructuredOutput({ result: '```json\n{"operation":"answer"}\n```' })).toEqual({ operation: 'answer' });
    expect(parseStructuredOutput({ result: 'I will answer with JSON: {\"operation\":\"answer\"}' })).toEqual({ operation: 'answer' });
    expect(() => parseStructuredOutput({ result: 'plain prose' })).toThrow('parseable JSON intent');
  });

  it('normalizes result messages and preserves error envelopes without parsing prose', () => {
    const ok = normalizeResult([
      { type: 'assistant', message: { content: [{ type: 'text', text: 'working' }] } },
      {
        type: 'result',
        structured_output: { operation: 'answer', message: 'Done' },
        usage: { input_tokens: 1 },
        modelUsage: { haiku: { input_tokens: 1 } },
        total_cost_usd: 0.01,
        result: '{"operation":"answer","message":"Done"}',
        is_error: false,
      },
    ]);
    expect(ok.structured_output).toEqual({ operation: 'answer', message: 'Done' });
    expect(ok.message_count).toBe(2);

    const errored = normalizeResult([
      { type: 'result', is_error: true, result: 'tool failed', usage: {}, modelUsage: {} },
    ]);
    expect(errored.structured_output).toBeNull();
    expect(errored.is_error).toBe(true);
    expect(errored.result).toBe('tool failed');
  });
});
