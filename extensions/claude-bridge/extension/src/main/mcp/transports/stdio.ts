// stdio transport for MCP servers.
//
// Spawns the child process with the configured command + args + env (shell:true
// on Windows so PATH lookups behave the same as a user shell). JSON-RPC frames
// are newline-delimited on stdout. stderr is mirrored to console AND split
// per-line into the UI log pane so non-devs can diagnose boot failures.

import { spawn } from 'child_process'
import type { McpResult } from '../tool-registry'
import { collectMcpList } from '../pagination'
import type { McpServerState, TransportContext } from './context'

export async function connectStdio(ctx: TransportContext, id: string): Promise<void> {
  const state = ctx.servers.get(id)!
  const command = state.config.command
  if (!command) throw new Error('stdio server command is required')

  const args = state.config.args ?? []

  // Замерено на машине с корпоративным DLP/прокси: `npx -y <pkg>@latest`
  // отвечает на initialize через ~29 СЕКУНД (резолв реестра до старта
  // сервера). Прежний 30с-таймаут убивал процесс за долю секунды до
  // готовности и заводил вечный цикл reconnect → npx → SIGTERM.
  const CONNECT_TIMEOUT_MS = 90_000

  return new Promise<void>((resolve, reject) => {
    let settled = false
    const timeout = setTimeout(() => {
      if (settled) return
      settled = true
      ctx.appendLog(id, 'error', `[stdio] connect timeout after ${CONNECT_TIMEOUT_MS / 1000}s — no initialize response; killing child`)
      try { state.process?.kill() } catch { /* already dead */ }
      reject(new Error(`Connection timeout (${CONNECT_TIMEOUT_MS / 1000}s)`))
    }, CONNECT_TIMEOUT_MS)
    const finish = (ok: boolean, err?: Error) => {
      if (settled) return
      settled = true
      clearTimeout(timeout)
      if (ok) resolve()
      else reject(err ?? new Error('Unknown error'))
    }

    ctx.appendLog(id, 'info', `[stdio] spawning: ${command} ${args.join(' ')}`)
    const proc = spawn(command, args, {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: { ...process.env, ...state.config.env },
      shell: true,
      // windowsHide: true keeps conhost.exe windows out of Task Manager —
      // every `shell: true` spawn on Windows otherwise creates a visible
      // Console Window Host process per MCP server (so 6 stdio MCPs = 6
      // conhosts cluttering the process tree).
      windowsHide: true,
      ...(state.config.workingDirectory ? { cwd: state.config.workingDirectory } : {}),
    })

    state.process = proc
    state.inputBuffer = ''
    ctx.appendLog(id, 'info', `[stdio] spawned pid=${proc.pid ?? 'n/a'}`)

    let stderrBuf = ''
    // Circuit breaker: some MCP servers (e.g. unity-mcp) hammer their own
    // internal reconnect loop and dump identical errors several times per
    // second when the upstream (named pipe / socket / external app) is
    // offline. Kill the child after either:
    //   (a) a sustained burst — N total stderr lines in W seconds, or
    //   (b) the SAME error line repeated R times in a row.
    const STDERR_BURST_LIMIT = 20     // lines
    const STDERR_BURST_WINDOW = 10000 // 10s
    const STDERR_REPEAT_LIMIT = 8     // identical-line streak
    let stderrTimes: number[] = []
    let lastLine = ''
    let lastLineCount = 0
    let circuitTripped = false
    // Сколько строк stderr пускаем в консоль процесса (панель логов — без лимита)
    const STDERR_CONSOLE_LIMIT = 5
    let stderrConsoleCount = 0
    const tripCircuit = (reason: string) => {
      if (circuitTripped) return
      circuitTripped = true
      const msg = `[stdio] circuit breaker tripped — ${reason}; killing child to stop the spam loop`
      console.error(`[MCP ${state.config.name}]`, msg)
      ctx.appendLog(id, 'error', msg)
      state.status = 'error'
      state.error = 'Circuit breaker: too many stderr errors'
      try { proc.kill() } catch { /* already dead */ }
      ctx.notifyChanged()
      // Settle a pending connect Promise so the manager doesn't sit on the
      // 30s timeout — reject immediately as a connect failure.
      finish(false, new Error('Circuit breaker: too many stderr errors'))
    }
    proc.stderr?.on('data', (data: Buffer) => {
      if (circuitTripped) return
      const text = data.toString()
      stderrBuf += text
      let idx: number
      while ((idx = stderrBuf.indexOf('\n')) !== -1) {
        const line = stderrBuf.slice(0, idx).replace(/\r$/, '')
        stderrBuf = stderrBuf.slice(idx + 1)
        if (!line) continue
        const now = Date.now()
        stderrTimes = stderrTimes.filter(t => now - t < STDERR_BURST_WINDOW)
        stderrTimes.push(now)
        if (line === lastLine) lastLineCount++
        else { lastLine = line; lastLineCount = 1 }
        if (stderrTimes.length > STDERR_BURST_LIMIT) {
          tripCircuit(`${stderrTimes.length} stderr lines in ${STDERR_BURST_WINDOW / 1000}s`)
          return
        }
        if (lastLineCount > STDERR_REPEAT_LIMIT) {
          tripCircuit(`identical line repeated ${lastLineCount}× ("${line.slice(0, 80)}")`)
          return
        }
        // В КОНСОЛЬ — только первые строки: каждая идёт через IPC ребёнок →
        // родитель → stdout → клиент, и на холодном старте сломанный сервер
        // (unity-mcp: 168 строк за прогон) забивал этот путь. В панель логов
        // пишем ВСЁ — там оно и нужно.
        // Файл сервера отсутствует (плагин снесли, запись осталась) — это не
        // лечится ретраями; exit-хендлер увидит диагноз и не станет реконнектить.
        if (line.includes('Cannot find module') || line.includes('MODULE_NOT_FOUND')) {
          state.permanentFailure = line.trim()
        }
        stderrConsoleCount += 1
        if (stderrConsoleCount <= STDERR_CONSOLE_LIMIT) {
          console.error(`[MCP ${state.config.name}] stderr:`, line)
        } else if (stderrConsoleCount === STDERR_CONSOLE_LIMIT + 1) {
          console.error(`[MCP ${state.config.name}] stderr: …дальше только в панель логов`)
        }
        ctx.appendLog(id, 'stderr', line)
      }
    })

    proc.stdout?.on('data', (data: Buffer) => {
      state.inputBuffer += data.toString()

      // Try to parse complete JSON-RPC messages (newline-delimited)
      let newlineIdx: number
      while ((newlineIdx = state.inputBuffer.indexOf('\n')) !== -1) {
        const line = state.inputBuffer.slice(0, newlineIdx).trim()
        state.inputBuffer = state.inputBuffer.slice(newlineIdx + 1)

        if (!line) continue
        try {
          const msg = JSON.parse(line)
          ctx.handleServerMessage(id, msg, (response) => stdioSend(state, response))
        } catch {
          // Non-JSON line on stdout — some servers log informational text
          // there instead of stderr. Surface it in the log so it's visible.
          ctx.appendLog(id, 'stdout', line)
        }
      }
    })

    proc.on('error', (err) => {
      ctx.appendLog(id, 'error', `[stdio] spawn error: ${err.message}`)
      state.status = 'error'
      state.error = err.message
      finish(false, err)
    })

    proc.on('exit', (code, signal) => {
      ctx.appendLog(id, code === 0 ? 'info' : 'warn', `[stdio] process exited code=${code} signal=${signal ?? 'none'}`)
      state.status = 'disconnected'
      state.tools = []
      ctx.notifyChanged()
      ctx.onToolsChanged?.()
      // If the child died before initialize came back, fail the connect
      // *now* instead of waiting for the 30s timeout. Much better UX — the
      // user sees `failed — process exited code=1` the moment it happens.
      finish(false, new Error(`Process exited early (code=${code ?? 'null'})`))
      // Перманентная поломка (MODULE_NOT_FOUND и т.п.): реконнект гонял бы
      // пустые спавны node на каждом буте — сразу error без ретраев.
      if (state.permanentFailure) {
        state.status = 'error'
        state.error = `Server file is broken (fix or remove the entry): ${state.permanentFailure}`
        ctx.appendLog(id, 'error', `[stdio] permanent failure — auto-reconnect disabled: ${state.permanentFailure}`)
        ctx.notifyChanged()
        return
      }
      // Trigger manager-side auto-reconnect with backoff. Skipped when the
      // manager raised `expectedDisconnect` first (user disabled the
      // server, app shutting down) — see TransportContext docs.
      ctx.onUnexpectedDisconnect?.(id, `process exited code=${code ?? 'null'} signal=${signal ?? 'none'}`)
    })

    // Send initialize
    const initId = state.nextRequestId++
    const initPromise = new Promise<any>((res, rej) => {
      state.pendingRequests.set(initId, { resolve: res, reject: rej })
    })

    ctx.appendLog(id, 'info', '[stdio] → initialize')
    stdioSend(state, {
      jsonrpc: '2.0',
      id: initId,
      method: 'initialize',
      params: {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'open-claude-bridge', version: '4.0.0' },
      },
    })

    initPromise.then(async (initResult) => {
      if (initResult.error) {
        ctx.appendLog(id, 'error', `[stdio] initialize error: ${initResult.error.message}`)
        finish(false, new Error(initResult.error.message))
        return
      }

      // Store server-declared capabilities for later capability-gated calls.
      state.capabilities = initResult.result?.capabilities ?? {}
      const serverInfo = initResult.result?.serverInfo
      if (serverInfo?.name) {
        ctx.appendLog(id, 'info', `[stdio] ← initialize ok: server "${serverInfo.name}" v${serverInfo.version ?? '?'}`)
      } else {
        ctx.appendLog(id, 'info', '[stdio] ← initialize ok')
      }

      // Send initialized notification
      stdioSend(state, { jsonrpc: '2.0', method: 'notifications/initialized' })

      // Get tools
      ctx.appendLog(id, 'info', '[stdio] → tools/list')
      const tools = await collectMcpList<any>(
        (method, params) => stdioJsonRpc(ctx, id, method, params),
        'tools/list',
        'tools',
      )
      state.tools = tools.map((tool: { name: string }) => tool.name)
      state.toolSchemas.clear()
      for (const tool of tools) {
        state.toolSchemas.set(tool.name, tool)
      }
      ctx.appendLog(id, 'info', `[stdio] ← tools/list: ${state.tools.length} tools (${state.tools.slice(0, 5).join(', ')}${state.tools.length > 5 ? ', …' : ''})`)

      finish(true)
    }).catch((err) => {
      const m = err instanceof Error ? err.message : String(err)
      ctx.appendLog(id, 'error', `[stdio] initialize/tools failed: ${m}`)
      finish(false, err)
    })
  })
}

export async function callStdioTool(
  ctx: TransportContext,
  id: string,
  toolName: string,
  input: Record<string, unknown>,
): Promise<McpResult> {
  const state = ctx.servers.get(id)!
  if (!state.process) {
    return { content: [{ type: 'text', text: `Error: Server "${state.config.name}" process not running` }] }
  }

  const reqId = state.nextRequestId++
  const promise = new Promise<any>((resolve, reject) => {
    const entry = { resolve, reject, timer: undefined as ReturnType<typeof setTimeout> | undefined }
    state.pendingRequests.set(reqId, entry)
    entry.timer = setTimeout(() => {
      if (state.pendingRequests.has(reqId)) {
        state.pendingRequests.delete(reqId)
        reject(new Error('Tool call timeout (120s)'))
      }
    }, 120000)
  })

  stdioSend(state, {
    jsonrpc: '2.0',
    id: reqId,
    method: 'tools/call',
    params: { name: toolName, arguments: input },
  })

  const result = await promise

  if (result.error) {
    return { content: [{ type: 'text', text: `MCP Error: ${result.error.message}` }] }
  }

  if (result.result?.content && Array.isArray(result.result.content)) {
    return result.result
  }

  return { content: [{ type: 'text', text: typeof result.result === 'string' ? result.result : JSON.stringify(result.result) }] }
}

export function stdioSend(state: McpServerState, msg: Record<string, unknown>): void {
  if (state.process?.stdin?.writable) {
    state.process.stdin.write(JSON.stringify(msg) + '\n')
  }
}

/** Request/response over stdio. Returns the `result` payload or throws on error. */
export async function stdioJsonRpc(
  ctx: TransportContext,
  id: string,
  method: string,
  params: Record<string, unknown>,
): Promise<unknown> {
  const state = ctx.servers.get(id)
  if (!state?.process) throw new Error('stdio server not running')
  const reqId = state.nextRequestId++
  const promise = new Promise<any>((resolve, reject) => {
    const entry = { resolve, reject, timer: undefined as ReturnType<typeof setTimeout> | undefined }
    state.pendingRequests.set(reqId, entry)
    entry.timer = setTimeout(() => {
      if (state.pendingRequests.has(reqId)) {
        state.pendingRequests.delete(reqId)
        reject(new Error(`${method} timeout (120s)`))
      }
    }, 120000)
  })
  stdioSend(state, { jsonrpc: '2.0', id: reqId, method, params })
  const msg = await promise
  if (msg.error) throw new Error(msg.error.message)
  return msg.result
}
