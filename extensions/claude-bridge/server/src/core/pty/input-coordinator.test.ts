import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getSessionInputSnapshot,
  notifySessionAttachmentChanged,
  requestMaintenanceSubmission,
  setSessionPromptReady,
} from './session-input-coordinator'
import { submitTextToSession, writeInputToSession } from './session-io'
import type { PtySession } from './types'

const CLEAR_SETTLE_MS = 50
const ECHO_QUIET_MS = 80
const POST_ENTER_SETTLE_MS = 80
const MAINTENANCE_QUIET_MS = 300

function fakePty() {
  const writes: string[] = []
  let onDataCb: ((data: string) => void) | null = null
  const pty = {
    write: (data: string) => {
      writes.push(data)
    },
    onData: (callback: (data: string) => void) => {
      onDataCb = callback
      return {
        dispose: () => {
          if (onDataCb === callback) onDataCb = null
        },
      }
    },
  }
  const session = {
    id: 'session-1',
    state: 'running',
    pty,
    ws: { readyState: 0, bufferedAmount: 0, send: vi.fn() },
    lastActivityAt: new Date(),
    detachedAt: null,
  } as unknown as PtySession
  return {
    session,
    writes,
    echo: (data: string) => onDataCb?.(data),
    pastes: () => writes.filter((write) => write.startsWith('\x1b[200~')),
  }
}

function finishSubmission(f: ReturnType<typeof fakePty>, echo: string): void {
  vi.advanceTimersByTime(CLEAR_SETTLE_MS)
  f.echo(echo)
  vi.advanceTimersByTime(ECHO_QUIET_MS + POST_ENTER_SETTLE_MS)
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-08-05T00:00:00Z'))
})
afterEach(() => {
  vi.useRealTimers()
})

describe('PTY input coordinator', () => {
  it('serializes two submissions as separate clear/paste/Enter transactions', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'first')
    submitTextToSession(f.session, 'second')

    finishSubmission(f, 'first')
    expect(f.writes).toEqual(['\x15', '\x1b[200~first\x1b[201~', '\r', '\x15'])
    finishSubmission(f, 'second')
    expect(f.pastes()).toEqual(['\x1b[200~first\x1b[201~', '\x1b[200~second\x1b[201~'])
  })

  it('buffers raw input until a semantic submission has finished', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'message')
    vi.advanceTimersByTime(CLEAR_SETTLE_MS)
    writeInputToSession(f.session, 'typed-later')
    expect(f.writes).not.toContain('typed-later')

    f.echo('message')
    vi.advanceTimersByTime(ECHO_QUIET_MS + POST_ENTER_SETTLE_MS)
    expect(f.writes.slice(-2)).toEqual(['\r', 'typed-later'])
    expect(getSessionInputSnapshot(f.session).rawInputDirty).toBe(true)
  })

  it('lets Ctrl+C bypass the transaction and cancel delayed/queued sends', () => {
    const f = fakePty()
    submitTextToSession(f.session, 'first')
    submitTextToSession(f.session, 'second')
    vi.advanceTimersByTime(CLEAR_SETTLE_MS)
    f.echo('first')

    writeInputToSession(f.session, '\x03')
    vi.advanceTimersByTime(5000)

    expect(f.writes).not.toContain('\r')
    expect(f.pastes()).toEqual(['\x1b[200~first\x1b[201~'])
    expect(f.writes.at(-1)).toBe('\x03')
    expect(getSessionInputSnapshot(f.session).queuedSubmissions).toBe(0)
  })

  it('keeps reload pending while console text survives detach and reattach', () => {
    const f = fakePty()
    setSessionPromptReady(f.session, true)
    writeInputToSession(f.session, 'console-draft')
    ;(f.session as { detachedAt: Date | null }).detachedAt = new Date()
    notifySessionAttachmentChanged(f.session)
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')

    vi.advanceTimersByTime(5000)
    ;(f.session as { detachedAt: Date | null }).detachedAt = null
    notifySessionAttachmentChanged(f.session)
    vi.advanceTimersByTime(5000)

    expect(f.pastes()).toHaveLength(0)
    expect(getSessionInputSnapshot(f.session)).toMatchObject({
      rawInputDirty: true,
      pendingMaintenance: ['reload-skills'],
    })

    writeInputToSession(f.session, '\r')
    setSessionPromptReady(f.session, true)
    vi.advanceTimersByTime(MAINTENANCE_QUIET_MS + CLEAR_SETTLE_MS)
    expect(f.pastes()).toEqual(['\x1b[200~/reload-skills\x1b[201~'])
  })

  it('does not start newly requested maintenance while the session is detached', () => {
    const f = fakePty()
    setSessionPromptReady(f.session, true)
    ;(f.session as { detachedAt: Date | null }).detachedAt = new Date()
    notifySessionAttachmentChanged(f.session)
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')
    vi.advanceTimersByTime(5000)

    expect(f.writes).toHaveLength(0)
    expect(getSessionInputSnapshot(f.session).pendingMaintenance).toEqual(['reload-skills'])

    ;(f.session as { detachedAt: Date | null }).detachedAt = null
    notifySessionAttachmentChanged(f.session)
    vi.advanceTimersByTime(CLEAR_SETTLE_MS)
    expect(f.pastes()).toEqual(['\x1b[200~/reload-skills\x1b[201~'])
  })

  it('reserves the prompt between an old client Ctrl+U and submitText frame', () => {
    const f = fakePty()
    setSessionPromptReady(f.session, true)
    writeInputToSession(f.session, '\x15')
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')

    vi.advanceTimersByTime(50)
    submitTextToSession(f.session, 'user message')
    finishSubmission(f, 'user message')
    vi.advanceTimersByTime(5000)
    expect(f.pastes()).toEqual(['\x1b[200~user message\x1b[201~'])

    setSessionPromptReady(f.session, true)
    vi.advanceTimersByTime(MAINTENANCE_QUIET_MS + CLEAR_SETTLE_MS)
    expect(f.pastes()).toEqual(['\x1b[200~user message\x1b[201~', '\x1b[200~/reload-skills\x1b[201~'])
  })

  it('coalesces duplicate maintenance requests and preserves a newer revision', () => {
    const f = fakePty()
    setSessionPromptReady(f.session, true)
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')
    vi.advanceTimersByTime(MAINTENANCE_QUIET_MS + CLEAR_SETTLE_MS)
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')

    f.echo('/reload-skills')
    vi.advanceTimersByTime(ECHO_QUIET_MS + POST_ENTER_SETTLE_MS)
    expect(f.pastes()).toHaveLength(1)
    expect(getSessionInputSnapshot(f.session).pendingMaintenance).toEqual(['reload-skills'])

    setSessionPromptReady(f.session, true)
    vi.advanceTimersByTime(MAINTENANCE_QUIET_MS + CLEAR_SETTLE_MS)
    expect(f.pastes()).toHaveLength(2)
  })
})
