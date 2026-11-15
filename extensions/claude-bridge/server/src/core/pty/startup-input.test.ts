import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clearSessionInputState, setSessionPromptReady } from './session-input-coordinator'
import { submitTextToSession, writeInputToSession } from './session-io'
import type { PtySession } from './types'

function startingCli() {
  const writes: string[] = []
  const session = {
    id: 'starting-cli',
    state: 'running',
    pty: { write: (data: string) => writes.push(data), onData: () => ({ dispose() {} }) },
    ws: { readyState: 0, bufferedAmount: 0, send: vi.fn() },
    detachedAt: null,
    lastActivityAt: new Date(),
  } as unknown as PtySession
  return { session, writes }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('native input during CLI startup', () => {
  it('holds complete multiline messages until the first CLI prompt, preserving their order', () => {
    const { session, writes } = startingCli()
    submitTextToSession(session, 'FIRST\nSECOND\nEND')
    submitTextToSession(session, 'next request')
    vi.advanceTimersByTime(30_000)
    expect(writes).toEqual([])

    setSessionPromptReady(session, true)
    vi.advanceTimersByTime(5_000)
    expect(writes).toEqual([
      '\x15',
      '\x1b[200~FIRST\nSECOND\nEND\x1b[201~',
      '\r',
      '\x15',
      '\x1b[200~next request\x1b[201~',
      '\r',
    ])
  })

  it('lets interrupt cancel messages that are still waiting for startup', () => {
    const { session, writes } = startingCli()
    submitTextToSession(session, 'cancel this')
    writeInputToSession(session, '\x03')
    setSessionPromptReady(session, true)
    vi.advanceTimersByTime(30_000)
    expect(writes).toEqual(['\x03'])
  })

  it('discards waiting messages on teardown even if a late ready notification arrives', () => {
    const { session, writes } = startingCli()
    submitTextToSession(session, 'abandoned')
    clearSessionInputState(session)
    setSessionPromptReady(session, true)
    vi.advanceTimersByTime(30_000)
    expect(writes).toEqual([])
  })

  it('does not treat the interrupt idle notification as the first CLI prompt', () => {
    const { session, writes } = startingCli()
    writeInputToSession(session, '\x03')
    submitTextToSession(session, 'still starting')
    vi.advanceTimersByTime(30_000)
    expect(writes).toEqual(['\x03'])
    setSessionPromptReady(session, true)
    vi.advanceTimersByTime(3_000)
    expect(writes).toEqual(['\x03', '\x15', '\x1b[200~still starting\x1b[201~', '\r'])
  })

  it('keeps native mid-turn queuing available after the CLI has shown its first prompt', () => {
    const { session, writes } = startingCli()
    setSessionPromptReady(session, true)
    setSessionPromptReady(session, false)
    submitTextToSession(session, 'queue during a turn')
    vi.advanceTimersByTime(3_000)
    expect(writes).toEqual(['\x15', '\x1b[200~queue during a turn\x1b[201~', '\r'])
  })
})
