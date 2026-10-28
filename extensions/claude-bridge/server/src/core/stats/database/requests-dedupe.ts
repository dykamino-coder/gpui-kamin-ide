// One-time cleanup of user-input request rows duplicated by transcript replay
// (INC-2026-0078).
//
// Until the replay fix, every resume/reattach re-fired the "user input"
// recorder for each historical user message of the transcript. Each firing
// inserted a NEW `requests` row (fresh random id) for the same message: same
// original `timestamp` (the JSONL entry's own) and same `user_message`, but the
// session_key of the CURRENT PTY session and the user who resumed. So the
// duplicates differ from the original only in id/session_key/user_name.
//
// The identity of one real input is therefore (timestamp, user_message): a
// genuinely repeated message has a different timestamp and is kept. Of each
// group the earliest-inserted row (lowest rowid) is the original live insert,
// carrying the original session and user, and survives.
//
// Runs once (settings flag), inside the exclusive maintenance window, after a
// backup copy of the database file when the disk has room for it; without room
// it is skipped with a warning and retried on the next start.

import fs from 'fs'
import fsp from 'fs/promises'
import path from 'path'
import type { DuckDBConnection } from '@duckdb/node-api'
import { getDataDir, getDuckDbPath, withExclusiveDb } from './lifecycle'
import { infoLog, warnLog } from '../../logging'

export const REQUESTS_DEDUPE_FLAG = 'requests_replay_dedupe_v1'

const SCOPE = `endpoint = 'pty' AND method = 'INPUT' AND COALESCE(CAST(is_user_request AS INTEGER), 0) = 1`

export interface DedupeResult {
  status: 'done' | 'already-done' | 'skipped-no-space' | 'failed'
  deleted: number
  backup?: string
}

async function flagSet(conn: DuckDBConnection): Promise<boolean> {
  const rows = (
    await conn.runAndReadAll(`SELECT value FROM settings WHERE key = ?`, [REQUESTS_DEDUPE_FLAG])
  ).getRowObjects()
  return rows.length > 0
}

async function freeBytes(dir: string): Promise<number> {
  const s = await fsp.statfs(dir)
  return Number(s.bavail) * Number(s.bsize)
}

/** Delete replay duplicates once. `opts.backup=false` only for tests. */
export async function dedupeReplayedRequests(
  opts: { backup?: boolean; free?: (dir: string) => Promise<number> } = {},
): Promise<DedupeResult> {
  const doBackup = opts.backup ?? true
  try {
    return await withExclusiveDb(async (conn) => {
      if (await flagSet(conn)) return { status: 'already-done', deleted: 0 }
      let backup: string | undefined
      if (doBackup) {
        // A consistent file copy needs the WAL folded in; the window
        // guarantees nothing else writes meanwhile.
        await conn.run('CHECKPOINT')
        const db = getDuckDbPath()
        const size = fs.existsSync(db) ? fs.statSync(db).size : 0
        const free = await (opts.free ?? freeBytes)(getDataDir())
        if (free < size * 1.1 + 64 * 1024 * 1024) {
          warnLog('[db-maint] requests dedupe skipped: not enough disk for the backup copy', { size, free })
          return { status: 'skipped-no-space', deleted: 0 }
        }
        // Copied THROUGH DuckDB (the open file is locked on some platforms
        // and a raw copy is only consistent by convention): a complete,
        // openable database file.
        backup = path.join(getDataDir(), `proxy.pre-dedupe-${Date.now()}.duckdb`)
        const esc = (s: string) => s.replace(/'/g, "''")
        const cat = (await conn.runAndReadAll(`SELECT current_catalog() AS c`)).getRowObjects()[0] as { c: unknown }
        await conn.run(`ATTACH '${esc(backup)}' AS dedupe_backup`)
        try {
          await conn.run(`COPY FROM DATABASE "${String(cat.c)}" TO dedupe_backup`)
        } finally {
          await conn.run(`DETACH dedupe_backup`)
        }
      }
      const count = async (): Promise<number> => {
        const r = (
          await conn.runAndReadAll(`SELECT count(*) AS n FROM requests WHERE ${SCOPE}`)
        ).getRowObjects()[0] as {
          n: bigint
        }
        return Number(r.n)
      }
      const before = await count()
      await conn.run('BEGIN')
      try {
        await conn.run(
          `DELETE FROM requests
           WHERE ${SCOPE}
             AND rowid NOT IN (
               SELECT min(rowid) FROM requests WHERE ${SCOPE}
               GROUP BY timestamp, user_message
             )`,
        )
        await conn.run(
          `INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT DO UPDATE SET value = excluded.value`,
          [REQUESTS_DEDUPE_FLAG, new Date().toISOString()],
        )
        await conn.run('COMMIT')
      } catch (e) {
        await conn.run('ROLLBACK').catch(() => {})
        throw e
      }
      const deleted = before - (await count())
      if (deleted > 0) await conn.run('CHECKPOINT')
      infoLog('[db-maint] replay-duplicated user input rows removed', { deleted, backup })
      return { status: 'done', deleted, backup }
    })
  } catch (e) {
    warnLog('[db-maint] requests dedupe failed; will retry on next start', { error: String(e) })
    return { status: 'failed', deleted: 0 }
  }
}

/** Boot hook: run the cleanup, then compact when it freed a lot of rows. */
export async function runRequestsDedupeAtBoot(): Promise<void> {
  const r = await dedupeReplayedRequests()
  if (r.status === 'done' && r.deleted >= 50_000) {
    const { compactDatabase } = await import('./maintenance')
    const c = await compactDatabase()
    infoLog('[db-maint] compaction after requests dedupe', { ...c })
  }
}
