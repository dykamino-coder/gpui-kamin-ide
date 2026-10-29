import 'reflect-metadata'
import { EventEmitter } from 'node:events'
import type { WebSocket } from 'ws'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { sessions } from '../pty/session-core'
import type { PtySession } from '../types/pty'
import { cancelSessionLocalExecs, handleLocalHookResponse } from './dispatcher'
import { clearSession, registerSessionHooks } from './registry'
import { createHooksRoutes } from './routes'

const id = 'hook-client-disconnect'
const app = createHooksRoutes()

function client() {
  return Object.assign(new EventEmitter(), { readyState: 1, send: vi.fn() })
}

function seed(ws: ReturnType<typeof client> | null, source: 'user' | 'bridge' = 'user', timeout?: number) {
  sessions.set(id, { id, ws, mcpToken: 'synthetic', cwd: '/client', state: 'running' } as unknown as PtySession)
  const registered = registerSessionHooks(
    id,
    {
      SessionStart: [{ hooks: [{ type: 'command', command: ':', host: 'local', timeout }] }],
    },
    { kind: source },
  )
  return [...registered.values()][0]!.id
}

async function request(hookId: string) {
  return app.request(`/api/hooks/${id}/SessionStart/${hookId}`, {
    method: 'POST',
    headers: {
      Authorization: 'Bearer synthetic',
      'X-Bridge-Hook-Relay': 'command',
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ hook_event_name: 'SessionStart', source: 'compact' }),
  })
}

afterEach(() => {
  cancelSessionLocalExecs(id)
  clearSession(id)
  sessions.delete(id)
  vi.useRealTimers()
})

describe('local SessionStart relay while the IDE disconnects', () => {
  it.each([null, 2, 3])('returns an HTTP result when the client is absent or closed (%s)', async (state) => {
    const ws = state === null ? null : client()
    if (ws) ws.readyState = state!
    const res = await request(seed(ws))
    expect(res.status).toBe(200)
    expect((await res.json()).result).toMatchObject({ exitCode: 1, outcome: 'error' })
    if (ws) expect(ws.send).not.toHaveBeenCalled()
  })

  it.each(['close', 'error', 'send-error', 'send-throw'])('answers HTTP when WS fails via %s', async (failure) => {
    const ws = client()
    const hookId = seed(ws)
    const sent = new Promise<void>((resolve) => {
      ws.send.mockImplementation((_frame: string, callback?: (err: Error) => void) => {
        if (failure === 'send-throw') {
          resolve()
          throw new Error('closed')
        }
        if (failure === 'send-error') callback?.(new Error('closed'))
        resolve()
      })
    })
    const pending = request(hookId)
    await sent
    if (failure === 'error') ws.emit('error', new Error('closed'))
    else if (failure === 'close') ws.emit('close')
    const res = await pending
    expect(res.status).toBe(200)
    expect((await res.json()).result).toMatchObject({
      exitCode: 1,
      outcome: 'error',
      stderr:
        'Bridge hook dispatch {"event":"SessionStart","type":"command","host":"local"}: Client-host WS disconnected during local hook',
    })
    expect(ws.listenerCount('close')).toBe(0)
    expect(ws.listenerCount('error')).toBe(0)
  })

  it('does not replay the old command on reconnect and permits the next hook', async () => {
    const oldWs = client()
    const hookId = seed(oldWs)
    const sent = new Promise<void>((resolve) => oldWs.send.mockImplementation(() => resolve()))
    const pending = request(hookId)
    await sent
    oldWs.emit('close')
    expect((await (await pending).json()).result.exitCode).toBe(1)
    const ws = client()
    sessions.get(id)!.ws = ws as unknown as WebSocket
    expect(ws.send).not.toHaveBeenCalled()
    ws.send.mockImplementation((raw: string) => {
      const frame = JSON.parse(raw)
      if (frame.type === 'hook:execute')
        handleLocalHookResponse(frame.requestId, {
          stdout: 'ok',
          stderr: '',
          exitCode: 0,
          outcome: 'success',
          durationMs: 1,
        })
    })
    const res = await request(hookId)
    expect((await res.json()).result).toMatchObject({ stdout: 'ok', exitCode: 0 })
    expect(ws.listenerCount('close')).toBe(0)
  })

  it('returns the dispatcher timeout as an HTTP envelope', async () => {
    vi.useFakeTimers()
    const ws = client()
    const hookId = seed(ws, 'user', 1)
    const sent = new Promise<void>((resolve) => ws.send.mockImplementation(() => resolve()))
    const pending = request(hookId)
    await sent
    await vi.advanceTimersByTimeAsync(1000)
    const res = await pending
    expect(res.status).toBe(200)
    expect((await res.json()).result).toMatchObject({ exitCode: 124, outcome: 'timeout' })
    expect(ws.listenerCount('close')).toBe(0)
  })

  it('acknowledges the Bridge status hook without dispatch even on a closed WS', async () => {
    const ws = client()
    ws.readyState = 3
    const res = await request(seed(ws, 'bridge'))
    expect(res.status).toBe(200)
    expect((await res.json()).result).toMatchObject({ exitCode: 0, outcome: 'success' })
    expect(ws.send).not.toHaveBeenCalled()
  })
})
