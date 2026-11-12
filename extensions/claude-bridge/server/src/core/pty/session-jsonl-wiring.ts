// Transcript watcher wiring shared by every engine. Claude CLI writes its
// JSONL itself; a codex session writes Bridge's normalized projection into the
// same slug directory, so both are tailed, replayed, paged and indexed by the
// one JsonlWatcher. Moved verbatim from createAdmittedSession (session-core).
import type { AgentEngineId } from '../../shared/agent-events'
import { debugLog, warnLog } from '../logging'
import { lifecycleLog } from '../logging/lifecycle'
import { JsonlWatcher } from './jsonl-watcher'
import { startSegmentPump } from '../conversation/conversation'
import { leanEntries } from './jsonl-projection'
import { sendToClient, socketWritable, trySendToClient } from './session-io'
import { handleJsonlUserEntry } from './session-stats-recorder'
import { accumulateJsonlStats, resetJsonlStats } from './session-jsonl-stats'
import type { PtySession, SessionConfig } from './types'

export interface JsonlWiringHooks {
  /** Persist old→new id when the conversation file changes (auto-compact). */
  recordCompactLink(oldId: string, newId: string): void
  /** Called with every adopted conversation id (proxy usage attribution). */
  onConversationId?(conversationId: string): void
  engine?: AgentEngineId
}

export function attachSessionJsonlWatcher(
  session: PtySession,
  config: SessionConfig,
  startTime: number,
  hooks: JsonlWiringHooks,
): JsonlWatcher {
  const sessionId = session.id
  const { tokenId, userName } = session
  // A multi-engine conversation shows the client its unified log (in the
  // conversation's own directory, already on disk); the engine's native
  // transcript is copied into it by the segment pump.
  const conversation = session.conversation
  const settingsDir = conversation?.dir ?? session.settingsDir
  if (conversation) {
    startTime = 0
    session.segmentPump = startSegmentPump(conversation) ?? undefined
  }
  // All watcher callbacks send via session.ws (not the captured ws) so a
  // reattached client keeps receiving entries.
  const jsonlWatcher = new JsonlWatcher(
    startTime,
    // `replay: true` — батч истории/превью вне файла-порядка: клиент не должен
    // двигать по нему курсор зеркала (прод-дыра «сообщение не отрисовалось»).
    (entries, replay) =>
      trySendToClient(session.ws, {
        type: 'jsonl:entries',
        entries: leanEntries(entries),
        ...(replay ? { replay: true } : {}),
      }),
    (status) => sendToClient(session.ws, { type: 'jsonl:status', ...status }),
    (entry) => handleJsonlUserEntry(session, entry),
    settingsDir,
    (conversationId) => {
      // Смена id (авто-compact → новый файл): помнить ПРЕЖНИЕ id — клиент,
      // перезапустившийся со старым id из metadata, обязан находить эту живую
      // сессию (finder ниже матчит и по alias'ам); плюс персист линка
      // old→new, чтобы холодный резюм после смерти PTY дошёл до свежего
      // файла, а не воскрешал скомпакченный (инцидент «откат чата»).
      const prev = session.cliConversationId
      if (prev && prev !== conversationId) {
        ;(session.conversationAliases ??= []).push(prev)
        hooks.recordCompactLink(prev, conversationId)
      }
      session.cliConversationId = conversationId
      hooks.onConversationId?.(conversationId)
      // INC-2026-0070: every identity the client is told to adopt, old→new.
      // `discovered` with previousRef equal to conversationRef confirms the
      // resumed id; a fresh session's first discovery has no previousRef.
      lifecycleLog('conversation_id', {
        sessionId,
        conversationId,
        previousConversationId: prev ?? undefined,
        idSource: prev && prev !== conversationId ? 'changed' : 'discovered',
      })
      debugLog('JSONL conversationId discovered', { sessionId, conversationId })
      sendToClient(session.ws, { type: 'session:conversation-id', sessionId, conversationId })
      // Map JSONL session → PTY token so the JSONL sweeper can attribute
      // jsonl_events rows to the right API token (Stats panel + per-token
      // session list rely on this). DuckDB is async — wrap in a fire-and-
      // forget IIFE so we don't change the JsonlWatcher callback signature.
      void (async () => {
        try {
          const { getDb } = await import('../stats/database/lifecycle')
          const { withStatsWrite } = await import('../stats/database/write-lock')
          const db = await getDb()
          const realCwd = config.cwd || null
          const titleAtInsert = session.sessionTitle || null
          const engine = hooks.engine ?? 'claude'
          // Serialised with the JSONL sweeper, which inserts the same
          // session_tokens row on first sight of the file: two writers on
          // one key would fail a COMMIT, and DuckDB turns that into a
          // process-killing FatalException (stats/database/write-lock.ts).
          await withStatsWrite(async () => {
            await db.run(
              `INSERT INTO session_tokens (session_id, token_id, user_name, cwd, title, created_at, deleted, engine)
               VALUES (?, ?, ?, ?, ?, ?, 0, ?)
               ON CONFLICT (session_id) DO NOTHING`,
              [conversationId, tokenId, userName, realCwd, titleAtInsert, new Date().toISOString(), engine],
            )
            await db.run(
              `UPDATE session_tokens
               SET token_id = ?,
                   user_name = ?,
                   cwd = COALESCE(NULLIF(cwd, ''), ?),
                   title = COALESCE(NULLIF(title, ''), ?)
               WHERE session_id = ? AND (token_id IS NULL OR token_id = '')`,
              [tokenId, userName, realCwd, titleAtInsert, conversationId],
            )
            // The sweeper may have inserted the row first without an engine.
            await db.run(`UPDATE session_tokens SET engine = ? WHERE session_id = ? AND engine IS NULL`, [
              engine,
              conversationId,
            ])
          })
        } catch (err) {
          warnLog('session_tokens insert failed', { conversationId, error: String(err) })
        }
      })()
    },
    (agentName, entries, agentId) =>
      trySendToClient(session.ws, {
        type: 'jsonl:subagent-entries',
        agentName,
        agentId,
        entries: leanEntries(entries),
      }),
    () => resetJsonlStats(session),
    (entry) => accumulateJsonlStats(session, entry),
    // Bulk replay only goes out while the CURRENT socket (reattach swaps it)
    // is open and under the replay high-water mark.
    () => socketWritable(session.ws),
    config.history,
  )
  session.jsonlWatcher = jsonlWatcher
  jsonlWatcher.start()
  return jsonlWatcher
}
