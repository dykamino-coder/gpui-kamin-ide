import 'reflect-metadata'
import { describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { buildClaudeArgs } from './session-env'
import { lastModelFromJsonlTail } from './session-resume-helpers'
import type { SessionConfig } from '../types/pty'

const session = (model?: string): SessionConfig => ({ cwd: '/repo', model }) as SessionConfig

describe('Claude CLI model selection', () => {
  it('launches new sessions with Opus 5.5 and keeps an explicit user choice', () => {
    const defaultArgs = buildClaudeArgs(session())
    const chosenArgs = buildClaudeArgs(session('claude-sonnet-5'))
    const oldOpusArgs = buildClaudeArgs(session('claude-opus-5'))

    expect(defaultArgs[defaultArgs.indexOf('--model') + 1]).toBe('claude-opus-5-5')
    expect(chosenArgs[chosenArgs.indexOf('--model') + 1]).toBe('claude-sonnet-5')
    expect(oldOpusArgs[oldOpusArgs.indexOf('--model') + 1]).toBe('claude-opus-5')
  })

  it('lets Claude Code restore the transcript model if the resume model is unavailable', () => {
    const args = buildClaudeArgs({ ...session(), resumeConversationId: 'old-conversation' })
    expect(args).toContain('--resume')
    expect(args).not.toContain('--model')
  })

  it('recovers the last real model from a prior transcript', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-model-'))
    const transcript = path.join(dir, 'session.jsonl')
    try {
      const line = (type: string, model?: string) => JSON.stringify({ type, message: { model } })
      fs.writeFileSync(transcript, [
        line('assistant', 'claude-opus-5'),
        line('user'),
        line('assistant', '<synthetic>'),
        '',
      ].join('\n'))
      expect(lastModelFromJsonlTail(transcript)).toBe('claude-opus-5')
    } finally {
      fs.rmSync(dir, { recursive: true, force: true })
    }
  })
})
