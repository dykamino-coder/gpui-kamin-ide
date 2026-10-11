// Persistent metadata only. Legacy logging accepts arbitrary payloads and must
// never share this volume. One server process owns the fixed rotation files.
import { createHmac, randomBytes, randomUUID } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'

export const LIFECYCLE_FILE_BYTES = 1024 * 1024
export const LIFECYCLE_BACKUPS = 4
const boot = randomUUID()
const secret = randomBytes(32)
let sequence = 0
let lastWarning = -Infinity

const EVENTS = new Set([
  'server_start',
  'server_shutdown',
  'session_create',
  'session_detach',
  'session_reattach',
  'session_destroy',
  'session_finalize',
  'pty_exit',
  'resume_reuse',
  'resume_wait',
])
const REASONS = new Set([
  'unspecified',
  'explicit_end',
  'dashboard_kill',
  'replace_start',
  'replace_resume',
  'effort_restart',
  'model_restart',
  'startup_failed',
  'detach_not_running',
  'detach_grace',
  'startup_timeout',
  'idle_timeout',
  'max_lifetime',
  'session-end',
  'pty-exit',
  'timeout',
  'shutdown',
])
export type DestroyReason =
  | 'unspecified'
  | 'explicit_end'
  | 'dashboard_kill'
  | 'replace_start'
  | 'replace_resume'
  | 'effort_restart'
  | 'model_restart'
  | 'startup_failed'
  | 'detach_not_running'
  | 'detach_grace'
  | 'startup_timeout'
  | 'idle_timeout'
  | 'max_lifetime'

/** Values are checked again at runtime; no arbitrary strings enter the file. */
export function lifecycleLog(event: string, fields: Record<string, unknown> = {}): void {
  if (!EVENTS.has(event)) return
  const record: Record<string, unknown> = {
    v: 1,
    timestamp: new Date().toISOString(),
    boot,
    sequence: ++sequence,
    event,
  }
  if (typeof fields.sessionId === 'string') {
    record.session = createHmac('sha256', secret).update(fields.sessionId).digest('hex').slice(0, 24)
  }
  if (typeof fields.reason === 'string' && REASONS.has(fields.reason)) record.reason = fields.reason
  for (const name of ['ageMs', 'idleMs', 'graceMs', 'waitedMs', 'exitCode']) {
    const value = fields[name]
    if (typeof value === 'number' && Number.isSafeInteger(value)) record[name] = value
  }
  for (const name of ['isSubAgent', 'suppressed']) {
    if (typeof fields[name] === 'boolean') record[name] = fields[name]
  }
  // No queue, handles or timer survive a write. Low-frequency lifecycle writes
  // are synchronous so a normal process exit cannot lose a queued termination.
  try {
    const directory = process.env.BRIDGE_LIFECYCLE_LOG_DIR || path.join(process.cwd(), 'logs', 'lifecycle')
    fs.mkdirSync(directory, { recursive: true, mode: 0o700 })
    const file = path.join(directory, 'lifecycle.jsonl')
    const line = JSON.stringify(record) + '\n'
    let size = 0
    try {
      size = fs.statSync(file).size
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    }
    if (size + Buffer.byteLength(line) > LIFECYCLE_FILE_BYTES) {
      fs.rmSync(`${file}.${LIFECYCLE_BACKUPS}`, { force: true })
      for (let n = LIFECYCLE_BACKUPS - 1; n >= 0; n--) {
        const from = n ? `${file}.${n}` : file
        try {
          fs.renameSync(from, `${file}.${n + 1}`)
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
        }
      }
    }
    fs.appendFileSync(file, line, { mode: 0o600 })
  } catch {
    // Report only a fixed outcome; filesystem errors may contain private paths.
    if (Date.now() - lastWarning >= 60000) {
      lastWarning = Date.now()
      console.warn('[bridge] Lifecycle journal unavailable; lifecycle event omitted')
    }
  }
}
