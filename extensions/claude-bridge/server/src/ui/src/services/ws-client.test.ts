import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { wsClient } from './ws-client'
import { isConnected, terminalSessions } from '../signals/server'

// Server CI installs only server dependencies, not the dashboard.
// This transport fixture needs mutable values, not Preact rendering.
vi.mock('../signals/server', () => ({
  serverHealth: { value: null },
  healthCheckedAt: { value: null },
  serverStats: { value: null },
  isConnected: { value: false },
  terminalSessions: { value: 0 },
  ptySessions: { value: [] },
  cachedAccount: { value: null },
  accountCheckedAt: { value: null },
  cachedUsage: { value: null },
}))
vi.mock('../signals/requests', () => ({
  addRequest: vi.fn(),
  updateRequest: vi.fn(),
  addError: vi.fn(),
  seedErrors: vi.fn(),
}))

class Socket {
  static OPEN = 1
  static instances: Socket[] = []
  static fail = false
  readyState = 1
  onopen: (() => void) | null = null
  onclose: (() => void) | null = null
  onerror: (() => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  send = vi.fn()
  constructor(public url: string) {
    if (Socket.fail) throw new Error('construction failed')
    Socket.instances.push(this)
  }
  close() {
    this.readyState = 3
    // Browsers dispatch close asynchronously, after logout cleanup has returned.
    setTimeout(() => this.onclose?.(), 0)
  }
  message(count: number) {
    this.onmessage?.({ data: JSON.stringify({ type: 'terminal:sessions', data: { count } }) })
  }
}

describe('dashboard socket lifecycle (INC-2026-0014)', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.stubGlobal('WebSocket', Socket)
    Socket.instances = []
    Socket.fail = false
    terminalSessions.value = 0
  })
  afterEach(() => {
    wsClient.disconnect()
    wsClient.onReconnect = null
    vi.clearAllTimers()
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })
  const latest = () => Socket.instances[Socket.instances.length - 1]!

  it('does not reconnect after delayed intentional close', () => {
    wsClient.connect('ws://synthetic/first')
    latest().onopen?.()
    wsClient.disconnect()
    vi.advanceTimersByTime(9000)
    expect(Socket.instances).toHaveLength(1)
    expect(isConnected.value).toBe(false)
  })

  it('cancels a pending retry on stop', () => {
    wsClient.connect('ws://synthetic/first')
    latest().onclose?.()
    wsClient.disconnect()
    vi.advanceTimersByTime(9000)
    expect(Socket.instances).toHaveLength(1)
  })

  it('ignores old open/message/error/close after logout and login', () => {
    wsClient.connect('ws://synthetic/first')
    const old = latest()
    wsClient.disconnect()
    wsClient.connect('ws://synthetic/second')
    old.onopen?.()
    expect(isConnected.value).toBe(false)
    latest().onopen?.()
    latest().message(7)
    old.message(99)
    old.onerror?.()
    old.onclose?.()
    vi.advanceTimersByTime(9000)
    expect(isConnected.value).toBe(true)
    expect(terminalSessions.value).toBe(7)
    expect(Socket.instances).toHaveLength(2)
  })

  it('repeated connect replaces the previous socket and cancels its retry', () => {
    wsClient.connect('ws://synthetic/first')
    const old = latest()
    old.onclose?.()
    wsClient.connect('ws://synthetic/second')
    latest().onopen?.()
    vi.advanceTimersByTime(9000)
    wsClient.send({ type: 'synthetic' })
    expect(Socket.instances).toHaveLength(2)
    expect(latest().send).toHaveBeenCalledOnce()
    expect(old.send).not.toHaveBeenCalled()
  })

  it('keeps one retry for unexpected close and rejects callbacks from the replaced socket', () => {
    wsClient.connect('ws://synthetic/first')
    const old = latest()
    old.onclose?.()
    old.onclose?.()
    vi.advanceTimersByTime(3000)
    latest().onopen?.()
    latest().message(3)
    old.onerror?.()
    old.message(99)
    old.onclose?.()
    vi.advanceTimersByTime(9000)
    expect(Socket.instances).toHaveLength(2)
    expect(terminalSessions.value).toBe(3)
    expect(isConnected.value).toBe(true)
  })

  it('cancels construction-failure retry on stop and can connect later', () => {
    Socket.fail = true
    wsClient.connect('ws://synthetic/first')
    wsClient.disconnect()
    Socket.fail = false
    vi.advanceTimersByTime(9000)
    expect(Socket.instances).toHaveLength(0)
    wsClient.connect('ws://synthetic/second')
    expect(Socket.instances).toHaveLength(1)
  })
})
