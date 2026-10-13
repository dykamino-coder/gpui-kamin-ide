import { afterEach, describe, expect, it, vi } from 'vitest'
import { execFileSync } from 'child_process'

import {
  armSwitchConfirm,
  effortCommand,
  isCacheWarningDialog,
  isEffortLevel,
  isSafeModelId,
  modelCommand,
  normalizeScreenText,
  PRE_MODEL_SWITCH_ALLOW_OUTPUT,
  preModelSwitchAllowMatcher,
} from './live-switch'
import type { PtySession } from '../types/pty'

// Dialog bodies as Claude Code 2.1.296 renders them (captured in a tmux PTY).
const EFFORT_DIALOG = [
  '\x1b[1m   Change effort level?\x1b[22m\r\n',
  '   Your next response will be slower and use more tokens\r\n',
  '   This conversation is cached for the current effort level. Switching to \x1b[1mhigh\x1b[22m means the full history gets re-read on your next message.\r\n',
  '   \x1b[36m❯ 1. Yes, switch to high\x1b[39m\r\n',
  '     2. No, go back\r\n',
].join('')
const MODEL_DIALOG =
  '   Switch model?\r\n   This conversation is cached for the current model. Switching to Sonnet 4.6 means …\r\n   ❯ 1. Yes, switch to Sonnet 4.6\r\n'
const HOOK_ASK_DIALOG =
  '   Switch model?\r\n   A PreModelSwitch hook asked you to confirm\r\n   Team policy: ask before Opus\r\n   ❯ 1. Yes, switch to Opus 5.5\r\n'

function fakeSession() {
  const listeners = new Set<(data: string) => void>()
  const session = {
    id: 's1',
    state: 'running',
    pty: {
      onData(cb: (data: string) => void) {
        listeners.add(cb)
        return { dispose: () => listeners.delete(cb) }
      },
    },
  } as unknown as PtySession
  return { session, emit: (data: string) => [...listeners].forEach((cb) => cb(data)), listeners }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('live switch commands', () => {
  it('accepts only the CLI effort levels', () => {
    for (const level of ['low', 'medium', 'high', 'xhigh', 'max']) expect(isEffortLevel(level)).toBe(true)
    for (const bad of ['', 'auto', 'HIGH', 'high\r/exit', undefined, 3]) expect(isEffortLevel(bad)).toBe(false)
  })

  it('rejects model ids that could submit a second prompt line', () => {
    for (const ok of ['claude-sonnet-4-6', 'opus[1m]', 'us.anthropic.claude-opus-4-8-v1:0', 'gw/model@2'])
      expect(isSafeModelId(ok)).toBe(true)
    for (const bad of ['', 'a b', 'x\r/exit', 'x\n', '-flag', 'm'.repeat(200)]) expect(isSafeModelId(bad)).toBe(false)
  })

  it('clears the input line before the slash command', () => {
    expect(effortCommand('xhigh')).toBe('\x15/effort xhigh\r')
    expect(modelCommand('claude-haiku-4-5')).toBe('\x15/model claude-haiku-4-5\r')
  })

  it('PreModelSwitch hook prints an allow decision and drains stdin', () => {
    expect(JSON.parse(PRE_MODEL_SWITCH_ALLOW_OUTPUT)).toEqual({
      hookSpecificOutput: {
        hookEventName: 'PreModelSwitch',
        permissionDecision: 'allow',
        permissionDecisionReason: 'Model switch requested from the Bridge chat',
      },
    })
    const hook = preModelSwitchAllowMatcher().hooks[0]!
    expect(hook.type).toBe('command')
    if (process.platform === 'win32') return // the CLI runs it with sh inside the Linux container
    const out = execFileSync('sh', ['-c', hook.command], { input: '{"from_model":"a","to_model":"b"}' }).toString()
    expect(JSON.parse(out)).toEqual(JSON.parse(PRE_MODEL_SWITCH_ALLOW_OUTPUT))
  })
})

describe('cache-warning dialog detection', () => {
  it('normalizes cursor-positioned full-screen output', () => {
    expect(normalizeScreenText('Change\x1b[1Ceffort\x1b[1Clevel?')).toBe('changeeffortlevel?')
  })

  it('matches the effort dialog only for the requested level', () => {
    const text = normalizeScreenText(EFFORT_DIALOG)
    expect(isCacheWarningDialog(text, 'effort', 'high')).toBe(true)
    expect(isCacheWarningDialog(text, 'effort', 'low')).toBe(false)
    expect(isCacheWarningDialog(text, 'model')).toBe(false)
  })

  it('matches the model cache dialog but never a hook-requested confirmation', () => {
    expect(isCacheWarningDialog(normalizeScreenText(MODEL_DIALOG), 'model')).toBe(true)
    expect(isCacheWarningDialog(normalizeScreenText(HOOK_ASK_DIALOG), 'model')).toBe(false)
  })
})

describe('armSwitchConfirm', () => {
  it('confirms the dialog once when it appears, even split across chunks', () => {
    vi.useFakeTimers()
    const { session, emit, listeners } = fakeSession()
    const write = vi.fn(() => true)
    armSwitchConfirm(session, 'effort', 'high', { write })
    emit(EFFORT_DIALOG.slice(0, 40))
    emit(EFFORT_DIALOG.slice(40))
    vi.advanceTimersByTime(200)
    expect(write).toHaveBeenCalledTimes(1)
    expect(write).toHaveBeenCalledWith('\r')
    expect(listeners.size).toBe(0)
    emit(EFFORT_DIALOG)
    vi.advanceTimersByTime(200)
    expect(write).toHaveBeenCalledTimes(1)
  })

  it('writes nothing when no dialog appears and stops at the TTL', () => {
    vi.useFakeTimers()
    const { session, emit, listeners } = fakeSession()
    const write = vi.fn(() => true)
    armSwitchConfirm(session, 'effort', 'high', { write, ttlMs: 1000 })
    emit('  ⎿  Set effort level to high (saved as your default for new sessions)\r\n')
    vi.advanceTimersByTime(1500)
    expect(write).not.toHaveBeenCalled()
    expect(listeners.size).toBe(0)
  })

  it('a newer switch replaces the armed responder', () => {
    vi.useFakeTimers()
    const { session, emit, listeners } = fakeSession()
    const write = vi.fn(() => true)
    armSwitchConfirm(session, 'effort', 'high', { write })
    armSwitchConfirm(session, 'effort', 'low', { write })
    expect(listeners.size).toBe(1)
    emit(EFFORT_DIALOG) // dialog for "high" is stale now
    vi.advanceTimersByTime(200)
    expect(write).not.toHaveBeenCalled()
  })

  it('does not answer a hook-requested model confirmation', () => {
    vi.useFakeTimers()
    const { session, emit } = fakeSession()
    const write = vi.fn(() => true)
    armSwitchConfirm(session, 'model', undefined, { write })
    emit(HOOK_ASK_DIALOG)
    vi.advanceTimersByTime(200)
    expect(write).not.toHaveBeenCalled()
  })
})
