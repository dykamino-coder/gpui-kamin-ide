// Request deadlines for the Streamable HTTP transport, which has no pending
// entry to time out: Node's fetch (undici) itself waits 300 s for response
// headers and 300 s between body chunks, so a tool call answered after five
// minutes failed even though Bridge allowed it longer.
//
// fetch accepts a `dispatcher`, but the extension has no undici dependency and
// an Agent from a separate undici copy is not guaranteed to work with Node's
// built-in fetch. Instead this composes an interceptor onto the dispatcher
// Node's fetch already uses (published under undici's global symbol), so any
// proxy dispatcher the host installed keeps working; only the per-request
// `headersTimeout` / `bodyTimeout` change. Without that API fetch keeps its
// defaults, as before.

import { mcpRequestDeadline } from './request-deadline'

const GLOBAL_DISPATCHER = Symbol.for('undici.globalDispatcher.1')

type Dispatch = (options: Record<string, unknown>, handler: unknown) => boolean
interface ComposableDispatcher {
  compose(interceptor: (dispatch: Dispatch) => Dispatch): unknown
}

function globalDispatcher(): ComposableDispatcher | undefined {
  const value = (globalThis as Record<symbol, unknown>)[GLOBAL_DISPATCHER] as Partial<ComposableDispatcher> | undefined
  return typeof value?.compose === 'function' ? value as ComposableDispatcher : undefined
}

export interface FetchDeadline {
  /** Until response headers: a JSON reply arrives only when the tool is done. */
  headersMs: number
  /** Between body chunks: SSE progress events restart it. */
  bodyMs: number
}

/** Deadline of one HTTP JSON-RPC request, from the shared request deadlines. */
export function fetchDeadline(method: string): FetchDeadline {
  const { idleMs, maxMs } = mcpRequestDeadline(method)
  return { headersMs: maxMs, bodyMs: idleMs }
}

const composed = new WeakMap<object, Map<string, unknown>>()

/** `dispatcher` for fetch with these timeouts, or undefined to keep fetch's own. */
export async function fetchDispatcher(deadline: FetchDeadline): Promise<unknown> {
  let base = globalDispatcher()
  // Node installs its dispatcher when fetch first loads; a local data: URL
  // loads it without network I/O.
  if (!base) {
    await fetch('data:,').catch(() => {})
    base = globalDispatcher()
  }
  if (!base) return undefined
  const key = `${deadline.headersMs}:${deadline.bodyMs}`
  let byDeadline = composed.get(base)
  if (!byDeadline) composed.set(base, byDeadline = new Map())
  let dispatcher = byDeadline.get(key)
  if (!dispatcher) {
    dispatcher = base.compose((dispatch) => (options, handler) =>
      dispatch({ ...options, headersTimeout: deadline.headersMs, bodyTimeout: deadline.bodyMs }, handler))
    byDeadline.set(key, dispatcher)
  }
  return dispatcher
}
