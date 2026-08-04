// JSONL-based input tracking extracted from session-core.ts (Sprint 5 /
// Stage E1). The watcher hands us each `type: "user"` entry from the
// JSONL file; this is the source of truth for user-input counting (PTY
// keystroke tracking was lossy). Tool-result entries are also "user"
// type but skipped because they're not real user input.

import { randomUUID } from 'crypto'
import type { PtySession } from './types'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { insertRequest } from '../stats/database/crud'
import type { RequestLogEntry } from '../stats/types'
import { eventBus } from '../events/bus'
import { warnLog } from '../logging'

export function handleJsonlUserEntry(session: PtySession, entry: JsonlEntry): void {
  const content = entry.message?.content
  if (Array.isArray(content) && content.some((b) => b.type === 'tool_result')) return

  const userMessage = typeof content === 'string'
    ? content
    : Array.isArray(content)
      ? content.filter((b) => b.type === 'text').map((b) => (b as { text: string }).text).join('\n')
      : ''

  if (!userMessage.trim()) return

  session.inputCount++
  session.lastActivityAt = new Date()

  const logEntry: RequestLogEntry = {
    id: randomUUID(),
    timestamp: entry.timestamp ? new Date(entry.timestamp) : new Date(),
    endpoint: 'pty',
    method: 'INPUT',
    model: '',
    pluginId: 'pty',
    userName: session.userName,
    durationMs: 0,
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    toolsUsed: [],
    status: 'success',
    statusCode: 200,
    isUserRequest: true,
    userMessage: userMessage.slice(0, 2000),
    sessionKey: session.id,
  }

  try {
    insertRequest(logEntry)
    eventBus.emit('request:start', logEntry)
    eventBus.emit('request:complete', logEntry)
  } catch (err) {
    warnLog('Failed to record JSONL user entry', { sessionId: session.id, error: String(err) })
  }

  eventBus.emit('session:updated', {
    sessionId: session.id,
    mcpCallCount: session.mcpCallCount,
    inputCount: session.inputCount,
    mcpInitialized: session.mcpInitialized,
    mcpLastError: session.mcpLastError,
    lastActivityAt: session.lastActivityAt.toISOString(),
  })
}
