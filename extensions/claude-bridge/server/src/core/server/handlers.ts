// ============================================================================
// Request Handlers - Health endpoint
// ============================================================================

import { statsWriteQueueInfo } from '../stats/database/write-lock'
import type { Context } from 'hono'
import { VERSION, SERVICE_NAME } from '../config'
import { getAllSessions } from '../pty/session-manager'

export async function handleHealth(c: Context) {
  const sessions = getAllSessions()
  const activeSessions = sessions.filter((s) => s.state === 'running')

  return c.json({
    status: 'ok',
    service: SERVICE_NAME,
    version: VERSION,
    mode: 'pty',
    sessions: {
      total: sessions.length,
      active: activeSessions.length,
    },
    // Stats writer health: a growing depth or a long-running head is the
    // early signal of a stuck database owner.
    statsWrite: (() => {
      const q = statsWriteQueueInfo()
      return { depth: q.depth, oldestWaitMs: q.oldestWaitMs, headRunningMs: q.head?.runningMs ?? 0 }
    })(),
  })
}
