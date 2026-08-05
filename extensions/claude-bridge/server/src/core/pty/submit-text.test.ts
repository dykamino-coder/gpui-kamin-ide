import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import {
  getSessionInputSnapshot,
  notifySessionAttachmentChanged,
  requestMaintenanceSubmission,
  setSessionPromptReady,
  submitTextToSession,
  writeInputToSession,
} from './session-io'
import type { PtySession } from './types'

const CLEAR_SETTLE_MS = 50
const ECHO_QUIET_MS = 80
const POST_ENTER_SETTLE_MS = 80
const MAINTENANCE_QUIET_MS = 300

function fakePty() {
  const writes: string[] = []
  let onDataCb: ((d: string) => void) | null = null
  const pty = {
    write: (d: string) => { writes.push(d) },
    onData: (cb: (d: string) => void) => {
      onDataCb = cb
      return { dispose: () => { if (onDataCb === cb) onDataCb = null } }
    },
  }
  const session = {
    id: 's',
    state: 'running',
    pty,
    ws: { readyState: 0, bufferedAmount: 0, send: vi.fn() },
    lastActivityAt: new Date(),
    detachedAt: null,
  } as unknown as PtySession
  return {
    session,
    writes,
    echo: (d: string) => onDataCb?.(d),
    enterCount: () => writes.filter((w) => w === '\r').length,
    pastes: () => writes.filter((w) => w.startsWith('\x1b[200~')),
  }
}

function advanceToPaste(): void {
  vi.advanceTimersByTime(CLEAR_SETTLE_MS)
}

function settleEcho(f: ReturnType<typeof fakePty>, echo = 'echo'): void {
  f.echo(echo)
  vi.advanceTimersByTime(ECHO_QUIET_MS)
}

function finishPostEnterSettle(): void {
  vi.advanceTimersByTime(POST_ENTER_SETTLE_MS)
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-08-05T00:00:00Z'))
})
afterEach(() => { vi.useRealTimers() })

describe('PTY input coordinator', () => {
  it('owns clear, bracketed paste and Enter as one transaction', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')

    expect(f.writes).toEqual(['\x15'])
    vi.advanceTimersByTime(CLEAR_SETTLE_MS - 1)
    expect(f.pastes()).toHaveLength(0)
    vi.advanceTimersByTime(1)
    expect(f.pastes()).toEqual(['\x1b[200~hello\x1b[201~'])
    expect(f.enterCount()).toBe(0)

    settleEcho(f, 'hello')
    expect(f.writes).toEqual(['\x15', '\x1b[200~hello\x1b[201~', '\r'])
  })

  it('does not press Enter before the CLI echoes the paste', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    advanceToPaste()
    vi.advanceTimersByTime(500)
    expect(f.enterCount()).toBe(0)
  })

  it('waits until a burst of paste echo goes quiet', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'a long message rendered in chunks')
    advanceToPaste()
    f.echo('a long '); vi.advanceTimersByTime(50)
    f.echo('message '); vi.advanceTimersByTime(50)
    f.echo('rendered'); vi.advanceTimersByTime(50)
    expect(f.enterCount()).toBe(0)
    vi.advanceTimersByTime(ECHO_QUIET_MS)
    expect(f.enterCount()).toBe(1)
  })

  it('uses the hard cap when the paste produces no echo', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'hello')
    advanceToPaste()
    vi.advanceTimersByTime(1999)
    expect(f.enterCount()).toBe(0)
    vi.advanceTimersByTime(1)
    expect(f.enterCount()).toBe(1)
  })

  it('serializes two complete submissions without concatenating their pastes', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'first')
    submitTextToSession(f.session, 'second')

    advanceToPaste()
    settleEcho(f, 'first')
    finishPostEnterSettle()
    expect(f.writes).toEqual([
      '\x15', '\x1b[200~first\x1b[201~', '\r',
      '\x15',
    ])

    advanceToPaste()
    settleEcho(f, 'second')
    expect(f.writes).toEqual([
      '\x15', '\x1b[200~first\x1b[201~', '\r',
      '\x15', '\x1b[200~second\x1b[201~', '\r',
    ])
  })

  it('buffers raw terminal input until the active submission finishes', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'message')
    advanceToPaste()
    writeInputToSession(f.session, 'typed-later')
    expect(f.writes).not.toContain('typed-later')

    settleEcho(f, 'message')
    finishPostEnterSettle()
    expect(f.writes.slice(-2)).toEqual(['\r', 'typed-later'])
    expect(getSessionInputSnapshot(f.session).rawInputDirty).toBe(true)
  })

  it('lets Ctrl+C bypass the transaction and cancels delayed Enter and queued sends', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'first')
    submitTextToSession(f.session, 'second')
    advanceToPaste()
    f.echo('first')

    writeInputToSession(f.session, '\x03')
    vi.advanceTimersByTime(5000)

    expect(f.enterCount()).toBe(0)
    expect(f.pastes()).toEqual(['\x1b[200~first\x1b[201~'])
    expect(f.writes.at(-1)).toBe('\x03')
    expect(getSessionInputSnapshot(f.session).queuedSubmissions).toBe(0)
  })

  it('keeps reload-skills pending while raw console text survives detach/reattach', () => {
    const f = fakePty()
    setSessionPromptReady(f.session, true)
    writeInputToSession(f.session, 'abc')
    ;(f.session as { detachedAt: Date | null }).detachedAt = new Date()
    notifySessionAttachmentChanged(f.session)
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')

    vi.advanceTimersByTime(5000)
    expect(f.pastes()).toHaveLength(0)
    expect(getSessionInputSnapshot(f.session)).toMatchObject({
      rawInputDirty: true,
      pendingMaintenance: ['reload-skills'],
    })

    ;(f.session as { detachedAt: Date | null }).detachedAt = null
    notifySessionAttachmentChanged(f.session)
    vi.advanceTimersByTime(5000)
    expect(f.pastes()).toHaveLength(0)

    // User submits the preserved line. The reload remains pending until the
    // deterministic Stop hook says the next prompt is ready.
    writeInputToSession(f.session, '\r')
    setSessionPromptReady(f.session, true)
    vi.advanceTimersByTime(MAINTENANCE_QUIET_MS)
    advanceToPaste()
    expect(f.pastes()).toEqual(['\x1b[200~/reload-skills\x1b[201~'])
  })

  it('does not steal the prompt between an older client Ctrl+U and submitText frame', () => {
    const f = fakePty()
    setSessionPromptReady(f.session, true)
    writeInputToSession(f.session, '\x15')
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')

    vi.advanceTimersByTime(50)
    submitTextToSession(f.session, 'user message')
    advanceToPaste()
    expect(f.pastes()).toEqual(['\x1b[200~user message\x1b[201~'])

    settleEcho(f, 'user message')
    finishPostEnterSettle()
    vi.advanceTimersByTime(5000)
    expect(f.pastes()).toEqual(['\x1b[200~user message\x1b[201~'])

    setSessionPromptReady(f.session, true)
    vi.advanceTimersByTime(MAINTENANCE_QUIET_MS)
    advanceToPaste()
    expect(f.pastes()).toEqual([
      '\x1b[200~user message\x1b[201~',
      '\x1b[200~/reload-skills\x1b[201~',
    ])
  })

  it('does nothing for a session that is not running', () => {
    const f = fakePty()
    ;(f.session as { state: string }).state = 'exited'
    submitTextToSession(f.session, 'hello')
    writeInputToSession(f.session, 'raw')
    expect(f.writes).toHaveLength(0)
  })
})
