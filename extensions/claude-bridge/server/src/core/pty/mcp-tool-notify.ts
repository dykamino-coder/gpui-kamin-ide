// ============================================================================
// MCP tool-list change delivery — `notifications/tools/list_changed`
// ============================================================================
//
// The extension pushes a complete external-tool snapshot over WS
// (`mcp:register-external-tools`) whenever plugins/connectors are installed,
// enabled, disabled or reloaded. Before this module the server only replaced
// `session.registeredTools`; the running Claude CLI kept the catalog from its
// first `tools/list` until a reconnect/respawn (INC-2026-0042).
//
// MCP Streamable HTTP lets a client open a GET SSE stream for server-initiated
// messages. Claude CLI opens it right after `notifications/initialized` is
// accepted, and on `notifications/tools/list_changed` (capability
// `tools.listChanged: true`, CLI ≥ 2.1.0) it re-requests `tools/list` on the
// same connection — no reconnect, no respawn.
//
// Lifecycle contract (see the INC-2026-0042 research boundary):
// - at most ONE stream per session: a new GET replaces (closes) the previous
//   one, so a notification is never broadcast on several streams (spec) and
//   resources stay bounded across client reconnects;
// - streams are keyed by the authenticated session id only — no cross-session
//   delivery;
// - a change while no stream is attached (CLI between reconnects) is kept as a
//   single pending flag and flushed when the next stream opens; tools/list
//   itself always reads the current snapshot, so the flag is all that is lost;
// - keepalive comments detect dead consumers; teardown closes the stream.
// Only the tools catalog is announced: resources/prompts keep their old
// capabilities (no listChanged) because they have no change producer here.
// https://modelcontextprotocol.io/specification/2025-11-25/basic/transports#listening-for-messages-from-the-server
// https://modelcontextprotocol.io/specification/2025-11-25/server/tools#list-changed-notification

import { debugLog } from '../logging'
import type { ToolDefinition } from './types'

const KEEPALIVE_MS = 25_000
const encoder = new TextEncoder()

export const TOOLS_LIST_CHANGED = 'notifications/tools/list_changed'

interface NotificationStream {
  readonly id: number
  send(text: string): boolean
  close(): void
}

const streams = new Map<string, NotificationStream>()
const pending = new Set<string>()
let nextStreamId = 1

function frame(message: unknown): string {
  return `event: message\ndata: ${JSON.stringify(message)}\n\n`
}

function listChangedFrame(): string {
  return frame({ jsonrpc: '2.0', method: TOOLS_LIST_CHANGED })
}

/**
 * Open the session's server→client SSE stream. Replaces any earlier stream of
 * the same session. `isAlive` is polled on every keepalive so a stream never
 * outlives its session even if the teardown hook is missed.
 */
export function openToolNotificationStream(sessionId: string, isAlive: () => boolean): Response {
  streams.get(sessionId)?.close()

  const id = nextStreamId++
  let closed = false
  let timer: ReturnType<typeof setInterval> | null = null
  let ctrl: ReadableStreamDefaultController<Uint8Array> | null = null

  const detach = (): void => {
    if (timer) clearInterval(timer)
    timer = null
    if (streams.get(sessionId)?.id === id) streams.delete(sessionId)
  }
  const handle: NotificationStream = {
    id,
    send(text) {
      if (closed || !ctrl) return false
      try {
        ctrl.enqueue(encoder.encode(text))
        return true
      } catch {
        closed = true
        detach()
        return false
      }
    },
    close() {
      if (closed) return
      closed = true
      detach()
      try {
        ctrl?.close()
      } catch {
        /* consumer already gone */
      }
    },
  }

  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      ctrl = controller
      // First bytes flush headers so the client's fetch resolves immediately.
      handle.send(': open\n\n')
      if (pending.delete(sessionId)) handle.send(listChangedFrame())
      timer = setInterval(() => {
        if (!isAlive()) {
          handle.close()
          return
        }
        handle.send(': keepalive\n\n')
      }, KEEPALIVE_MS)
    },
    cancel() {
      closed = true
      detach()
    },
  })

  streams.set(sessionId, handle)
  debugLog('MCP notification stream opened', { sessionId, streamId: id })

  return new Response(body, {
    status: 200,
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache, no-transform',
      Connection: 'keep-alive',
      'X-Accel-Buffering': 'no',
      'Mcp-Session-Id': sessionId,
    },
  })
}

/** Send `notifications/tools/list_changed` to the session's CLI. Returns
 *  `true` when written to a live stream; otherwise it stays pending for the
 *  next stream of this session. */
export function notifyToolListChanged(sessionId: string): boolean {
  const stream = streams.get(sessionId)
  if (stream && stream.send(listChangedFrame())) {
    pending.delete(sessionId)
    return true
  }
  pending.add(sessionId)
  return false
}

/** Close the stream and forget pending state (session teardown). */
export function closeToolNotificationStream(sessionId: string): void {
  pending.delete(sessionId)
  streams.get(sessionId)?.close()
}

export function hasToolNotificationStream(sessionId: string): boolean {
  return streams.has(sessionId)
}

/** Semantic catalog identity: names, descriptions and schemas, order-free. */
export function toolCatalogFingerprint(tools: readonly ToolDefinition[]): string {
  return JSON.stringify(
    tools
      .map((t) => [t.name, t.description ?? '', t.inputSchema ?? null] as const)
      .sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0)),
  )
}

interface ToolCatalogHolder {
  id: string
  registeredTools: ToolDefinition[]
  mcpInitialized: boolean
}

/**
 * Replace the session's external-tool snapshot and, when the semantic catalog
 * changed for a CLI that already initialized MCP, announce it. Before
 * `initialize` the CLI's first `tools/list` reads the snapshot anyway.
 * Returns whether the catalog changed.
 */
export function replaceRegisteredTools(session: ToolCatalogHolder, tools: ToolDefinition[], reason: string): boolean {
  const changed = toolCatalogFingerprint(session.registeredTools) !== toolCatalogFingerprint(tools)
  session.registeredTools = tools
  if (!changed || !session.mcpInitialized) return changed
  const delivered = notifyToolListChanged(session.id)
  debugLog('MCP tools/list_changed', { sessionId: session.id, reason, count: tools.length, delivered })
  return changed
}
