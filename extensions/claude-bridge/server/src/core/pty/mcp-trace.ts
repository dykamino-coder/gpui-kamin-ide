// Where the time of one forwarded tool call goes. The server stamps when the
// CLI's request arrived, when the mcp:call frame left and when the answer came
// back; the client host reports its own durations (waiting before the tool,
// the tool itself, total) in the mcp:response. Only durations measured on one
// clock are combined, so client and server clocks never need to agree.

/** Durations the client host measured, in milliseconds. */
export interface ClientTiming {
  /** From receiving mcp:call until the tool started (permission, module load). */
  queuedMs: number
  /** The tool itself. */
  toolMs: number
  /** From receiving mcp:call until handing the result to the socket. */
  totalMs: number
}

export interface McpPhases {
  /** CLI request received → mcp:call accepted by the socket (admission, task store). */
  serverBeforeMs: number
  /** Socket round trip outside the client host's own work. */
  transportMs: number
  /** Client host before the tool started. */
  clientQueuedMs: number
  /** The tool on the client host. */
  toolMs: number
  /** Client host after the tool, before the result reached its socket. */
  clientAfterMs: number
  /** mcp:response received → JSON-RPC answer ready (settlement, wake-up). */
  serverAfterMs: number
}

interface Trace {
  startedAt: number
  sentAt?: number
  answeredAt?: number
  client?: ClientTiming
}

/** Bounded: a trace whose call never completes (lost socket) is evicted. */
const MAX_TRACES = 512
const traces = new Map<string, Trace>()

/** Starts tracing the call that will be sent as `requestId`. */
export function traceStart(requestId: string, startedAt: number): void {
  traces.delete(requestId)
  traces.set(requestId, { startedAt })
  if (traces.size > MAX_TRACES) traces.delete(traces.keys().next().value!)
}

/** The mcp:call frame for `requestId` was accepted by the socket. */
export function traceSent(requestId: string, now = Date.now()): void {
  const trace = traces.get(requestId)
  if (trace && trace.sentAt === undefined) trace.sentAt = now
}

function clientTiming(value: unknown): ClientTiming | undefined {
  if (!value || typeof value !== 'object') return undefined
  const { queuedMs, toolMs, totalMs } = value as Record<string, unknown>
  const ok = (n: unknown): n is number => typeof n === 'number' && Number.isFinite(n) && n >= 0
  return ok(queuedMs) && ok(toolMs) && ok(totalMs) ? { queuedMs, toolMs, totalMs } : undefined
}

/** The client host answered `requestId`, optionally with its own timing. */
export function traceAnswered(requestId: string, timing: unknown, now = Date.now()): void {
  const trace = traces.get(requestId)
  if (!trace || trace.answeredAt !== undefined) return
  trace.answeredAt = now
  trace.client = clientTiming(timing)
}

/** Ends the trace and returns its phases, or undefined when a stamp is missing
 *  (an older client host reports no timing). */
export function traceEnd(requestId: string, now = Date.now()): McpPhases | undefined {
  const trace = traces.get(requestId)
  traces.delete(requestId)
  if (!trace || trace.sentAt === undefined || trace.answeredAt === undefined || !trace.client) return undefined
  const roundTrip = trace.answeredAt - trace.sentAt
  const { queuedMs, toolMs, totalMs } = trace.client
  return {
    serverBeforeMs: trace.sentAt - trace.startedAt,
    transportMs: Math.max(0, roundTrip - totalMs),
    clientQueuedMs: queuedMs,
    toolMs,
    clientAfterMs: Math.max(0, totalMs - queuedMs - toolMs),
    serverAfterMs: now - trace.answeredAt,
  }
}

/** Test hook. */
export function resetTracesForTests(): void {
  traces.clear()
}
