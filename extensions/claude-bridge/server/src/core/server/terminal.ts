// ============================================================================
// Terminal WebSocket — browser-based PTY for Claude Code auth
// ============================================================================

import { WebSocketServer, WebSocket as WS } from 'ws'
import type { Server as HttpServer } from 'http'
import os from 'os'
import { registerWsRoute } from './ws'
import { eventBus } from '../events/bus'
import { injectProxyEnv } from '../config/settings'

const sessions = new Set<{ pty: ReturnType<typeof import('node-pty').spawn>; ws: WS }>()

function broadcastSessionCount(): void {
  eventBus.emit('terminal:sessions', { count: sessions.size })
}

function getShell(): { file: string; args: string[] } {
  if (process.platform === 'win32') {
    return { file: 'cmd.exe', args: [] }
  }
  const shell = process.env.SHELL || '/bin/bash'
  return { file: shell, args: [] }
}

/**
 * Attach a WebSocket server on `/ws/terminal` for interactive PTY sessions.
 * Only one concurrent session is allowed — additional connections are rejected.
 */
export function attachTerminalWebSocket(_server: HttpServer): void {
  const wss = new WebSocketServer({ noServer: true })

  // Register route with central upgrade handler
  registerWsRoute('/ws/terminal', wss)

  wss.on('connection', async (ws: WS) => {
    let pty: ReturnType<typeof import('node-pty').spawn>
    try {
      // Dynamic import — node-pty is a native module (ESM-compatible)
      const nodePty = await import('node-pty')
      const shell = getShell()
      pty = nodePty.spawn(shell.file, shell.args, {
        name: 'xterm-256color',
        cols: 80,
        rows: 24,
        cwd: os.homedir(),
        env: (() => {
          const env = { ...process.env } as Record<string, string>
          // Remove Claude Code session markers so `claude` CLI can run inside PTY
          delete env.CLAUDECODE
          delete env.CLAUDE_CODE_SESSION
          // Inject proxy settings (DB > env vars) for Claude Code only
          injectProxyEnv(env)
          return env
        })(),
      })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      ws.send(JSON.stringify({ type: 'error', data: `Failed to spawn PTY: ${msg}` }))
      ws.close()
      return
    }

    const session = { pty, ws }
    sessions.add(session)
    broadcastSessionCount()

    // PTY → Browser (trailing-edge debounce + max wait)
    // Waits for a gap in output before sending, so full TUI redraws arrive as one chunk.
    // Max wait prevents starvation on continuous output.
    let outputBuffer = ''
    let trailTimer: ReturnType<typeof setTimeout> | null = null
    let maxTimer: ReturnType<typeof setTimeout> | null = null
    function flushOutput() {
      if (trailTimer) { clearTimeout(trailTimer); trailTimer = null }
      if (maxTimer) { clearTimeout(maxTimer); maxTimer = null }
      if (outputBuffer && ws.readyState === WS.OPEN) {
        ws.send(JSON.stringify({ type: 'output', data: outputBuffer }))
        outputBuffer = ''
      }
    }
    pty.onData((data: string) => {
      outputBuffer += data
      // Reset trailing timer on each chunk — wait for gap in output
      if (trailTimer) clearTimeout(trailTimer)
      trailTimer = setTimeout(flushOutput, 8)
      // Max wait: don't hold data longer than 32ms
      if (!maxTimer) {
        maxTimer = setTimeout(flushOutput, 32)
      }
    })

    // PTY exit
    pty.onExit(({ exitCode }: { exitCode: number }) => {
      if (ws.readyState === WS.OPEN) {
        ws.send(JSON.stringify({ type: 'exit', code: exitCode }))
        ws.close()
      }
      sessions.delete(session)
      broadcastSessionCount()
    })

    // Browser → PTY
    ws.on('message', (raw: Buffer | string) => {
      try {
        const msg = JSON.parse(typeof raw === 'string' ? raw : raw.toString())
        if (msg.type === 'input' && typeof msg.data === 'string') {
          pty.write(msg.data)
        } else if (msg.type === 'resize' && typeof msg.cols === 'number' && typeof msg.rows === 'number') {
          pty.resize(Math.max(1, msg.cols), Math.max(1, msg.rows))
        } else if (msg.type === 'refresh') {
          // Force TUI redraw — re-send same size triggers SIGWINCH
          pty.resize(pty.cols, pty.rows)
        }
      } catch {
        // Ignore malformed messages
      }
    })

    function cleanup() {
      if (trailTimer) { clearTimeout(trailTimer); trailTimer = null }
      if (maxTimer) { clearTimeout(maxTimer); maxTimer = null }
      try { pty.kill() } catch {}
      sessions.delete(session)
      broadcastSessionCount()
    }

    // Browser disconnect → kill PTY
    ws.on('close', cleanup)
    ws.on('error', cleanup)
  })
}

/** Get the number of active terminal sessions. */
export function getTerminalSessionCount(): number {
  return sessions.size
}

/** Clean up all active PTY sessions (called on server shutdown). */
export function cleanupTerminal(): void {
  for (const session of sessions) {
    try { session.pty.kill() } catch {}
    try { session.ws.close() } catch {}
  }
  sessions.clear()
}
