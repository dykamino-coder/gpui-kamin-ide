// After a live `/effort` the Claude model must learn the new level: its system
// prompt names only the level the chat started with. The next user message
// carries the current level in Bridge's hidden block, once.
import 'reflect-metadata'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { submitTextToSession } from './session-io'
import { setSessionPromptReady } from './session-input-coordinator'
import { forgetEffortChange, noteEffortChanged } from './effort-note'
import type { PtySession } from './types'

function fakeSession(effort: string) {
  const writes: string[] = []
  let echo: ((d: string) => void) | null = null
  // The CLI echoes a paste; the submit presses Enter only after that echo.
  const pty = {
    write: (d: string) => {
      writes.push(d)
      if (d.includes('\x1b[200~')) setTimeout(() => echo?.('echo'), 1)
    },
    onData: (cb: (d: string) => void) => {
      echo = cb
      return { dispose: () => void (echo = null) }
    },
  }
  const session = { id: 's', state: 'running', pty, effort, lastActivityAt: new Date() } as unknown as PtySession
  setSessionPromptReady(session, true)
  /** The text of the last bracketed paste the CLI received. */
  const pasted = (): string => {
    const paste = [...writes].reverse().find((w) => w.includes('\x1b[200~')) ?? ''
    return paste.slice(paste.indexOf('\x1b[200~') + 6, paste.indexOf('\x1b[201~'))
  }
  return { session, writes, pasted }
}

/** Submit one message and let the paste reach the fake PTY. */
function send(session: PtySession, text: string): void {
  // The previous turn is over: the CLI shows its prompt again.
  setSessionPromptReady(session, true)
  submitTextToSession(session, text)
  vi.advanceTimersByTime(3000)
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('Claude effort changed during the chat', () => {
  it('tells the model the new level with the next message, once', () => {
    const f = fakeSession('high')
    send(f.session, 'first')
    expect(f.pasted()).toBe('first')

    // What the session:change-effort live path does after writing `/effort low`.
    f.session.effort = 'low'
    noteEffortChanged(f.session)
    send(f.session, 'which effort do you run with?')
    const engine = f.pasted()
    expect(engine.startsWith('which effort do you run with?')).toBe(true)
    expect(engine).toContain('<bridge-context>')
    expect(engine).toContain('Reasoning effort is now `low`')
    expect(engine).not.toContain('`high`')

    send(f.session, 'and now?')
    expect(f.pasted()).toBe('and now?')
  })

  it('names the level current at delivery after several changes', () => {
    const f = fakeSession('high')
    f.session.effort = 'low'
    noteEffortChanged(f.session)
    f.session.effort = 'max'
    noteEffortChanged(f.session)
    send(f.session, 'hi')
    expect(f.pasted()).toContain('Reasoning effort is now `max`')
    expect(f.pasted()).not.toContain('`low`')
  })

  it('keeps the note for a real message, not a slash command for the CLI', () => {
    const f = fakeSession('high')
    f.session.effort = 'medium'
    noteEffortChanged(f.session)
    send(f.session, '/context')
    expect(f.pasted()).toBe('/context')
    send(f.session, 'hello')
    expect(f.pasted()).toContain('Reasoning effort is now `medium`')
  })

  it('adds nothing after a respawn, whose system prompt already names the level', () => {
    const f = fakeSession('high')
    f.session.effort = 'low'
    noteEffortChanged(f.session)
    forgetEffortChange(f.session)
    send(f.session, 'hello')
    expect(f.pasted()).toBe('hello')
  })
})
