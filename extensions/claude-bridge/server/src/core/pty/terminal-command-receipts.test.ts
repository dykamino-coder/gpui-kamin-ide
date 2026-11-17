import { beforeEach, describe, expect, it, vi } from 'vitest'

const sent = vi.hoisted(() => [] as Array<Record<string, unknown>>)
vi.mock('./session-io', () => ({
  sendToClient: (_ws: unknown, msg: Record<string, unknown>) => (sent.push(msg), true),
}))

import type { PtySession } from './types'
import {
  replayTerminalCommandReceipts,
  sessionStartCommand,
  settleTerminalCommand,
  trackTerminalCommand,
} from './terminal-command-receipts'

const A = '0b6e1f1c-5a8a-4c4e-9d43-3f6a3c1f2b10'
const B = '7d2c9e40-1f3b-4a55-8e61-2c9b0d4e6f71'
const session = (): PtySession => ({ id: 's1', ws: null }) as unknown as PtySession
const settledIds = () => sent.map((msg) => msg.id)

describe('terminal command receipts', () => {
  beforeEach(() => {
    sent.length = 0
  })

  it('parses only commands whose completion SessionStart reports', () => {
    expect(sessionStartCommand(`/resume ${A}`)).toEqual({ source: 'resume', target: A })
    expect(sessionStartCommand(`/resume ${A}\n\n<context>`)).toEqual({ source: 'resume', target: A })
    expect(sessionStartCommand('/resume my old chat')).toEqual({ source: 'resume' })
    expect(sessionStartCommand('/clear')).toEqual({ source: 'clear' })
    expect(sessionStartCommand('/model opus')).toBeUndefined()
    expect(sessionStartCommand('/tmp/a.png')).toBeUndefined()
  })

  it('settles /resume only by the conversation it asked for, as a committed receipt', () => {
    const s = session()
    trackTerminalCommand(s, 'r1', `/resume ${A}`, 10)
    settleTerminalCommand(s, 'startup', A)
    settleTerminalCommand(s, 'clear', A)
    settleTerminalCommand(s, 'resume', B)
    expect(sent).toEqual([])
    settleTerminalCommand(s, 'resume', A.toUpperCase())
    expect(sent).toEqual([
      {
        type: 'session:submission',
        sessionId: 's1',
        id: 'r1',
        text: `/resume ${A}`,
        state: 'fallback',
        cancelable: false,
        committed: true,
        createdAt: 10,
      },
    ])
    settleTerminalCommand(s, 'resume', A)
    expect(sent).toHaveLength(1)
  })

  it('settles two identical resumes one SessionStart at a time, oldest first', () => {
    const s = session()
    trackTerminalCommand(s, 'clear', '/clear', 1)
    trackTerminalCommand(s, 'r1', `/resume ${A}`, 2)
    trackTerminalCommand(s, 'r2', `/resume ${A}`, 3)
    settleTerminalCommand(s, 'resume', A)
    expect(settledIds()).toEqual(['r1'])
    settleTerminalCommand(s, 'clear', B)
    settleTerminalCommand(s, 'resume', A)
    expect(settledIds()).toEqual(['r1', 'clear', 'r2'])
  })

  it('ignores ordinary messages and replays completions on reattach', () => {
    const s = session()
    trackTerminalCommand(s, 'p', 'hello there', 1)
    trackTerminalCommand(s, 'm', '/model opus', 1)
    trackTerminalCommand(s, 'c', '/compact', 1)
    settleTerminalCommand(s, 'compact', A)
    expect(settledIds()).toEqual(['c'])
    replayTerminalCommandReceipts(s)
    expect(settledIds()).toEqual(['c', 'c'])
    replayTerminalCommandReceipts(session())
    expect(sent).toHaveLength(2)
  })
})
