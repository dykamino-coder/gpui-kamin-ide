// ============================================================================
// Dashboard status & config endpoints
// ============================================================================

import * as os from 'os'
import * as fs from 'fs'
import * as path from 'path'
import type { Hono } from 'hono'
import { config, VERSION } from '../../config'
import { getEditableSettings } from '../../config/settings'
import { getAllSessions } from '../../pty/session-manager'
import { getDb } from '../../stats/database/lifecycle'
import { getSystemStats, startSystemStatsSampler } from '../system-stats'
import { denyApiToken } from '../authz'

export function registerStatusRoutes(api: Hono): void {
  // GET /api/dashboard/status -- server status
  api.get('/api/dashboard/status', async (c) => {
    startSystemStatsSampler() // lazy-start the CPU/RAM background sampler
    const sessions = getAllSessions()

    // DB info — DuckDB file + schema version. Size is the on-disk file
    // bytes (which approximates total persistent storage).
    let dbType = 'duckdb'
    let dbVersion: string | null = null
    let dbSizeBytes = 0
    try {
      const db = await getDb()
      const v = (await db.runAndReadAll(`SELECT version() AS v`)).getRowObjects()[0] as any
      dbVersion = String(v?.v ?? '')
    } catch { /* DB unavailable — leave nulls */ }
    try {
      const dbPath = path.join(process.cwd(), 'data', 'proxy.duckdb')
      if (fs.existsSync(dbPath)) dbSizeBytes = fs.statSync(dbPath).size
    } catch { /* ignore */ }

    return c.json({
      service: 'open-claude-bridge',
      version: VERSION,
      mode: 'pty',
      host: config.host,
      port: config.port,
      debug: config.debug,
      verbose: config.verbose,
      sessions: {
        total: sessions.length,
        active: sessions.filter(s => s.state === 'running').length,
      },
      os: `${os.type() === 'Windows_NT' ? 'Windows' : os.type() === 'Darwin' ? 'macOS' : os.type()} ${os.release()} (${os.arch()})`,
      nodeVersion: process.version,
      db: { type: dbType, version: dbVersion, sizeBytes: dbSizeBytes },
      // Live host CPU/RAM load shown next to the OS line.
      system: getSystemStats(),
    })
  })

  // GET /api/dashboard/config -- current config (read-only + editable settings)
  api.get('/api/dashboard/config', (c) => {
    const editable = getEditableSettings()
    return c.json({
      port: config.port,
      host: config.host,
      mode: 'pty',
      ...editable,
    })
  })

  // GET /api/dashboard/db/diag -- per-table size diagnostics (in-process
  // version of the manual MODE=diag maintenance script).
  api.get('/api/dashboard/db/diag', async (c) => {
    const { dbDiag } = await import('../../stats/database/maintenance')
    return c.json(await dbDiag())
  })

  // POST /api/dashboard/db/compact -- run a full rewrite+swap now. Blocks DB
  // work briefly; refuses to run concurrently with itself. Manual override of
  // the automatic threshold trigger.
  api.post('/api/dashboard/db/compact', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const { compactDatabase } = await import('../../stats/database/maintenance')
    const result = await compactDatabase()
    return c.json(result, result.ok ? 200 : 409)
  })

  // POST /api/dashboard/db/retention -- run the retention pass (age-out +
  // CHECKPOINT) now.
  api.post('/api/dashboard/db/retention', async (c) => {
    const denied = denyApiToken(c); if (denied) return denied
    const { runRetention } = await import('../../stats/database/maintenance')
    return c.json(await runRetention())
  })
}
