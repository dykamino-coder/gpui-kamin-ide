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
// - the outgoing queue is bounded. `list_changed` carries no payload and the
//   CLI always re-reads the CURRENT snapshot, so any number of changes that
//   happen while the reader is behind collapse into one dirty flag, sent when
//   the reader pulls again. A reader that stays full across several keepalive
//   ticks is treated as stalled: the stream is closed with a warning and its
//   unsent change is kept as the pending flag below;
// - a change while no stream is attached (CLI between reconnects) is kept as a
//   single pending flag per session and flushed when the next stream opens;
//   the set of pending sessions is capped, and every session terminal path
//   (exit/finalize, destroy, restart, shutdown) forgets the flag;
// - the stream ends when the request is aborted, when keepalive finds the
//   session gone, or on teardown.
// Only the tools catalog is announced: resources/prompts keep their old
// capabilities (no listChanged) because they have no change producer here.
// https://modelcontextprotocol.io/specification/2025-11-25/basic/transports#listening-for-messages-from-the-server
// https://modelcontextprotocol.io/specification/2025-11-25/server/tools#list-changed-notification

import { debugLog, warnLog } from '../logging'
import type { ToolDefinition } from './types'

export const KEEPALIVE_MS = 25_000
/** Frames the stream may hold for a slow reader (headers flush + one change +
 *  keepalives). Nothing is enqueued beyond it: changes coalesce instead. */
export const STREAM_QUEUE_FRAMES = 4
/** Consecutive keepalive ticks with a full queue before the reader counts as
 *  stalled and the stream is closed. */
export const STALLED_KEEPALIVE_TICKS = 3
/** Upper bound on sessions holding an undelivered flag without a stream. Only
 *  live sessions add to it and every terminal path removes them, so the cap is
 *  a guard against a missed cleanup, not a normal limit. */
export const MAX_PENDING_SESSIONS = 1024
const encoder = new TextEncoder()

export const TOOLS_LIST_CHANGED = 'notifications/tools/list_changed'

/** Outcome of one change announcement. */
export type NotifyResult = 'sent' | 'coalesced' | 'pending' | 'dropped'

interface NotificationStream {
  readonly id: number
  /** Announce a change: send now if the reader has room, else coalesce. */
  notify(): 'sent' | 'coalesced' | 'closed'
  close(): void
}

const streams = new Map<string, NotificationStream>()
const pending = new Set<string>()
let nextStreamId = 1

function frame(message: unknown): string {
  return `event: message\ndata: ${JSON.stringify(message)}\n\n`
}

const LIST_CHANGED_FRAME = encoder.encode(frame({ jsonrpc: '2.0', method: TOOLS_LIST_CHANGED }))
const OPEN_FRAME = encoder.encode(': open\n\n')
const KEEPALIVE_FRAME = encoder.encode(': keepalive\n\n')

function markPending(sessionId: string, reason: string): 'pending' | 'dropped' {
  if (pending.has(sessionId)) return 'pending'
  if (pending.size >= MAX_PENDING_SESSIONS) {
    warnLog('MCP tools/list_changed dropped: pending cap reached', {
      sessionId,
      reason,
      cap: MAX_PENDING_SESSIONS,
    })
    return 'dropped'
  }
  pending.add(sessionId)
  return 'pending'
}

/**
 * Open the session's server→client SSE stream. Replaces any earlier stream of
 * the same session. `isAlive` is checked on every keepalive so a stream never
 * outlives its session even if the teardown hook is missed; `signal` (the
 * request's abort signal) ends it as soon as the client goes away.
 */
export function openToolNotificationStream(sessionId: string, isAlive: () => boolean, signal?: AbortSignal): Response {
  streams.get(sessionId)?.close()

  const id = nextStreamId++
  let closed = false
  // A change the reader has not received yet (queue was full). At most one.
  let dirty = false
  let stalledTicks = 0
  let timer: ReturnType<typeof setInterval> | null = null
  let ctrl: ReadableStreamDefaultController<Uint8Array> | null = null

  const onAbort = (): void => finish('request-aborted')

  /** Enqueue only when the reader has room; never grows the queue past its
   *  high-water mark. */
  const offer = (chunk: Uint8Array): boolean => {
    if (closed || !ctrl) return false
    const room = ctrl.desiredSize
    if (room === null || room <= 0) return false
    try {
      ctrl.enqueue(chunk)
      return true
    } catch {
      finish('enqueue-failed')
      return false
    }
  }
  /** Leave the registry; an unsent change survives as the pending flag. */
  const detach = (): void => {
    if (timer) clearInterval(timer)
    timer = null
    signal?.removeEventListener('abort', onAbort)
    if (streams.get(sessionId)?.id === id) streams.delete(sessionId)
    if (dirty) {
      dirty = false
      markPending(sessionId, 'stream-closed')
    }
  }
  function finish(reason: string): void {
    if (closed) return
    closed = true
    detach()
    try {
      ctrl?.close()
    } catch {
      /* consumer already gone */
    }
    debugLog('MCP notification stream closed', { sessionId, streamId: id, reason })
  }

  const handle: NotificationStream = {
    id,
    notify() {
      if (closed) return 'closed'
      if (offer(LIST_CHANGED_FRAME)) {
        dirty = false
        return 'sent'
      }
      dirty = true
      return 'coalesced'
    },
    close: () => finish('replaced-or-teardown'),
  }

  const body = new ReadableStream<Uint8Array>(
    {
      start(controller) {
        ctrl = controller
        // First bytes flush headers so the client's fetch resolves immediately.
        offer(OPEN_FRAME)
        if (pending.delete(sessionId)) dirty = true
        if (dirty && offer(LIST_CHANGED_FRAME)) dirty = false
        timer = setInterval(() => {
          if (!isAlive()) {
            finish('session-gone')
            return
          }
          if (offer(KEEPALIVE_FRAME)) {
            stalledTicks = 0
            return
          }
          stalledTicks += 1
          if (stalledTicks >= STALLED_KEEPALIVE_TICKS) {
            warnLog('MCP notification stream stalled — closing', { sessionId, streamId: id, ticks: stalledTicks })
            finish('stalled-reader')
          }
        }, KEEPALIVE_MS)
      },
      pull() {
        // The reader drained the queue: deliver the coalesced change, once.
        stalledTicks = 0
        if (dirty && offer(LIST_CHANGED_FRAME)) dirty = false
      },
      cancel() {
        if (closed) return
        closed = true
        detach()
        debugLog('MCP notification stream closed', { sessionId, streamId: id, reason: 'reader-cancelled' })
      },
    },
    new CountQueuingStrategy({ highWaterMark: STREAM_QUEUE_FRAMES }),
  )

  if (signal) {
    if (signal.aborted) queueMicrotask(onAbort)
    else signal.addEventListener('abort', onAbort, { once: true })
  }
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

/** Announce `notifications/tools/list_changed` to the session's CLI:
 *  `sent` — written to the live stream; `coalesced` — the reader is behind and
 *  already owes one notification; `pending` — no stream, flushed when the next
 *  one opens; `dropped` — no stream and the pending cap is reached (logged). */
export function notifyToolListChanged(sessionId: string): NotifyResult {
  const result = streams.get(sessionId)?.notify()
  if (result === 'sent' || result === 'coalesced') {
    pending.delete(sessionId)
    return result
  }
  return markPending(sessionId, 'no-stream')
}

/** Close the stream and forget pending state. Every session terminal path
 *  calls it: destroy, exit/finalize, restart and shutdown. */
export function closeToolNotificationStream(sessionId: string): void {
  streams.get(sessionId)?.close()
  pending.delete(sessionId)
}

export function hasToolNotificationStream(sessionId: string): boolean {
  return streams.has(sessionId)
}

export function hasPendingToolNotification(sessionId: string): boolean {
  return pending.has(sessionId)
}

/** Diagnostics/tests: number of sessions holding an undelivered flag. */
export function pendingToolNotificationCount(): number {
  return pending.size
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
  const delivery = notifyToolListChanged(session.id)
  debugLog('MCP tools/list_changed', { sessionId: session.id, reason, count: tools.length, delivery })
  return changed
}
