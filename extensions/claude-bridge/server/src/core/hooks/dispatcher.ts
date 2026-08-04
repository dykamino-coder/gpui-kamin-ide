// Server-side dispatcher: receives a webhook from CLI on
// /api/hooks/<sessionId>/<event>/<hookId>, looks up the original handler
// and routes execution per its effectiveHost.
//
//   local  → forward via WS to Electron, await response
//   server → spawn child_process inside the bridge container
//   http   → forward as-is to user's URL (rare; usually pre-resolved)

import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import type { WebSocket as WS } from 'ws'
import { getHook } from './registry'
import type { HookExecutionResult, HookInputPayload, RegisteredHook } from './types'
import { debugLog, warnLog } from '../logging'
import { appendExecution } from './audit-log'

// Match Claude Code's DEFAULT_HTTP_HOOK_TIMEOUT_MS = 10 * 60 * 1000.
// Long-running hooks (formatters, test runners) need this room.
const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000

interface PendingHookExec {
  resolve: (r: HookExecutionResult) => void
  reject: (err: Error) => void
  timer: NodeJS.Timeout | null
  startedAt: number
}
const pendingLocalExecs: Map<string, PendingHookExec> = new Map()

/** Dispatch a webhook payload to its handler's runtime location. */
export async function dispatchHook(
  sessionId: string,
  hookId: string,
  payload: HookInputPayload,
  ws: WS | null,
): Promise<HookExecutionResult> {
  const reg = getHook(sessionId, hookId)
  if (!reg) {
    return errorResult(`Unknown hook ${hookId} for session ${sessionId}`)
  }
  const start = Date.now()
  let result: HookExecutionResult
  try {
    if (reg.effectiveHost === 'local') {
      if (!ws) {
        result = errorResult('No active Electron WS — local hook unavailable', start)
      } else {
        result = await dispatchLocal(reg, payload, ws)
      }
    } else if (reg.effectiveHost === 'server') {
      result = await dispatchServer(reg, payload, start)
    } else if (reg.handler.type === 'http') {
      // http: shouldn't normally land here — the rewriter writes them as
      // direct CLI-side `http` hooks. Fall back to fetch ourselves.
      result = await dispatchHttp(reg, payload, start)
    } else {
      result = errorResult(`Cannot dispatch host ${reg.effectiveHost}`, start)
    }
  } catch (err) {
    result = errorResult(err instanceof Error ? err.message : String(err), start)
  }

  // Audit-log every dispatched hook (success/error/timeout/cancel) so the
  // UI Log tab and downstream compliance tooling can see what happened.
  appendExecution({
    ts: Date.now(),
    sessionId,
    hookId: reg.id,
    event: reg.event,
    matcher: reg.matcher,
    source: reg.source,
    effectiveHost: reg.effectiveHost,
    outcome: result.outcome,
    exitCode: result.exitCode,
    durationMs: result.durationMs,
    stdoutPreview: result.stdout ? result.stdout.slice(0, 256) : undefined,
    stderrPreview: result.stderr ? result.stderr.slice(0, 256) : undefined,
  })
  return result
}

async function dispatchLocal(
  reg: RegisteredHook,
  payload: HookInputPayload,
  ws: WS,
): Promise<HookExecutionResult> {
  if (reg.handler.type !== 'command') {
    return errorResult(`Only command hooks dispatch to local; got type ${reg.handler.type}`)
  }
  const start = Date.now()
  const requestId = randomUUID()
  const timeoutMs = (reg.handler.timeout ?? DEFAULT_TIMEOUT_MS / 1000) * 1000
  return new Promise<HookExecutionResult>((resolve, reject) => {
    const timer = setTimeout(() => {
      pendingLocalExecs.delete(requestId)
      resolve({
        stdout: '', stderr: `Hook timeout after ${timeoutMs}ms`,
        exitCode: 124, outcome: 'timeout', durationMs: Date.now() - start,
      })
    }, timeoutMs)
    pendingLocalExecs.set(requestId, { resolve, reject, timer, startedAt: start })
    try {
      ws.send(JSON.stringify({
        type: 'hook:execute',
        requestId,
        sessionId: reg.sessionId,
        hookId: reg.id,
        event: reg.event,
        command: (reg.handler as { command: string }).command,
        shell: (reg.handler as { shell?: string }).shell ?? 'bash',
        cwd: payload.cwd,
        env: { CLAUDE_BRIDGE_HOOK: '1' },
        payload,
        timeoutMs,
      }))
    } catch (err) {
      clearTimeout(timer)
      pendingLocalExecs.delete(requestId)
      reject(err instanceof Error ? err : new Error(String(err)))
    }
  })
}

/** Resolve a pending local hook from Electron's WS response. */
export function handleLocalHookResponse(requestId: string, result: HookExecutionResult): void {
  const pending = pendingLocalExecs.get(requestId)
  if (!pending) {
    warnLog('Hook response for unknown requestId', { requestId })
    return
  }
  if (pending.timer) clearTimeout(pending.timer)
  pendingLocalExecs.delete(requestId)
  pending.resolve(result)
}

/** Cancel any pending local execs for a session (called on PTY teardown). */
export function cancelSessionLocalExecs(sessionId: string): void {
  for (const [id, p] of pendingLocalExecs) {
    if (p.timer) clearTimeout(p.timer)
    pendingLocalExecs.delete(id)
    p.resolve({
      stdout: '', stderr: `Session ${sessionId} cancelled`,
      exitCode: 130, outcome: 'cancelled', durationMs: Date.now() - p.startedAt,
    })
  }
}

async function dispatchServer(
  reg: RegisteredHook,
  payload: HookInputPayload,
  start: number,
): Promise<HookExecutionResult> {
  if (reg.handler.type !== 'command') {
    return errorResult(`Only command hooks dispatch to server; got type ${reg.handler.type}`, start)
  }
  const command = (reg.handler as { command: string }).command
  const shell = (reg.handler as { shell?: string }).shell ?? 'bash'
  const timeoutMs = (reg.handler.timeout ?? DEFAULT_TIMEOUT_MS / 1000) * 1000

  return new Promise<HookExecutionResult>((resolve) => {
    const proc = spawn(shell === 'powershell' ? 'pwsh' : 'bash', ['-c', command], {
      env: { ...process.env, CLAUDE_BRIDGE_HOOK: '1' },
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    // Cap accumulation: a hook that streams unbounded output (e.g. `yes` or a
    // stuck loop) would otherwise grow these strings until the server OOMs.
    // 1 MiB is far more than any legitimate hook JSON/log; excess is dropped.
    const MAX_HOOK_OUTPUT = 1024 * 1024
    proc.stdout?.on('data', d => { if (stdout.length < MAX_HOOK_OUTPUT) stdout += d.toString() })
    proc.stderr?.on('data', d => { if (stderr.length < MAX_HOOK_OUTPUT) stderr += d.toString() })

    const killer = setTimeout(() => {
      try { proc.kill('SIGKILL') } catch { /* ok */ }
      resolve({
        stdout, stderr: stderr + `\n[hook killed after ${timeoutMs}ms]`,
        exitCode: 124, outcome: 'timeout', durationMs: Date.now() - start,
      })
    }, timeoutMs)

    proc.on('exit', (code) => {
      clearTimeout(killer)
      let jsonOutput: Record<string, unknown> | undefined
      try {
        const trimmed = stdout.trim()
        if (trimmed.startsWith('{')) jsonOutput = JSON.parse(trimmed)
      } catch { /* not JSON */ }
      resolve({
        stdout, stderr,
        exitCode: code ?? 0,
        outcome: code === 0 ? 'success' : 'error',
        jsonOutput,
        durationMs: Date.now() - start,
      })
    })

    proc.on('error', (err) => {
      clearTimeout(killer)
      resolve({
        stdout, stderr: stderr + `\nspawn error: ${err.message}`,
        exitCode: 127, outcome: 'error', durationMs: Date.now() - start,
      })
    })

    // A hook that never reads stdin (or exits early) makes our write emit an
    // async 'error' (EPIPE) on the stdin stream — the try/catch below only
    // covers the synchronous write, so without this listener EPIPE escapes as
    // an uncaughtException.
    proc.stdin?.on('error', () => { /* ignore EPIPE — hook didn't read stdin */ })
    try {
      proc.stdin?.write(JSON.stringify(payload))
      proc.stdin?.end()
    } catch { /* ignore EPIPE */ }
  })
}

async function dispatchHttp(
  reg: RegisteredHook,
  payload: HookInputPayload,
  start: number,
): Promise<HookExecutionResult> {
  if (reg.handler.type !== 'http') return errorResult('not http', start)
  const h = reg.handler
  const timeoutMs = (h.timeout ?? DEFAULT_TIMEOUT_MS / 1000) * 1000
  try {
    const response = await fetch(h.url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...(h.headers ?? {}) },
      body: JSON.stringify(payload),
      signal: AbortSignal.timeout(timeoutMs),
    })
    const stdout = await response.text()
    return {
      stdout, stderr: '',
      exitCode: response.ok ? 0 : 1,
      outcome: response.ok ? 'success' : 'error',
      durationMs: Date.now() - start,
    }
  } catch (err) {
    return errorResult(err instanceof Error ? err.message : String(err), start)
  }
}

function errorResult(msg: string, start: number = Date.now()): HookExecutionResult {
  debugLog('Hook dispatch error', { msg })
  return {
    stdout: '', stderr: msg,
    exitCode: 1, outcome: 'error',
    durationMs: Date.now() - start,
  }
}
