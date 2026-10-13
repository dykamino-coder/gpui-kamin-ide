// Live subscription limits and context fill for a running chat session.
//
// Claude Code runs the configured status-line command with a JSON snapshot on
// stdin after assistant messages, mode changes and rate-limit window rollovers
// (https://code.claude.com/docs/en/statusline). `rate_limits.five_hour` and
// `rate_limits.seven_day` (2.1.80+; claude.ai Pro/Max, after the first API
// response) carry `used_percentage` and `resets_at`; `context_window`
// (2.1.6+) carries `used_percentage` and `context_window_size`.
//
// The session's settings.json declares a status-line command that keeps only
// those fields and writes them atomically next to the settings; the server
// watches that file and pushes `session:usage` over the session's WebSocket.
// The child needs no network and no credential, and the raw snapshot (paths,
// session ids, cost) never leaves the container. Window parsing is shared
// with the dashboard plan-usage view (BR-20) via `parseStatusline`.

import fs from 'fs'
import path from 'path'

import { debugLog, warnLog } from '../logging'
import { parseStatusline } from '../server/utils/plan-usage'
import { quoteCaptureArgument } from '../server/utils/usage-statusline'
import { sendToClient } from './session-io'
import type { PtySession } from '../types/pty'

export const STATUSLINE_SCRIPT_NAME = 'bridge-statusline.cjs'
export const STATUSLINE_RESULT_NAME = 'bridge-statusline.json'
const MAX_RESULT_BYTES = 4096

const SCRIPT = String.raw`
const fs = require('node:fs');
let input = ''; let bytes = 0;
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { bytes += Buffer.byteLength(chunk); if (bytes <= 262144) input += chunk; });
process.stdin.on('end', () => {
  if (bytes > 262144) return;
  try {
    const data = JSON.parse(input);
    const pct = v => typeof v === 'number' && Number.isFinite(v) && v >= 0 && v <= 100;
    const rate_limits = {};
    for (const name of ['five_hour', 'seven_day']) {
      const value = data.rate_limits && data.rate_limits[name];
      if (value && pct(value.used_percentage) && Number.isSafeInteger(value.resets_at) && value.resets_at > 0 && value.resets_at <= 253402300799)
        rate_limits[name] = { used_percentage: value.used_percentage, resets_at: value.resets_at };
    }
    const cw = data.context_window || {};
    const context = {
      used_percentage: pct(cw.used_percentage) ? cw.used_percentage : null,
      size: Number.isSafeInteger(cw.context_window_size) && cw.context_window_size > 0 ? cw.context_window_size : null,
    };
    const word = v => typeof v === 'string' && /^[A-Za-z0-9._:@/\[\]-]{1,160}$/.test(v) ? v : null;
    const version = typeof data.version === 'string' && /^\d+\.\d+\.\d+(?:[-+][\w.-]{1,32})?$/.test(data.version) ? data.version : null;
    const result = JSON.stringify({
      observedAt: new Date().toISOString(), version, rate_limits, context,
      model: word(data.model && data.model.id), effort: word(data.effort && data.effort.level),
    });
    const target = process.argv[2]; const temp = target + '.' + process.pid + '.tmp';
    fs.writeFileSync(temp, result, { mode: 0o600 }); fs.renameSync(temp, target);
  } catch { /* never echo stdin, paths or errors */ }
});
`

export interface SessionUsageWindow {
  id: string
  label: string
  percent: number
  resetsAt: string
}

/** Wire payload of `session:usage` (server → VSIX host → webview). */
export interface SessionUsage {
  windows: SessionUsageWindow[]
  contextPercent: number | null
  contextWindowSize: number | null
  model: string | null
  effort: string | null
  observedAt: string
}

function clean(value: unknown): string | null {
  return typeof value === 'string' && /^[A-Za-z0-9._:@/[\]-]{1,160}$/.test(value) ? value : null
}

/** Sanitized result file → wire payload. Expired windows are dropped (the
 *  CLI drops them too once `resets_at` passes). */
export function parseSessionStatus(raw: unknown): SessionUsage | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const data = raw as Record<string, unknown>
  const { windows } = parseStatusline(data)
  const context = data.context && typeof data.context === 'object' ? (data.context as Record<string, unknown>) : {}
  const pct = context.used_percentage
  const size = context.size
  return {
    windows: windows.map((w) => ({ id: w.id, label: w.label, percent: w.percent, resetsAt: w.resetsAt ?? '' })),
    contextPercent: typeof pct === 'number' && Number.isFinite(pct) && pct >= 0 && pct <= 100 ? pct : null,
    contextWindowSize: typeof size === 'number' && Number.isSafeInteger(size) && size > 0 ? size : null,
    model: clean(data.model),
    effort: clean(data.effort),
    observedAt:
      windows[0]?.observedAt ?? (typeof data.observedAt === 'string' ? data.observedAt : new Date().toISOString()),
  }
}

/** Write the per-session status-line script and return the `statusLine`
 *  setting that runs it with the Bridge's own Node. */
export function writeStatusLineScript(claudeDir: string): { type: 'command'; command: string } {
  const script = path.join(claudeDir, STATUSLINE_SCRIPT_NAME)
  const result = path.join(claudeDir, STATUSLINE_RESULT_NAME)
  fs.writeFileSync(script, SCRIPT, { mode: 0o600 })
  const command = [process.execPath, script, result].map((value) => quoteCaptureArgument(value)).join(' ')
  return { type: 'command', command }
}

const lastUsage = new WeakMap<PtySession, SessionUsage>()

export function sessionUsageMessage(session: PtySession, usage: SessionUsage): Record<string, unknown> {
  return { type: 'session:usage', sessionId: session.id, usage }
}

/** Re-send the latest snapshot to a client that (re)attached. */
export function resendSessionUsage(session: PtySession): void {
  const usage = lastUsage.get(session)
  if (usage) sendToClient(session.ws, sessionUsageMessage(session, usage))
}

/** Read the result file once and push it when it changed. Exported for tests. */
export function readSessionStatus(session: PtySession, resultPath: string, previous: { key: string }): void {
  let text: string
  try {
    if (fs.statSync(resultPath).size > MAX_RESULT_BYTES) return
    text = fs.readFileSync(resultPath, 'utf8')
  } catch {
    return
  }
  let usage: SessionUsage | null
  try {
    usage = parseSessionStatus(JSON.parse(text))
  } catch {
    return
  }
  if (!usage) return
  const { observedAt: _observedAt, ...comparable } = usage
  const key = JSON.stringify(comparable)
  if (key === previous.key) return
  previous.key = key
  lastUsage.set(session, usage)
  sendToClient(session.ws, sessionUsageMessage(session, usage))
}

/** Watch the session's status-line result file. */
export function watchSessionStatusLine(session: PtySession, claudeDir: string): { stop(): void } {
  const resultPath = path.join(claudeDir, STATUSLINE_RESULT_NAME)
  const previous = { key: '' }
  let timer: ReturnType<typeof setTimeout> | null = null
  let watcher: fs.FSWatcher | null = null
  const schedule = (): void => {
    if (timer) return
    timer = setTimeout(() => {
      timer = null
      readSessionStatus(session, resultPath, previous)
    }, 50)
  }
  try {
    watcher = fs.watch(claudeDir, (_event, file) => {
      if (!file || String(file) === STATUSLINE_RESULT_NAME) schedule()
    })
    watcher.on('error', (error) =>
      warnLog('Status-line watcher failed', { sessionId: session.id, error: String(error) }),
    )
  } catch (error) {
    warnLog('Status-line watcher unavailable', { sessionId: session.id, error: String(error) })
  }
  // A resumed session's directory may already hold the last snapshot.
  schedule()
  debugLog('Status-line watcher started', { sessionId: session.id })
  return {
    stop(): void {
      if (timer) clearTimeout(timer)
      timer = null
      watcher?.close()
      watcher = null
    },
  }
}
