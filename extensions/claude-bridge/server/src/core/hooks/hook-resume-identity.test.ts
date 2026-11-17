import 'reflect-metadata'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { sessions } from '../pty/session-core'
import type { PtySession } from '../types/pty'
import { clearSession, registerSessionHooks } from './registry'
import { createHooksRoutes } from './routes'

/** INC-2026-0102: the SessionStart hook of an inline /resume is the CLI's only
 *  report of the resumed identity; the route hands it to the session's watcher. */
const id = 'hook-resume-identity'
const app = createHooksRoutes()
const RESUMED = '11111111-2222-4333-8444-555555555555'

function seed(extra: Partial<PtySession> = {}) {
  const adoptResumedTranscript = vi.fn(() => true)
  sessions.set(id, {
    id,
    ws: null,
    mcpToken: 'synthetic',
    cwd: '/client',
    state: 'running',
    jsonlWatcher: { stop: vi.fn(), getFilePath: () => null, adoptResumedTranscript },
    ...extra,
  } as unknown as PtySession)
  const registered = registerSessionHooks(
    id,
    { SessionStart: [{ hooks: [{ type: 'command', command: ':', host: 'local' }] }] },
    { kind: 'bridge' },
  )
  return { hookId: [...registered.values()][0]!.id, adoptResumedTranscript }
}

function post(hookId: string, body: Record<string, unknown>) {
  return app.request(`/api/hooks/${id}/SessionStart/${hookId}`, {
    method: 'POST',
    headers: {
      Authorization: 'Bearer synthetic',
      'X-Bridge-Hook-Relay': 'command',
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ hook_event_name: 'SessionStart', session_id: RESUMED, ...body }),
  })
}

afterEach(() => {
  clearSession(id)
  sessions.delete(id)
})

describe('SessionStart resume hands the identity to the watcher', () => {
  it('adopts the resumed conversation even without an attached client', async () => {
    const { hookId, adoptResumedTranscript } = seed()
    const res = await post(hookId, { source: 'resume' })
    expect(res.status).toBe(200)
    expect(adoptResumedTranscript).toHaveBeenCalledWith(RESUMED)
  })

  it.each(['startup', 'clear', 'compact'])('leaves discovery to the watcher for source %s', async (source) => {
    const { hookId, adoptResumedTranscript } = seed()
    await post(hookId, { source })
    expect(adoptResumedTranscript).not.toHaveBeenCalled()
  })

  it('does not retarget a multi-engine conversation log', async () => {
    const { hookId, adoptResumedTranscript } = seed({
      conversation: { dir: '/conversations/c1' },
    } as unknown as Partial<PtySession>)
    await post(hookId, { source: 'resume' })
    expect(adoptResumedTranscript).not.toHaveBeenCalled()
  })
})
