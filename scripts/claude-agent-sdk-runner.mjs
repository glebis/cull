#!/usr/bin/env node
import { query } from '@anthropic-ai/claude-agent-sdk';
import { readFileSync } from 'node:fs';
import process from 'node:process';

function readStdinJson() {
  const input = readFileSync(0, 'utf8');
  if (!input.trim()) {
    throw new Error('Expected Claude Agent SDK request JSON on stdin');
  }
  return JSON.parse(input);
}

function inheritedEnvWithoutAnthropicKey() {
  const env = { ...process.env };
  delete env.ANTHROPIC_API_KEY;
  env.CLAUDE_AGENT_SDK_CLIENT_APP = env.CLAUDE_AGENT_SDK_CLIENT_APP || 'cull/agent-chat';
  return env;
}

function buildOptions(request) {
  const textOnly = request.visual_level === 'text' || !request.allowed_dirs?.length;
  const options = {
    model: request.model,
    maxBudgetUsd: request.max_budget_usd,
    permissionMode: 'dontAsk',
    persistSession: false,
    env: inheritedEnvWithoutAnthropicKey(),
    pathToClaudeCodeExecutable: request.claude_executable || 'claude',
    cwd: request.cwd || process.cwd(),
  };

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

function parseStructuredOutput(result) {
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

function normalizeResult(messages) {
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

try {
  const request = readStdinJson();
  const messages = [];
  for await (const message of query({
    prompt: request.prompt,
    options: buildOptions(request),
  })) {
    messages.push(message);
  }
  process.stdout.write(`${JSON.stringify(normalizeResult(messages))}\n`);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
