// Synthetic AB-04 regression: no server, PTY process or network is started.
import 'reflect-metadata'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { attachOutputDebounce, sendToClient } from './session-io'
import { detachSession, reattachSession, finalizeSessionTeardown, destroySession, sessions } from './session-core'
import type { PtySession } from './types'
let emit: (data: string) => void
let session: PtySession
let ws: { readyState: number; bufferedAmount: number; send: ReturnType<typeof vi.fn> }
beforeEach(() => {
  vi.useFakeTimers()
  ws = { readyState: 1, bufferedAmount: 0, send: vi.fn() }
  session = {
    id: 'bounds',
    ws,
    state: 'running',
    createdAt: new Date(),
    lastActivityAt: new Date(),
    outputBuffer: [],
    outputBufferBytes: 0,
    pty: {
      onData: (cb: typeof emit) => {
        emit = cb
        return { dispose: vi.fn() }
      },
      resize: vi.fn(),
      kill: vi.fn(),
      cols: 120,
      rows: 40,
    },
    pendingElicitations: new Map(),
    childSessions: [],
    settingsDir: '',
    cliConversationId: null,
  } as unknown as PtySession
  sessions.set(session.id, session)
  attachOutputDebounce(session)
})
afterEach(() => {
  sessions.clear()
  vi.clearAllTimers()
  vi.useRealTimers()
})
it('does not replay 20 MiB of detached output from the private debounce buffer', () => {
  ws.readyState = 3
  const chunk = 'x'.repeat(4095) + '\n'
  for (let i = 0; i < 5120; i++) {
    emit(chunk)
    vi.advanceTimersByTime(8)
  }
  expect(session.outputBufferBytes).toBeLessThan(650_000)
  expect(vi.getTimerCount()).toBe(0)
  ws.readyState = 1
  emit('new')
  vi.advanceTimersByTime(32)
  const bytes = ws.send.mock.calls.reduce((sum, [raw]) => sum + Buffer.byteLength(raw), 0)
  expect(bytes).toBeLessThan(100_000)
  expect(ws.send).toHaveBeenCalledExactlyOnceWith(JSON.stringify({ type: 'session:output', data: 'new' }))
})
it('bounds a synchronous output burst even before timers can run', () => {
  const chunk = 'я'.repeat(4095) + '😀'
  for (let i = 0; i < 512; i++) emit(chunk)
  vi.advanceTimersByTime(32)
  expect(ws.send.mock.calls.every(([raw]) => Buffer.byteLength(raw) < 100_000)).toBe(true)
  expect(ws.send.mock.calls.map(([raw]) => JSON.parse(raw).data).join('')).toBe(chunk.repeat(512))
})
it('preserves normal debounce order', () => {
  emit('hello')
  emit(' world')
  vi.advanceTimersByTime(8)
  expect(ws.send).toHaveBeenCalledExactlyOnceWith(JSON.stringify({ type: 'session:output', data: 'hello world' }))
})
it('clears pending debounce on detach and uses ring replay plus resize on reattach', () => {
  emit('before detach')
  detachSession(session.id)
  emit('while detached')
  const fresh = { ...ws, send: vi.fn() }
  reattachSession(session, fresh as never)
  vi.advanceTimersByTime(32)
  const output = fresh.send.mock.calls.map(([raw]) => JSON.parse(raw)).filter((m) => m.type === 'session:output')
  expect(output).toEqual([{ type: 'session:output', data: 'before detach\nwhile detached' }])
  expect(session.pty.resize).toHaveBeenCalledWith(119, 40)
})
it('cancels output timers and callbacks on teardown', () => {
  emit('pending')
  finalizeSessionTeardown(session.id, 'pty-exit', 0)
  ws.send.mockClear()
  expect(vi.getTimerCount()).toBe(0)
  emit('late callback')
  vi.advanceTimersByTime(32)
  expect(ws.send).not.toHaveBeenCalled()
})
it('includes the UTF-8 serialized new frame in the outbound budget', () => {
  ws.bufferedAmount = 16 * 1024 * 1024 - 100
  expect(sendToClient(ws as never, { data: 'я'.repeat(100) })).toBe(false)
  expect(ws.send).not.toHaveBeenCalled()
})

it('stops output immediately when the teardown grace window opens', () => {
  emit('pending')
  destroySession(session.id)
  emit('late output during CLI exit')
  vi.advanceTimersByTime(32)
  expect(ws.send).not.toHaveBeenCalled()
})
