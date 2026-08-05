// The Enter that submits a pasted message must fire only AFTER the CLI has begun
// echoing the paste — never on a fixed delay from the paste write. The rare
// "text landed in the input but never entered" was a `\r` fired before Ink had
// committed the bracketed paste. These drive a fake PTY through the two timings.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { submitTextToSession } from './session-io'
import type { PtySession } from './types'

function fakePty() {
  const writes: string[] = []
  let onDataCb: ((d: string) => void) | null = null
  const pty = {
    write: (d: string) => { writes.push(d) },
    onData: (cb: (d: string) => void) => { onDataCb = cb; return { dispose: () => { onDataCb = null } } },
  }
  return {
    session: { id: 's', state: 'running', pty, lastActivityAt: new Date() } as unknown as PtySession,
    writes,
    echo: (d: string) => onDataCb?.(d),
    enterCount: () => writes.filter((w) => w === '\r').length,
    pasteWritten: () => writes.some((w) => w.includes('\x1b[200~') && w.includes('\x1b[201~')),
  }
}

beforeEach(() => { vi.useFakeTimers() })
afterEach(() => { vi.useRealTimers() })

describe('submitTextToSession — Enter timing', () => {
  it('writes the bracketed paste immediately', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    expect(f.pasteWritten()).toBe(true)
    expect(f.enterCount()).toBe(0) // not yet — paste hasn't been echoed
  })

  it('does NOT press Enter on the 80ms quiet window when the CLI has not echoed yet', () => {
    // The bug: a slow/busy CLI that hasn't echoed the paste within 80ms used to
    // get a premature `\r`. Now nothing arms the quiet timer until the echo.
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    vi.advanceTimersByTime(500) // well past the old 80ms quiet window
    expect(f.enterCount()).toBe(0) // still not entered — no echo seen
  })

  it('presses Enter once, 80ms after the paste echo settles', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    f.echo('hello') // CLI echoes the paste
    vi.advanceTimersByTime(79)
    expect(f.enterCount()).toBe(0) // still within the quiet window
    vi.advanceTimersByTime(2)
    expect(f.enterCount()).toBe(1) // fired after quiet
  })

  it('waits for the echo to STOP — a burst of echo chunks keeps resetting the window', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'a long message rendered in chunks')
    f.echo('a long '); vi.advanceTimersByTime(50)
    f.echo('message '); vi.advanceTimersByTime(50)
    f.echo('rendered'); vi.advanceTimersByTime(50)
    expect(f.enterCount()).toBe(0) // never quiet for a full 80ms yet
    vi.advanceTimersByTime(80)
    expect(f.enterCount()).toBe(1) // now it settled
  })

  it('still enters via the hard cap if the paste produces no echo at all', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    vi.advanceTimersByTime(2000) // HARD_MAX_MS
    expect(f.enterCount()).toBe(1)
  })

  it('never presses Enter more than once', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    f.echo('hello')
    vi.advanceTimersByTime(5000)
    f.echo('more output after submit')
    vi.advanceTimersByTime(5000)
    expect(f.enterCount()).toBe(1)
  })

  it('does nothing for a session that is not running', () => {
    const f = fakePty()
    ;(f.session as { state: string }).state = 'stopped'
    submitTextToSession(f.session, 'hello')
    expect(f.writes).toHaveLength(0)
  })
})
