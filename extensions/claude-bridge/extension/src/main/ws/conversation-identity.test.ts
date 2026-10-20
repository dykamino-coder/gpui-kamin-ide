import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0070: after the Bridge container was recreated a Chat tab could
 * reconnect as a NEW empty conversation and overwrite its stored id. The
 * client used to drop the stored id on its own after repeated establish
 * timeouts or fast resumed exits, and adopted the server's fresh id after a
 * resume-not-found. These tests drive the actual ConnectionManager message
 * path: the stored id is always re-sent, never replaced without a user choice.
 */

const h = vi.hoisted(() => ({
  warnings: [] as Array<{ message: string; items: string[]; answer: (choice?: string) => void }>,
  identity: [] as Array<Record<string, unknown>>,
}))
vi.mock('vscode', () => ({
  window: {
    showWarningMessage: vi.fn((message: string, ...items: string[]) => new Promise((resolve) => {
      h.warnings.push({ message, items, answer: resolve })
    })),
  },
}))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))
vi.mock('../../incident-diagnostics', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../incident-diagnostics')>()
  return {
    ...real,
    recordConversationIdentity: (tabId: string, fields: unknown) => {
      const record = real.normalizeConversationIdentity(tabId, fields)
      if (record) h.identity.push(record as unknown as Record<string, unknown>)
    },
  }
})

import { ConnectionManager } from './connection-manager'
import { conversationRef } from './conversation-ref'
import { formatIncidentLine, normalizeConversationIdentity } from '../../incident-diagnostics'
import type { ConnectionConfig } from '../../shared/types'
import type { BrowserWindow } from '@kaminide/host-compat'

const A = '00000000-0000-4000-8000-000000000001'
const B = '00000000-0000-4000-8000-000000000002'

const managers: ConnectionManager[] = []
const adopted: Array<[string, string]> = []
const detached: Array<[string, string]> = []

function manager(stored?: string) {
  const m = new ConnectionManager({ serverUrl: 'ws://synthetic', token: 'synthetic' } as ConnectionConfig,
    { webContents: { send: vi.fn() } } as unknown as BrowserWindow, 'tab', stored)
  managers.push(m)
  const frames: Array<Record<string, unknown>> = []
  const socket = {
    readyState: 1, bufferedAmount: 0,
    send: (frame: string) => frames.push(JSON.parse(frame)),
    removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn(), terminate: vi.fn(),
  }
  const internal = m as any
  internal.ws = socket
  return { m, internal, frames, socket }
}

async function startSession(internal: any, socket: unknown): Promise<void> {
  await internal.startSession(socket)
}

beforeEach(() => {
  h.warnings.length = 0
  h.identity.length = 0
  adopted.length = 0
  detached.length = 0
  ConnectionManager.onConversationId = (tab, id) => { adopted.push([tab, id]) }
  ConnectionManager.onDetachedConversation = (tab, id) => { detached.push([tab, id]) }
})
afterEach(() => {
  for (const m of managers.splice(0)) m.disconnect({ endSession: true })
  ConnectionManager.onConversationId = null
  ConnectionManager.onDetachedConversation = null
  vi.useRealTimers()
})

describe('INC-2026-0070 conversation identity across reconnects', () => {
  it('repeated establish timeouts keep resuming the stored id and only offer a new conversation', async () => {
    vi.useFakeTimers()
    const { m, internal, frames, socket } = manager(A)
    for (let attempt = 0; attempt < 4; attempt++) {
      internal.state = { status: 'connected' }
      internal.ws = socket
      await startSession(internal, socket)
      vi.advanceTimersByTime(25_000)
    }
    expect(m.getConversationId()).toBe(A)
    expect(frames.map((f) => f.type)).toEqual(['session:resume', 'session:resume', 'session:resume', 'session:resume'])
    expect(frames.every((f) => f.conversationId === A)).toBe(true)
    expect(h.warnings).toHaveLength(1)
    expect(h.warnings[0]!.items).toEqual(['Start new conversation'])
    expect(h.identity.filter((r) => r.action === 'establish-timeout').map((r) => r.count)).toEqual([1, 2, 3, 4])
  })

  it('only an explicit user choice switches an unreachable tab to session:create', async () => {
    vi.useFakeTimers()
    const { m, internal, frames, socket } = manager(A)
    for (let attempt = 0; attempt < 3; attempt++) {
      internal.state = { status: 'connected' }
      await startSession(internal, socket)
      vi.advanceTimersByTime(25_000)
    }
    h.warnings[0]!.answer('Start new conversation')
    await vi.advanceTimersByTimeAsync(0)
    expect(m.getConversationId()).toBeNull()
    expect(h.identity.some((r) => r.action === 'user-new')).toBe(true)
    frames.length = 0
    internal.ws = socket
    await startSession(internal, socket)
    expect(frames[0]).toMatchObject({ type: 'session:create' })
  })

  it('fast resumed exits park reconnect with the stored id instead of starting fresh', async () => {
    const { m, internal, socket } = manager(A)
    for (let i = 0; i < 3; i++) {
      internal.ws = socket
      internal.dispatchMessage({ type: 'session:created', sessionId: `s${i}` })
      internal.dispatchMessage({ type: 'session:exit', code: 1, sessionId: `s${i}` })
    }
    expect(m.getConversationId()).toBe(A)
    expect(internal.resumeParked).toBe(true)
    expect(m.getState().status).toBe('error')
    expect(h.warnings).toHaveLength(1)
    expect(h.warnings[0]!.items).toEqual(['Start new conversation', 'Retry'])
    const scheduled = internal.reconnectTimer
    h.warnings[0]!.answer('Retry')
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(internal.resumeParked).toBe(false)
    expect(m.getConversationId()).toBe(A)
    expect(scheduled).toBeNull()
    expect(socket.terminate).toHaveBeenCalled()
  })

  it('resume-not-found keeps the stored id, records the fresh one in history only', async () => {
    const { m, internal } = manager(A)
    internal.dispatchMessage({ type: 'session:created', sessionId: 's', resumeNotFound: true })
    internal.dispatchMessage({ type: 'session:conversation-id', sessionId: 's', conversationId: B })

    expect(m.getConversationId()).toBe(A)
    expect(m.isIdentityHeld()).toBe(true)
    expect(adopted).toEqual([])
    expect(detached).toEqual([['tab', B]])
    expect(h.warnings).toHaveLength(1)
    expect(h.identity.find((r) => r.action === 'hold')).toMatchObject({
      storedRef: conversationRef(A),
      nextRef: conversationRef(B),
    })

    // A later reconnect resumes the STORED conversation, not the temporary one.
    internal.state = { status: 'connected' }
    const frames: Array<Record<string, unknown>> = []
    internal.ws = { readyState: 1, send: (f: string) => frames.push(JSON.parse(f)), terminate: vi.fn(), removeAllListeners: vi.fn(), on: vi.fn(), close: vi.fn() }
    await internal.startSession(internal.ws)
    expect(frames[0]).toMatchObject({ type: 'session:resume', conversationId: A })
  })

  it('choosing a new conversation after not-found adopts the running fresh session', async () => {
    const { m, internal } = manager(A)
    internal.dispatchMessage({ type: 'session:created', sessionId: 's', resumeNotFound: true })
    internal.dispatchMessage({ type: 'session:conversation-id', sessionId: 's', conversationId: B })
    h.warnings[0]!.answer('Start new conversation')
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(m.getConversationId()).toBe(B)
    expect(m.isIdentityHeld()).toBe(false)
    expect(adopted).toEqual([['tab', B]])
  })

  it('a successful resume ends a hold; first discovery and compact-chain changes are adopted and logged', () => {
    const held = manager(A)
    held.internal.dispatchMessage({ type: 'session:created', sessionId: 's', resumeNotFound: true })
    held.internal.dispatchMessage({ type: 'session:created', sessionId: 's2' })
    expect(held.m.isIdentityHeld()).toBe(false)

    const { m, internal } = manager()
    internal.dispatchMessage({ type: 'session:created', sessionId: 'n' })
    internal.dispatchMessage({ type: 'session:conversation-id', sessionId: 'n', conversationId: A })
    internal.dispatchMessage({ type: 'session:conversation-id', sessionId: 'n', conversationId: B })
    expect(m.getConversationId()).toBe(B)
    expect(adopted).toEqual([['tab', A], ['tab', B]])
    const adopts = h.identity.filter((r) => r.action === 'adopt')
    expect(adopts.map((r) => r.reason)).toEqual(['discovered', 'changed'])
    expect(adopts[1]).toMatchObject({ storedRef: conversationRef(A), nextRef: conversationRef(B) })
  })

  it('identity records are allowlisted pseudonyms matching the server journal refs', () => {
    // Same fixed vector as server/src/core/logging/lifecycle.test.ts.
    expect(conversationRef(A)).toBe('53d31033b678')
    const record = normalizeConversationIdentity('tab-secret', {
      action: 'hold', stored: A, next: B, reason: 'not-found', count: 2.7, payload: 'private',
    })!
    const line = formatIncidentLine(record)
    expect(record).toMatchObject({ action: 'hold', storedRef: '53d31033b678', nextRef: 'a09e04d7ed68', count: 2 })
    expect(line).not.toContain(A)
    expect(line).not.toContain(B)
    expect(line).not.toContain('tab-secret')
    expect(line).not.toContain('private')
    expect(normalizeConversationIdentity('t', { action: 'drop-everything' })).toBeNull()
  })
})
