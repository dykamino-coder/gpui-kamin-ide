import type { KaminBridgeApi } from '../../shared/types'

// The sandboxed webview can't hit http://localhost:3456 directly (CSP
// `connect-src 'self' https:`). All Bridge-server dashboard calls (token list,
// token create, streaming-settings) route through the ext-host (Node, no CSP)
// via the `bridge:server-fetch` invoke channel — see extension/src/core-ipc.ts.

export interface ServerFetchInit {
  method?: string
  body?: string
  headers?: Record<string, string>
}

export interface ServerFetchResult {
  ok: boolean
  status: number
  data: unknown
  error?: string
}

interface BridgeWithFetch {
  serverFetch: (httpBase: string, path: string, init?: ServerFetchInit) => Promise<ServerFetchResult>
}

/** Node-level failures surface as undici's bare "fetch failed" / "ECONNREFUSED",
 *  which panels then print verbatim — it reads like a bug in the panel rather
 *  than "the Bridge server isn't running". Rewrite transport errors (status 0)
 *  into a sentence that names the address and the likely cause. */
function explain(httpBase: string, error?: string): string {
  const raw = (error ?? '').trim()
  const transport = !raw
    || /fetch failed|ECONNREFUSED|ENOTFOUND|EHOSTUNREACH|ETIMEDOUT|network|aborted|timeout/i.test(raw)
  if (!transport) return raw
  return `Can't reach the Bridge server at ${httpBase} — check that it is running.`
}

/** Proxy an HTTP request to the Bridge server through the ext-host. `path` is
 *  appended to the server's http base (e.g. "/api/dashboard/tokens"). */
export async function serverFetch(
  bridge: KaminBridgeApi,
  httpBase: string,
  path: string,
  init?: ServerFetchInit,
): Promise<ServerFetchResult> {
  const res = await (bridge as unknown as BridgeWithFetch).serverFetch(httpBase, path, init)
  // status 0 = the request never reached the server (socket/DNS/timeout),
  // as opposed to a real 4xx/5xx answer.
  if (!res.ok && res.status === 0) return { ...res, error: explain(httpBase, res.error) }
  return res
}
