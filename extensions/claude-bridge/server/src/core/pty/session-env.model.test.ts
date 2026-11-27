import 'reflect-metadata'
import { describe, expect, it, vi } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { buildClaudeArgs, buildSessionEnv, claudeEffortLine, modelForResume } from './session-env'
import { lastModelFromJsonlTail } from './session-resume-helpers'
import type { SessionConfig } from '../types/pty'

const session = (model?: string): SessionConfig => ({ cwd: '/repo', model }) as SessionConfig
vi.mock('../config/settings', () => ({ injectProxyEnv: vi.fn() }))

it('trims nonessential traffic and keeps MCP Tasks in the background (CLI 2.1.319+)', () => {
  vi.stubEnv('CLAUDE_CODE_DISABLE_MCP_TASK_BACKGROUND', '1')
  vi.stubEnv('CLAUDE_CODE_DISABLE_MCP_TASKS', '1')
  try {
    const env = buildSessionEnv('synthetic-session', 'synthetic-user')
    expect(env.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC).toBe('1')
    expect(env).not.toHaveProperty('CLAUDE_CODE_DISABLE_MCP_TASK_BACKGROUND')
    expect(env).not.toHaveProperty('CLAUDE_CODE_DISABLE_MCP_TASKS')
    expect(env.CLAUDE_CODE_DISABLE_BACKGROUND_TASKS).toBe('1')
    expect(env.CLAUDE_CODE_NO_FLICKER).toBe('1')
  } finally {
    vi.unstubAllEnvs()
  }
})

describe('Claude CLI thinking display (INC-2026-0098)', () => {
  const display = (args: string[]) => {
    const i = args.indexOf('--thinking-display')
    return i < 0 ? undefined : args[i + 1]
  }

  it('asks for summarized thinking by default, so thinking blocks carry text', () => {
    expect(display(buildClaudeArgs(session()))).toBe('summarized')
    expect(display(buildClaudeArgs({ ...session(), resumeConversationId: 'c' } as SessionConfig))).toBe('summarized')
  })

  it('keeps the CLI display when the session or the server chose it, or the flags name one', () => {
    expect(display(buildClaudeArgs({ ...session(), thinkingDisplay: 'updates' }))).toBeUndefined()
    const own = buildClaudeArgs({ ...session(), extraArgs: ['--thinking-display', 'omitted'] })
    expect(own.filter((arg) => arg === '--thinking-display')).toHaveLength(1)
    expect(display(own)).toBe('omitted')
    vi.stubEnv('CLAUDE_BRIDGE_THINKING_DISPLAY', 'updates')
    try {
      expect(display(buildClaudeArgs(session()))).toBeUndefined()
      expect(display(buildClaudeArgs({ ...session(), thinkingDisplay: 'summarized' }))).toBe('summarized')
    } finally {
      vi.unstubAllEnvs()
    }
  })
})

describe('Claude CLI model selection', () => {
  it('launches new sessions with Opus 5.5 and keeps an explicit user choice', () => {
    const defaultArgs = buildClaudeArgs(session())
    const chosenArgs = buildClaudeArgs(session('claude-sonnet-5-5'))
    const oldOpusArgs = buildClaudeArgs(session('claude-opus-5'))

    expect(defaultArgs[defaultArgs.indexOf('--model') + 1]).toBe('claude-opus-5-5')
    expect(chosenArgs[chosenArgs.indexOf('--model') + 1]).toBe('claude-sonnet-5-5')
    expect(oldOpusArgs[oldOpusArgs.indexOf('--model') + 1]).toBe('claude-opus-5')
  })

  it('lets Claude Code restore the transcript model if the resume model is unavailable', () => {
    const args = buildClaudeArgs({ ...session(), resumeConversationId: 'old-conversation' })
    expect(args).toContain('--resume')
    expect(args).not.toContain('--model')
  })

  it('keeps old-client model choices and supports Fable 5.1 and 5.2 without changing the default', () => {
    for (const model of [
      'claude-opus-4-5-20251101',
      'claude-opus-4-8',
      'claude-opus-5',
      'claude-haiku-4-5',
      'claude-sonnet-5',
      'claude-fable-5',
      'claude-fable-5-1',
      'claude-fable-5-2',
    ]) {
      const args = buildClaudeArgs({ ...session(model), resumeConversationId: 'existing-conversation' })
      expect(args[args.indexOf('--model') + 1]).toBe(model)
      expect(modelForResume(model)).toBe(model)
    }
  })

  it('keeps the existing fallback only for retired Opus 4 and 4.1', () => {
    expect(modelForResume('claude-opus-4-20250514')).toBe('claude-opus-5-5')
    expect(modelForResume('claude-opus-4-1-20250805')).toBe('claude-opus-5-5')
  })

  it('recovers the last real model from a prior transcript', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-model-'))
    const transcript = path.join(dir, 'session.jsonl')
    try {
      const line = (type: string, model?: string) => JSON.stringify({ type, message: { model } })
      fs.writeFileSync(
        transcript,
        [line('assistant', 'claude-opus-5'), line('user'), line('assistant', '<synthetic>'), ''].join('\n'),
      )
      expect(lastModelFromJsonlTail(transcript)).toBe('claude-opus-5')
    } finally {
      fs.rmSync(dir, { recursive: true, force: true })
    }
  })
})

describe('Claude effort line', () => {
  it('points the model at the note a live /effort adds to the next message (effort-note.ts)', () => {
    const line = claudeEffortLine('high')
    expect(line).toContain('started with reasoning effort `high`')
    expect(line).toContain('Bridge note in a user message')
  })
})

it('starts a retained native permission mode directly and rejects ambiguous launch flags', () => {
  const retained: SessionConfig = { ...session(), resumeConversationId: 'existing', claudePermissionMode: 'plan' }
  expect(buildClaudeArgs(session())).toContain('--dangerously-skip-permissions')
  const args = buildClaudeArgs(retained)
  expect(args[args.indexOf('--permission-mode') + 1]).toBe('plan')
  expect(args[args.indexOf('--resume') + 1]).toBe('existing')
  expect(args).not.toContain('--dangerously-skip-permissions')
  for (const flag of [
    '--permission-mode=default',
    '--permission-mode',
    '--dangerously-skip-permissions',
    '--allow-dangerously-skip-permissions',
  ])
    expect(() => buildClaudeArgs({ ...retained, extraArgs: [flag] })).toThrow('Conflicting')
})
