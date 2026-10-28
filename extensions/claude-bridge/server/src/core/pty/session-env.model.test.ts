import 'reflect-metadata'
import { describe, expect, it, vi } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { buildClaudeArgs, buildSessionEnv, modelForResume } from './session-env'
import { lastModelFromJsonlTail } from './session-resume-helpers'
import type { SessionConfig } from '../types/pty'

const session = (model?: string): SessionConfig => ({ cwd: '/repo', model }) as SessionConfig
vi.mock('../config/settings', () => ({ injectProxyEnv: vi.fn() }))

it('permits MCP Tasks feature discovery while preserving native-agent and renderer controls', () => {
  vi.stubEnv('CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC', '1')
  vi.stubEnv('DISABLE_NONESSENTIAL_TRAFFIC', '1')
  try {
    const env = buildSessionEnv('synthetic-session', 'synthetic-user')
    expect(env).not.toHaveProperty('CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC')
    expect(env).not.toHaveProperty('DISABLE_NONESSENTIAL_TRAFFIC')
    expect(env.CLAUDE_CODE_DISABLE_BACKGROUND_TASKS).toBe('1')
    expect(env.CLAUDE_CODE_NO_FLICKER).toBe('1')
  } finally {
    vi.unstubAllEnvs()
  }
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

  it('keeps old-client model choices and supports Fable 5.1 without changing the default', () => {
    for (const model of [
      'claude-opus-4-5-20251101',
      'claude-opus-4-8',
      'claude-opus-5',
      'claude-haiku-4-5',
      'claude-sonnet-5',
      'claude-fable-5',
      'claude-fable-5-1',
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
