#!/usr/bin/env node
import { query } from '@anthropic-ai/claude-agent-sdk';
import { readFileSync } from 'node:fs';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

export const EVENT_PREFIX = 'CULL_AGENT_EVENT ';

export function readStdinJson() {
  const input = readFileSync(0, 'utf8');
  if (!input.trim()) {
    throw new Error('Expected Claude Agent SDK request JSON on stdin');
  }
  return JSON.parse(input);
}

export function inheritedEnvWithoutAnthropicKey() {
  const env = { ...process.env };
  delete env.ANTHROPIC_API_KEY;
  env.CLAUDE_AGENT_SDK_CLIENT_APP = env.CLAUDE_AGENT_SDK_CLIENT_APP || 'cull/agent-chat';
  return env;
}

export function buildOptions(request) {
  const textOnly = request.visual_level === 'text' || !request.allowed_dirs?.length;
  const options = {
    model: request.model,
    maxBudgetUsd: request.max_budget_usd,
    permissionMode: 'dontAsk',
    persistSession: false,
    includePartialMessages: true,
    env: inheritedEnvWithoutAnthropicKey(),
    pathToClaudeCodeExecutable: request.claude_executable || 'claude',
    cwd: request.cwd || process.cwd(),
  };

  if (request.schema) {
    options.outputFormat = {
      type: 'json_schema',
      schema: request.schema,
    };
  }

  if (textOnly) {
    options.tools = [];
    options.disallowedTools = ['*'];
  } else {
    options.tools = ['Read'];
    options.allowedTools = ['Read'];
    options.additionalDirectories = request.allowed_dirs;
  }

  return options;
}

export function textFromContent(content) {
  if (!Array.isArray(content)) return '';
  return content
    .filter((block) => block?.type === 'text' && typeof block.text === 'string')
    .map((block) => block.text)
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function clipText(text, max = 180) {
  const clean = String(text ?? '').replace(/\s+/g, ' ').trim();
  return clean.length > max ? `${clean.slice(0, max - 1)}...` : clean;
}

export function streamEventSummary(event, subtype, accumulatedText = '') {
  const details = {
    sdk_event_type: 'stream_event',
    subtype,
    raw_event_type: event?.type ?? null,
    content_block_type: event?.content_block?.type ?? null,
    delta_type: event?.delta?.type ?? null,
  };
  if (event?.type === 'content_block_start' && event.content_block?.type === 'tool_use') {
    return {
      phase: 'sdk_tool',
      message: `Using ${event.content_block.name ?? 'tool'}`,
      details: { ...details, tool_name: event.content_block.name ?? null },
    };
  }
  if (event?.type === 'content_block_delta' && event.delta?.type === 'text_delta') {
    const text = clipText(accumulatedText || event.delta.text || 'Writing response');
    return {
      phase: 'sdk_stream',
      message: text,
      details: { ...details, text_length: accumulatedText.length },
    };
  }
  if (event?.type === 'content_block_delta' && event.delta?.type === 'input_json_delta') {
    return {
      phase: 'sdk_tool',
      message: 'Preparing tool input',
      details,
    };
  }
  if (event?.type === 'content_block_stop') {
    return {
      phase: 'sdk_stream',
      message: 'Finished response block',
      details,
    };
  }
  if (event?.type === 'message_stop') {
    return {
      phase: 'sdk_stream',
      message: 'Finished message',
      details,
    };
  }
  return {
    phase: 'sdk_stream',
    message: 'Streaming response',
    details,
  };
}

export function createMessageSummarizer() {
  let assistantText = '';
  return (message) => {
    if (message.type === 'stream_event'
      && message.event?.type === 'content_block_delta'
      && message.event.delta?.type === 'text_delta'
      && typeof message.event.delta.text === 'string') {
      assistantText += message.event.delta.text;
    }
    if (message.type === 'result') {
      assistantText = '';
    }
    return summarizeMessage(message, assistantText);
  };
}

export function summarizeMessage(message, accumulatedText = '') {
  const subtype = message.subtype ?? message.event?.type ?? null;
  switch (message.type) {
    case 'system':
      if (message.subtype === 'init') {
        return {
          phase: 'sdk_init',
          message: `Claude SDK ready with ${message.tools?.length ?? 0} tools`,
          details: { sdk_event_type: message.type, subtype, model: message.model ?? null },
        };
      }
      if (message.subtype === 'status') {
        return {
          phase: 'sdk_status',
          message: message.status ? `Claude is ${message.status}` : 'Claude status updated',
          details: { sdk_event_type: message.type, subtype, status: message.status ?? null },
        };
      }
      if (message.subtype === 'thinking_tokens') {
        return {
          phase: 'sdk_thinking',
          message: 'Thinking',
          details: { sdk_event_type: message.type, subtype, estimated_tokens: message.estimated_tokens ?? null },
        };
      }
      if (message.subtype === 'api_retry') {
        return {
          phase: 'sdk_retry',
          message: `Retrying Claude API (${message.attempt}/${message.max_retries})`,
          details: { sdk_event_type: message.type, subtype, attempt: message.attempt, max_retries: message.max_retries },
        };
      }
      return {
        phase: 'sdk_system',
        message: subtype ? `SDK ${subtype}` : 'SDK system event',
        details: { sdk_event_type: message.type, subtype },
      };
    case 'stream_event': {
      return streamEventSummary(message.event, subtype, accumulatedText);
    }
    case 'assistant': {
      const text = clipText(textFromContent(message.message?.content), 180);
      return {
        phase: 'sdk_assistant',
        message: text || 'Assistant message received',
        details: { sdk_event_type: message.type, subtype, request_id: message.request_id ?? null },
      };
    }
    case 'tool_progress':
      return {
        phase: 'sdk_tool',
        message: `${message.tool_name ?? 'Tool'} running ${Math.round(message.elapsed_time_seconds ?? 0)}s`,
        details: { sdk_event_type: message.type, subtype, tool_name: message.tool_name ?? null },
      };
    case 'tool_use_summary':
      return {
        phase: 'sdk_tool',
        message: clipText(message.summary || 'Tool use summarized'),
        details: { sdk_event_type: message.type, subtype },
      };
    case 'result':
      return {
        phase: message.is_error ? 'sdk_error' : 'sdk_result',
        message: message.is_error ? 'Claude returned an execution error' : 'Claude returned a result',
        details: {
          sdk_event_type: message.type,
          subtype,
          duration_ms: message.duration_ms ?? null,
          total_cost_usd: message.total_cost_usd ?? null,
        },
      };
    default:
      return {
        phase: 'sdk_event',
        message: `SDK ${message.type ?? 'event'}`,
        details: { sdk_event_type: message.type ?? null, subtype },
      };
  }
}

export function emitEvent(message, summarize = summarizeMessage) {
  process.stderr.write(`${EVENT_PREFIX}${JSON.stringify(summarize(message))}\n`);
}

export function parseStructuredOutput(result) {
  if (result.structured_output) return result.structured_output;
  const text = result.result ?? '';
  try {
    return JSON.parse(text);
  } catch {
    const fenced = text.match(/```(?:json)?\s*([\s\S]*?)```/i);
    if (fenced) return JSON.parse(fenced[1]);
    const object = text.match(/\{[\s\S]*\}/);
    if (object) return JSON.parse(object[0]);
    throw new Error('Claude Agent SDK result did not contain parseable JSON intent');
  }
}

export function normalizeResult(messages) {
  const result = [...messages].reverse().find((message) => message.type === 'result');
  if (!result) {
    throw new Error('Claude Agent SDK did not emit a result message');
  }
  const structuredOutput = result.is_error ? null : parseStructuredOutput(result);
  return {
    runtime: 'claude_agent_sdk',
    structured_output: structuredOutput,
    usage: result.usage ?? {},
    modelUsage: result.modelUsage ?? {},
    total_cost_usd: result.total_cost_usd ?? null,
    result: result.result ?? null,
    is_error: result.is_error ?? false,
    message_count: messages.length,
  };
}

export async function main() {
  const request = readStdinJson();
  const messages = [];
  const summarize = createMessageSummarizer();
  for await (const message of query({
    prompt: request.prompt,
    options: buildOptions(request),
  })) {
    messages.push(message);
    emitEvent(message, summarize);
  }
  process.stdout.write(`${JSON.stringify(normalizeResult(messages))}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${message}\n`);
    process.exit(1);
  });
}
