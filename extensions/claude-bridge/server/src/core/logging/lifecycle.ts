// Persistent metadata only. Legacy logging accepts arbitrary payloads and must
// never share this volume. One server process owns the fixed rotation files.
import { createHash, createHmac, randomBytes, randomUUID } from 'node:crypto'
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
  // INC-2026-0070: what the client asked for and which identity it received.
  'session_request',
  'conversation_id',
])
const REQUEST_KINDS = new Set(['create', 'resume'])
// How createSession chose the settings directory: `found` = lookup located the
// requested transcript, `fresh` = no resume requested, `fresh_not_found` =
// resume requested but the transcript exists nowhere, `reuse` = restart.
const SETTINGS_DIR_SOURCES = new Set(['found', 'fresh', 'fresh_not_found', 'reuse'])
// Why a session:conversation-id was published: first discovery on the session,
// a changed id on a running session (compact chain / newer file) or the
// re-send that accompanies a live reattach.
const ID_SOURCES = new Set(['discovered', 'changed', 'reattach'])
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

/** Stable pseudonym of a CLI conversation id (INC-2026-0070). Unkeyed on
 *  purpose: the same id must map to the same ref across container recreations
 *  (the per-boot HMAC above cannot) and in the KaminIDE client's incident log,
 *  which computes the identical value. 48 bits of a domain-separated digest do
 *  not reveal a random UUID; only a holder of the raw id can confirm a match.
 *  Keep the domain string in sync with the client's copy. */
export function conversationRef(id: string): string {
  return createHash('sha256').update(`kamin-conversation-ref-v1:${id}`).digest('hex').slice(0, 12)
}

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
  for (const name of ['isSubAgent', 'suppressed', 'resumeRequested', 'resumeNotFound', 'settingsDirFound']) {
    if (typeof fields[name] === 'boolean') record[name] = fields[name]
  }
  // Raw conversation ids never enter the file — only their stable refs.
  for (const [name, ref] of [
    ['resumeConversationId', 'resumeRef'],
    ['resolvedConversationId', 'resolvedRef'],
    ['conversationId', 'conversationRef'],
    ['previousConversationId', 'previousRef'],
  ] as const) {
    const value = fields[name]
    if (typeof value === 'string' && value.length > 0) record[ref] = conversationRef(value)
  }
  if (typeof fields.requestKind === 'string' && REQUEST_KINDS.has(fields.requestKind)) {
    record.requestKind = fields.requestKind
  }
  if (typeof fields.settingsDirSource === 'string' && SETTINGS_DIR_SOURCES.has(fields.settingsDirSource)) {
    record.settingsDirSource = fields.settingsDirSource
  }
  if (typeof fields.idSource === 'string' && ID_SOURCES.has(fields.idSource)) record.idSource = fields.idSource
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
