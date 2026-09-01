// The streaming proxy pulls in @peculiar/x509 → tsyringe, which needs the
// reflect polyfill before its decorators run. Same import the server entry
// point needs today; unrelated to this change.
import 'reflect-metadata'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { eventBus } from '../events/bus'
import { clearSession, listSession, registerSessionHooks } from '../hooks/registry'
import type { HookSettings } from '../hooks/types'
import {
  destroySession,
  detachSession,
  finalizeSessionTeardown,
  isSessionTearingDown,
  sessions,
} from './session-core'
import type { PtySession } from '../types/pty'

const HOOKS: HookSettings = { SessionEnd: [{ hooks: [{ type: 'command', command: 'notify' }] }] }

function fakeSession(id: string): PtySession {
  const session = {
    id,
    pty: { kill: vi.fn(), pid: 1 },
    ws: { readyState: 1, send: vi.fn() },
    userName: 'tester',
    tokenId: 'token-1',
    settingsDir: '',
    cwd: '',
    state: 'running',
    createdAt: new Date(),
    lastActivityAt: new Date(),
    mcpCallCount: 0,
    inputCount: 0,
    userMessages: 0,
    assistantMessages: 0,
    contextTokens: 0,
    totalTokens: 0,
    mcpLog: [],
    mcpInitialized: true,
    mcpLastError: null,
    mcpToken: 'mcp-secret',
    outputBuffer: [],
    outputBufferBytes: 0,
    lastResizeAt: 0,
    consoleShrunk: false,
    registeredTools: [],
    registeredResources: [],
    registeredResourceTemplates: [],
    registeredPrompts: [],
    cliConversationId: null,
    childSessions: [],
  } as unknown as PtySession
  sessions.set(id, session)
  registerSessionHooks(id, HOOKS, { kind: 'user' })
  return session
}

describe('session teardown window', () => {
  const destroyed: string[] = []
  let unsubscribe: (() => void) | null = null

  beforeEach(() => {
    vi.useFakeTimers()
    destroyed.length = 0
    unsubscribe = eventBus.on('session:destroyed', (event) => {
      destroyed.push(String((event.data as { sessionId?: string }).sessionId))
    })
  })

  afterEach(() => {
    unsubscribe?.()
    unsubscribe = null
    vi.useRealTimers()
    for (const id of [...sessions.keys()]) {
      clearSession(id)
      sessions.delete(id)
    }
  })

  it('keeps the exiting session resolvable for its own SessionEnd relay', () => {
    fakeSession('teardown-open')
    destroySession('teardown-open')

    expect(isSessionTearingDown('teardown-open')).toBe(true)
    // The relay resolves the session by id and matches its mcpToken — both must
    // still be there, which is exactly what `Unknown session` used to break.
    expect(sessions.get('teardown-open')?.mcpToken).toBe('mcp-secret')
    expect(listSession('teardown-open')).toHaveLength(1)
    expect(destroyed).toEqual([])
  })

  it('closes the window on the first SessionEnd and ignores a duplicate callback', () => {
    fakeSession('teardown-once')
    destroySession('teardown-once')

    finalizeSessionTeardown('teardown-once', 'session-end', -1)
    expect(sessions.has('teardown-once')).toBe(false)
    expect(listSession('teardown-once')).toHaveLength(0)
    expect(destroyed).toEqual(['teardown-once'])

    finalizeSessionTeardown('teardown-once', 'session-end', -1)
    expect(destroyed).toEqual(['teardown-once'])
  })

  it('closes the window on timeout when no SessionEnd ever arrives', () => {
    fakeSession('teardown-timeout')
    destroySession('teardown-timeout')

    expect(sessions.has('teardown-timeout')).toBe(true)
    vi.advanceTimersByTime(10_000)

    expect(sessions.has('teardown-timeout')).toBe(false)
    expect(listSession('teardown-timeout')).toHaveLength(0)
    expect(destroyed).toEqual(['teardown-timeout'])
  })

  it('closes the window when the PTY exits first', () => {
    fakeSession('teardown-exit')
    destroySession('teardown-exit')

    finalizeSessionTeardown('teardown-exit', 'pty-exit', 143)
    expect(sessions.has('teardown-exit')).toBe(false)
    expect(destroyed).toEqual(['teardown-exit'])

    // The armed backstop must not fire a second destroy afterwards.
    vi.advanceTimersByTime(10_000)
    expect(destroyed).toEqual(['teardown-exit'])
  })

  it('never lets a predecessor timer tear down a reused session id', () => {
    fakeSession('reused-id')
    destroySession('reused-id')
    finalizeSessionTeardown('reused-id', 'session-end', -1)
    destroyed.length = 0

    // Same id, new session instance — the old teardown timer is still armed.
    const revived = fakeSession('reused-id')
    vi.advanceTimersByTime(10_000)

    expect(sessions.get('reused-id')).toBe(revived)
    expect(listSession('reused-id')).toHaveLength(1)
    expect(destroyed).toEqual([])
  })

  it('opens the same window when the detach grace expires', () => {
    fakeSession('detach-grace')
    detachSession('detach-grace')
    expect(isSessionTearingDown('detach-grace')).toBe(false)

    // Grace expiry destroys the session — the SessionEnd relay must survive it
    // exactly as it does for an explicit end. Stop just past the 10-minute
    // grace so the teardown backstop itself has not fired yet.
    vi.advanceTimersByTime(10 * 60 * 1000 + 1)
    expect(isSessionTearingDown('detach-grace')).toBe(true)
    expect(listSession('detach-grace')).toHaveLength(1)

    finalizeSessionTeardown('detach-grace', 'session-end', -1)
    expect(sessions.has('detach-grace')).toBe(false)
    expect(destroyed).toEqual(['detach-grace'])
  })

  it('is a no-op when called for a session that was never destroyed', () => {
    fakeSession('never-destroyed')
    finalizeSessionTeardown('missing-session', 'timeout', -1)
    expect(sessions.has('never-destroyed')).toBe(true)
    expect(destroyed).toEqual([])
  })

  it('refuses to reopen a window for a session already tearing down', () => {
    const session = fakeSession('teardown-twice')
    destroySession('teardown-twice')
    const opened = session.teardown
    destroySession('teardown-twice')
    expect(session.teardown).toBe(opened)
  })
})
