// One-time rescan of codex transcripts swept before engines were recorded.
//
// Those sweeps stored every codex item line as a zero-token `assistant` row
// and no usage at all (codex usage was not projected then). Dropping the
// byte cursor of each codex transcript makes the next sweep read it from byte
// 0: rows gain their engine, phantom item rows are deleted
// (jsonl-sweeper.ts), and usage lines a newer Bridge has written are counted.
// Claude transcripts keep their cursors — no full rescan of a large history.
// The flag makes it idempotent; a crash before the flag repeats the cheap
// cursor reset, never a double count (rows are keyed by uuid).

import { getDb } from './database/lifecycle'
import { withStatsWrite } from './database/write-lock'
import { transcriptEngine } from '../pty/transcript-engine'
import { infoLog, warnLog } from '../logging'

export const CODEX_RESCAN_FLAG = 'codex_engine_v1_rescanned'

/** Resets the cursors of the codex transcripts among `files` once per database.
 *  Returns the reset files (none when already done); the caller must not
 *  skip them as unchanged since their last read. */
export async function rescanCodexTranscriptsOnce(files: readonly string[]): Promise<string[]> {
  const db = await getDb()
  const done = (await db.runAndReadAll('SELECT 1 FROM settings WHERE key = ?', [CODEX_RESCAN_FLAG])).getRowObjects()
  if (done.length > 0) return []
  // Only files already swept have a cursor to reset; new ones start at 0 anyway.
  const swept = new Set(
    (
      (await db.runAndReadAll('SELECT file_path FROM jsonl_offsets')).getRowObjects() as Array<{ file_path: string }>
    ).map((r) => r.file_path),
  )
  const codex = files.filter((file) => swept.has(file) && transcriptEngine(file) === 'codex')
  await withStatsWrite(async () => {
    await db.run('BEGIN')
    try {
      for (const file of codex) await db.run('DELETE FROM jsonl_offsets WHERE file_path = ?', [file])
      await db.run('INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT DO NOTHING', [
        CODEX_RESCAN_FLAG,
        new Date().toISOString(),
      ])
      await db.run('COMMIT')
    } catch (err) {
      await db.run('ROLLBACK').catch((error) => warnLog('stats: Codex rescan rollback failed', { error: String(error) }))
      throw err
    }
  })
  if (codex.length > 0) infoLog('stats: codex transcripts queued for one rescan', { files: codex.length })
  return codex
}
