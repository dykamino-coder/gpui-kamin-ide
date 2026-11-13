// Постоянный roster: одна строка на владельца живых session_tokens.
// Изменение сессии и её вклад в roster коммитятся одной транзакцией.
import type { DuckDBConnection } from '@duckdb/node-api'
import { withStatsWrite } from './write-lock'
import { warnLog } from '../../logging'

export async function initSessionOwners(db: DuckDBConnection): Promise<void> {
  await db.run(`CREATE TABLE IF NOT EXISTS session_owners (
    user_name TEXT PRIMARY KEY, sessions BIGINT NOT NULL,
    first_seen TEXT NOT NULL, last_seen TEXT NOT NULL
  )`)
  await db.run('CREATE INDEX IF NOT EXISTS idx_session_tokens_user ON session_tokens(user_name)')
  const seeded = (await db.runAndReadAll("SELECT value FROM settings WHERE key = 'session_owners_v1'")).getRowObjects()
  if (seeded.length) return
  await db.run('BEGIN')
  try {
    await db.run('DELETE FROM session_owners')
    await db.run(`INSERT INTO session_owners
      SELECT user_name, COUNT(*), MIN(created_at), MAX(created_at) FROM session_tokens
      WHERE deleted = 0 AND user_name IS NOT NULL AND user_name != '' GROUP BY user_name`)
    await db.run("INSERT INTO settings (key, value) VALUES ('session_owners_v1', '1')")
    await db.run('COMMIT')
  } catch (error) {
    await rollback(db)
    throw error
  }
}

interface Owner {
  user_name: string
  created_at: string
}

async function owner(db: DuckDBConnection, sessionId: string): Promise<Owner | undefined> {
  return (
    await db.runAndReadAll(
      `SELECT user_name, created_at FROM session_tokens
    WHERE session_id = ? AND deleted = 0 AND user_name IS NOT NULL AND user_name != ''`,
      [sessionId],
    )
  ).getRowObjects()[0] as Owner | undefined
}

async function rollback(db: DuckDBConnection): Promise<void> {
  await db.run('ROLLBACK').catch((error) => warnLog('session owner rollback failed', { error: String(error) }))
}

/** Только записи, меняющие владельца/created_at/deleted; metadata не затрагивает roster. */
export async function writeSessionToken(
  db: DuckDBConnection,
  sessionId: string,
  write: () => Promise<unknown>,
): Promise<void> {
  await withStatsWrite(async () => {
    await db.run('BEGIN')
    try {
      const before = await owner(db, sessionId)
      await write()
      const after = await owner(db, sessionId)
      if (before?.user_name !== after?.user_name || before?.created_at !== after?.created_at) {
        if (before) {
          await db.run('UPDATE session_owners SET sessions = sessions - 1 WHERE user_name = ?', [before.user_name])
          await db.run('DELETE FROM session_owners WHERE user_name = ? AND sessions = 0', [before.user_name])
          // Пересчёт дат нужен только при удалении крайней даты владельца;
          // для небольших владельцев доступен индекс по user_name.
          const boundary = (
            await db.runAndReadAll(
              `SELECT 1 FROM session_owners
            WHERE user_name = ? AND (first_seen = ? OR last_seen = ?)`,
              [before.user_name, before.created_at, before.created_at],
            )
          ).getRowObjects()
          if (boundary.length) {
            await db.run(
              `UPDATE session_owners SET
              first_seen = (SELECT MIN(created_at) FROM session_tokens WHERE user_name = ? AND deleted = 0),
              last_seen = (SELECT MAX(created_at) FROM session_tokens WHERE user_name = ? AND deleted = 0)
              WHERE user_name = ?`,
              [before.user_name, before.user_name, before.user_name],
            )
          }
        }
        if (after) {
          await db.run(
            `INSERT INTO session_owners VALUES (?, 1, ?, ?)
            ON CONFLICT (user_name) DO UPDATE SET sessions = session_owners.sessions + 1,
              first_seen = LEAST(session_owners.first_seen, excluded.first_seen),
              last_seen = GREATEST(session_owners.last_seen, excluded.last_seen)`,
            [after.user_name, after.created_at, after.created_at],
          )
        }
      }
      await db.run('COMMIT')
    } catch (error) {
      await rollback(db)
      throw error
    }
  })
}
