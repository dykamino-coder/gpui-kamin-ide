// Server-side dispatcher: receives a webhook from CLI on
// /api/hooks/<sessionId>/<event>/<hookId>, looks up the original handler
// and routes execution per its effectiveHost.
//
//   local  → forward via WS to the VSIX bridge host, await response
//   server → spawn child_process inside the bridge container
//   http   → forward as-is to user's URL (rare; usually pre-resolved)

import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import type { WebSocket as WS } from 'ws'
import { getHook } from './registry'
import type { HookExecutionResult, HookInputPayload, RegisteredHook } from './types'
import { debugLog, warnLog } from '../logging'
import { appendExecution } from './audit-log'
import { payloadForHook } from './tool-names'

// Match Claude Code's DEFAULT_HTTP_HOOK_TIMEOUT_MS = 10 * 60 * 1000.
// Long-running hooks (formatters, test runners) need this room.
const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000

interface PendingHookExec {
  sessionId: string
  resolve: (r: HookExecutionResult) => void
  startedAt: number
}
const pendingLocalExecs: Map<string, PendingHookExec> = new Map()

/** How long a local hook waits for a dropped client to reattach before it is
 * given up. The client retries after 1 s and then 2 s, so this covers its
 * first two attempts without holding a turn for long. */
export const LOCAL_HOOK_REATTACH_WAIT_MS = 5000
const REATTACH_POLL_MS = 100

/** The client socket of a session: a fixed socket, or a reader of the current
 * one. Only a reader can observe a reattach, so only a reader waits. */
export type ClientWsSource = WS | null | (() => WS | null)

/** Sessions whose absent client was already reported. Further local hooks of
 * the same disconnect are skipped without a console line. */
const absentClientReported: Set<string> = new Set()

/** Sessions whose reattach wait already ran out. Reporting and waiting are
 * tracked apart: a hook lost in flight reports the disconnect, and the next
 * hook still gets its one bounded wait for the client to come back. */
const reattachGivenUp: Set<string> = new Set()

function openClientWs(source: ClientWsSource): WS | null {
  const ws = typeof source === 'function' ? source() : source
  return ws?.readyState === 1 ? ws : null
}

async function waitForReattach(sessionId: string, source: ClientWsSource): Promise<WS | null> {
  if (typeof source !== 'function' || reattachGivenUp.has(sessionId)) return null
  // The wait ends before the hook is sent, never after: a command that was
  // sent may already have had its side effect and is not sent again.
  const deadline = Date.now() + LOCAL_HOOK_REATTACH_WAIT_MS
  while (Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, REATTACH_POLL_MS))
    const ws = openClientWs(source)
    if (ws) return ws
  }
  reattachGivenUp.add(sessionId)
  return null
}

/** Dispatch a webhook payload to its handler's runtime location. */
export async function dispatchHook(
  sessionId: string,
  hookId: string,
  payload: HookInputPayload,
  wsSource: ClientWsSource,
  auditTokenId?: string,
): Promise<HookExecutionResult> {
  const reg = getHook(sessionId, hookId)
  if (!reg) {
    return errorResult(`Unknown hook ${hookId} for session ${sessionId}`)
  }
  const start = Date.now()
  payload = payloadForHook(reg.matcher, payload)
  let result: HookExecutionResult
  let auditNote: string | undefined
  try {
    if (reg.effectiveHost === 'local') {
      // An open socket is used at once, in the same tick as before.
      const ws = openClientWs(wsSource) ?? (await waitForReattach(sessionId, wsSource))
      if (ws) {
        absentClientReported.delete(sessionId)
        reattachGivenUp.delete(sessionId)
        result = await dispatchLocal(reg, payload, ws)
      } else if (absentClientReported.has(sessionId)) {
        auditNote = 'Skipped: client host is still disconnected'
        result = { stdout: '', stderr: '', exitCode: 0, outcome: 'cancelled', durationMs: Date.now() - start }
      } else {
        absentClientReported.add(sessionId)
        result = errorResult(
          'Client-host WS is not open — local hooks are skipped until the client reconnects',
          start,
          reg,
        )
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
  // Where a hook's time goes: `hostMs` is what the executing host measured
  // (the client host for local hooks), the rest of `totalMs` is the server and
  // the WebSocket. The CLI's relay start-up lies outside both.
  debugLog('Hook dispatched', {
    sessionId,
    event: reg.event,
    host: reg.effectiveHost,
    outcome: result.outcome,
    totalMs: Date.now() - start,
    hostMs: result.durationMs,
  })

  // Audit-log every dispatched hook (success/error/timeout/cancel) so the
  // UI Log tab and downstream compliance tooling can see what happened.
  appendExecution({
    ts: Date.now(),
    tokenId: auditTokenId,
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
    stderrPreview: (auditNote ?? result.stderr)?.slice(0, 256) || undefined,
  })
  return result
}

async function dispatchLocal(reg: RegisteredHook, payload: HookInputPayload, ws: WS): Promise<HookExecutionResult> {
  if (reg.handler.type !== 'command') {
    return errorResult(`Only command hooks dispatch to local; got type ${reg.handler.type}`)
  }
  if (ws.readyState !== 1) {
    return errorResult('Client-host WS is not open — local hook unavailable', Date.now(), reg)
  }
  const start = Date.now()
  const requestId = randomUUID()
  const timeoutMs = (reg.handler.timeout ?? DEFAULT_TIMEOUT_MS / 1000) * 1000
  return new Promise<HookExecutionResult>((resolve) => {
    // Закрытие WS не закрывает HTTP relay: возвращаем результат ровно один
    // раз. Повтор на новом WS опасен: команда могла уже выполнить побочный эффект.
    const finish = (result: HookExecutionResult) => {
      if (!pendingLocalExecs.delete(requestId)) return
      clearTimeout(timer)
      ws.off('close', disconnected)
      ws.off('error', disconnected)
      resolve(result)
    }
    const disconnected = () => {
      // This line is the report of the disconnect; later hooks stay quiet.
      absentClientReported.add(reg.sessionId)
      finish(errorResult('Client-host WS disconnected during local hook', start, reg))
    }
    const timer = setTimeout(() => {
      finish({
        stdout: '',
        stderr: `Hook timeout after ${timeoutMs}ms`,
        exitCode: 124,
        outcome: 'timeout',
        durationMs: Date.now() - start,
      })
    }, timeoutMs)
    pendingLocalExecs.set(requestId, { sessionId: reg.sessionId, resolve: finish, startedAt: start })
    ws.once('close', disconnected)
    ws.once('error', disconnected)
    try {
      ws.send(
        JSON.stringify({
          type: 'hook:execute',
          requestId,
          sessionId: reg.sessionId,
          hookId: reg.id,
          event: reg.event,
          command: (reg.handler as { command: string }).command,
          args: (reg.handler as { args?: string[] }).args,
          pluginId: reg.source.kind === 'plugin' ? reg.source.pluginId : undefined,
          // Preserve "unspecified": the Windows client may fall back from Git
          // Bash to PowerShell. An explicit shell remains strict.
          shell: (reg.handler as { shell?: string }).shell,
          cwd: payload.cwd,
          env: { CLAUDE_BRIDGE_HOOK: '1' },
          payload,
          timeoutMs,
        }),
        (err) => {
          if (err) disconnected()
        },
      )
    } catch {
      disconnected()
    }
  })
}

/** Resolve a pending local hook from the VSIX bridge host's WS response. */
export function handleLocalHookResponse(requestId: string, result: HookExecutionResult): void {
  const pending = pendingLocalExecs.get(requestId)
  if (!pending) {
    warnLog('Hook response for unknown requestId', { requestId })
    return
  }
  pending.resolve(result)
}

/** Cancel any pending local execs for a session (called on PTY teardown). */
export function cancelSessionLocalExecs(sessionId: string): void {
  absentClientReported.delete(sessionId)
  reattachGivenUp.delete(sessionId)
  for (const p of pendingLocalExecs.values()) {
    if (p.sessionId !== sessionId) continue
    p.resolve({
      stdout: '',
      stderr: `Session ${sessionId} cancelled`,
      exitCode: 130,
      outcome: 'cancelled',
      durationMs: Date.now() - p.startedAt,
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
    let stdoutDropped = 0
    // setEncoding decodes across chunk boundaries; per-chunk toString split
    // multi-byte characters into U+FFFD.
    proc.stdout?.setEncoding('utf8')
    proc.stderr?.setEncoding('utf8')
    proc.stdout?.on('data', (d: string) => {
      const room = MAX_HOOK_OUTPUT - stdout.length
      if (room > 0) stdout += d.slice(0, room)
      stdoutDropped += Math.max(0, d.length - Math.max(0, room))
    })
    proc.stderr?.on('data', (d: string) => {
      if (stderr.length < MAX_HOOK_OUTPUT) stderr += d.slice(0, MAX_HOOK_OUTPUT - stderr.length)
    })
    // A cut stdout is reported, never passed on as the hook's whole output.
    const cutNote = (): string =>
      stdoutDropped ? `\n[hook stdout exceeded ${MAX_HOOK_OUTPUT} characters; ${stdoutDropped} were dropped]` : ''

    const killer = setTimeout(() => {
      try {
        proc.kill('SIGKILL')
      } catch {
        /* ok */
      }
      resolve({
        stdout,
        stderr: stderr + cutNote() + `\n[hook killed after ${timeoutMs}ms]`,
        exitCode: 124,
        outcome: 'timeout',
        durationMs: Date.now() - start,
      })
    }, timeoutMs)

    proc.on('exit', (code) => {
      clearTimeout(killer)
      let jsonOutput: Record<string, unknown> | undefined
      if (code === 0 && !stdoutDropped) {
        try {
          const trimmed = stdout.trim()
          if (trimmed.startsWith('{')) jsonOutput = JSON.parse(trimmed)
        } catch {
          /* not JSON */
        }
      }
      resolve({
        stdout,
        stderr: stderr + cutNote(),
        exitCode: code ?? 0,
        outcome: code === 0 ? 'success' : 'error',
        jsonOutput,
        durationMs: Date.now() - start,
      })
    })

    proc.on('error', (err) => {
      clearTimeout(killer)
      resolve({
        stdout,
        stderr: stderr + `\nspawn error: ${err.message}`,
        exitCode: 127,
        outcome: 'error',
        durationMs: Date.now() - start,
      })
    })

    // A hook that never reads stdin (or exits early) makes our write emit an
    // async 'error' (EPIPE) on the stdin stream — the try/catch below only
    // covers the synchronous write, so without this listener EPIPE escapes as
    // an uncaughtException.
    proc.stdin?.on('error', () => {
      /* ignore EPIPE — hook didn't read stdin */
    })
    try {
      proc.stdin?.write(JSON.stringify(payload))
      proc.stdin?.end()
    } catch {
      /* ignore EPIPE */
    }
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
      stdout,
      stderr: '',
      exitCode: response.ok ? 0 : 1,
      outcome: response.ok ? 'success' : 'error',
      durationMs: Date.now() - start,
    }
  } catch (err) {
    return errorResult(err instanceof Error ? err.message : String(err), start)
  }
}

function errorResult(msg: string, start: number = Date.now(), reg?: RegisteredHook): HookExecutionResult {
  if (reg) {
    const context = { event: reg.event, type: reg.handler.type, host: reg.effectiveHost }
    msg = `Bridge hook dispatch ${JSON.stringify(context)}: ${msg}`
  }
  debugLog('Hook dispatch error', { msg })
  return {
    stdout: '',
    stderr: msg,
    exitCode: 1,
    outcome: 'error',
    durationMs: Date.now() - start,
  }
}
