// ============================================================================
// App-wide error log ring buffer (vendored from electron/main/error-log.ts).
// Captures console.error/warn plus feature-specific errors (marketplace git,
// MCP, etc.) so the Customize → Logs panel can surface them to the user.
//
// KAMIN DIFFERENCES vs the Electron original:
//  - NO process-level uncaughtException/unhandledRejection handlers: the
//    extension runs inside KaminIDE's shared ext-host child, which has its
//    own crash containment — adding listeners here would mask it.
//  - The console tee still installs (it only TEEs, never suppresses), but the
//    ext-host console is shared with other extensions, so entries are tagged
//    with an `exthost` source rather than pretending to be bridge-only.
// ============================================================================

import type { BrowserWindow } from '@kaminide/host-compat'
import { CircularBuffer } from './utils/circular-buffer'

export interface LogEntry {
  ts: number
  level: 'error' | 'warn' | 'info'
  source: string
  message: string
  stack?: string
}

const MAX_LOG = 500
const entries = new CircularBuffer<LogEntry>(MAX_LOG)
let targetWindow: BrowserWindow | null = null

export function setLogsWindow(win: BrowserWindow | null): void {
  targetWindow = win
}

export function pushLog(level: LogEntry['level'], source: string, message: string, stack?: string): void {
  const entry: LogEntry = { ts: Date.now(), level, source, message: truncate(message, 4000), stack: stack ? truncate(stack, 8000) : undefined }
  entries.add(entry)
  const win = targetWindow
  if (win && !win.isDestroyed()) {
    try { win.webContents.send('logs:entry', entry) } catch { /* webview may be reloading */ }
  }
}

export function getLogs(): LogEntry[] {
  return entries.toArray()
}

export function clearLogs(): void {
  entries.clear()
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s
  return s.slice(0, max) + `… (+${s.length - max} chars)`
}

// ── Console capture ─────────────────────────────────────────────────────────
// Tee console.error/warn into the ring buffer. Original behaviour (writing to
// the ext-host log) is preserved — we only TEE the output.

let installed = false
export function installConsoleCapture(): void {
  if (installed) return
  installed = true

  const origErr = console.error.bind(console)
  const origWarn = console.warn.bind(console)

  console.error = (...args: unknown[]) => {
    origErr(...args)
    const { message, stack } = stringifyArgs(args)
    pushLog('error', 'exthost', message, stack)
  }
  console.warn = (...args: unknown[]) => {
    origWarn(...args)
    const { message } = stringifyArgs(args)
    pushLog('warn', 'exthost', message)
  }
}

function stringifyArgs(args: unknown[]): { message: string; stack?: string } {
  const parts: string[] = []
  let stack: string | undefined
  for (const a of args) {
    if (a instanceof Error) {
      parts.push(a.message)
      if (!stack && a.stack) stack = a.stack
    } else if (typeof a === 'string') {
      parts.push(a)
    } else {
      try { parts.push(JSON.stringify(a)) } catch { parts.push(String(a)) }
    }
  }
  return { message: parts.join(' '), stack }
}
