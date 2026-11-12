import { describe, expect, it, vi } from 'vitest'
import type { AgentEvent } from '../../shared/agent-events'
import { ClaudeAgentSession, type ClaudeSessionOps } from './claude-session'
import { agentEngineOrDefault, isAgentEngineId } from './engines'

function fakeOps() {
  let destroyed: ((code: number | null) => void) | undefined
  let alive = true
  const ops = {
    conversationId: vi.fn(() => 'conv-1'),
    exists: vi.fn(() => alive),
    submitText: vi.fn(),
    interrupt: vi.fn(),
    switchModel: vi.fn(() => true),
    destroy: vi.fn(() => {
      alive = false
      destroyed?.(0)
    }),
    onDestroyed: vi.fn((_id: string, l: (code: number | null) => void) => {
      destroyed = l
      return () => {
        destroyed = undefined
      }
    }),
  } satisfies ClaudeSessionOps
  return { ops, fireDestroyed: (code: number | null) => ((alive = false), destroyed?.(code)) }
}

const collect = (s: ClaudeAgentSession) => {
  const events: AgentEvent[] = []
  s.events.subscribe((e) => events.push(e))
  return events
}

describe('ClaudeAgentSession', () => {
  it('delegates the verbs to the existing session functions', async () => {
    const { ops } = fakeOps()
    const s = new ClaudeAgentSession('s1', ops, () => 5)
    expect(s.capabilities).toEqual({ nativeSteer: false, liveModelSwitch: true, normalizedTranscript: false })
    expect(s.conversationId()).toBe('conv-1')
    await s.send('hi', 'sub-1')
    await s.steer('more', 'sub-2')
    expect(ops.submitText.mock.calls).toEqual([
      ['s1', 'hi', 'sub-1'],
      ['s1', 'more', 'sub-2'],
    ])
    await s.interrupt()
    expect(ops.interrupt).toHaveBeenCalledWith('s1')
    await expect(s.setModel('opus')).resolves.toBe('applied')
    ops.switchModel.mockReturnValueOnce(false)
    await expect(s.setModel('sonnet')).resolves.toBe('restart-required')
  })

  it('stop destroys the PTY session and ends the stream once', async () => {
    const { ops } = fakeOps()
    const s = new ClaudeAgentSession('s1', ops, () => 5)
    const events = collect(s)
    await s.stop('explicit_end')
    await s.stop()
    expect(ops.destroy).toHaveBeenCalledTimes(1)
    expect(ops.destroy).toHaveBeenCalledWith('s1', 'explicit_end')
    expect(events).toEqual([
      { kind: 'session.exited', threadId: 'conv-1', at: 5, code: 0, signal: null, reason: 'explicit_end' },
    ])
    await expect(s.send('x')).rejects.toThrow('not running')
    await s.interrupt()
    expect(ops.interrupt).not.toHaveBeenCalled()
  })

  it('maps unknown stop reasons to a valid destroy reason', async () => {
    const { ops } = fakeOps()
    await new ClaudeAgentSession('s1', ops).stop('tab_closed')
    expect(ops.destroy).toHaveBeenCalledWith('s1', 'unspecified')
  })

  it('reports a destruction that happened elsewhere', async () => {
    const { ops, fireDestroyed } = fakeOps()
    ops.conversationId.mockReturnValue(null as unknown as string)
    const s = new ClaudeAgentSession('s1', ops, () => 9)
    const events = collect(s)
    fireDestroyed(137)
    expect(events).toEqual([{ kind: 'session.exited', threadId: 's1', at: 9, code: 137, signal: null }])
    await expect(s.setModel('opus')).rejects.toThrow('not running')
  })
})

describe('engine ids', () => {
  it('accepts known engines and defaults to Claude', () => {
    expect(isAgentEngineId('codex')).toBe(true)
    expect(isAgentEngineId('gpt')).toBe(false)
    expect(agentEngineOrDefault(undefined)).toBe('claude')
    expect(agentEngineOrDefault('codex')).toBe('codex')
  })
})
