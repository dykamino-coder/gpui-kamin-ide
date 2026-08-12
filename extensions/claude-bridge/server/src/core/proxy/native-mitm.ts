// ============================================================================
// Native MITM proxy — request-level via https.createServer + undici.fetch.
//
// The previous implementation byte-piped CLI's TLS over a hand-rolled
// `tls.connect({socket})` upstream, then parsed HTTP/1.1 manually to find
// SSE bodies. That path produced a TLS fingerprint distinct from undici
// (which Claude Code itself uses) and corp/CF occasionally RST'd it. It also
// hit known Node 22 ALPN/cipher negotiation bugs on `tls.connect` over a
// manually-passed socket.
//
// This shape is the same one Claude Code uses (src/utils/proxy.ts):
//   1. Outer `net.createServer` accepts CLI's CONNECT, ack 200, hand socket on.
//   2. Inner `https.createServer` terminates CLI's TLS with a per-host leaf
//      cert minted from our trusted bridge CA (CLI trusts it via
//      NODE_EXTRA_CA_CERTS) and parses HTTP/1.1 requests for us.
//   3. Each request is re-issued via `undici.fetch` with an
//      EnvHttpProxyAgent / ProxyAgent dispatcher that handles ALPN h2
//      negotiation, corp-proxy CONNECT tunneling, mTLS, and CA bundles.
//   4. For SSE responses (`text/event-stream`) the body is tee'd through a
//      PassThrough that feeds SseStreamParser → EntrySynthesizer; other
//      responses just pipe straight back to CLI.
//
// Stale-pool ECONNRESET handling: we cache one dispatcher per
// (proxyUrl, caBundle) and drop+rebuild it on ECONNRESET so the next request
// opens a fresh TCP socket — mirroring Claude Code's `disableKeepAlive()`.
// ============================================================================

import { Buffer } from "node:buffer"
import * as fs from "node:fs/promises"
import * as http from "node:http"
import * as https from "node:https"
import * as net from "node:net"
import { PassThrough, Readable } from "node:stream"
import * as tls from "node:tls"
import { EnvHttpProxyAgent, ProxyAgent, fetch as undiciFetch } from "undici"
import type { Dispatcher, RequestInit as UndiciRequestInit } from "undici"
import { getOrCreateBridgeCA } from "./ca-store"
import { EntrySynthesizer, type SynthAssistantEntry } from "./entry-synthesizer"
import { mintLeafCert } from "./leaf-cert"
import { SseStreamParser } from "./sse-parser"
import { warnLog } from "../logging"

const ANTHROPIC_HOST = "api.anthropic.com"
const ANTHROPIC_MESSAGES_PATH = "/v1/messages"

/** undici throws a bare `TypeError: fetch failed` and hides the REAL reason —
 *  ECONNRESET, a corp TLS-interception cert error, ETIMEDOUT, ENOTFOUND, "socket
 *  hang up" — in `err.cause` (sometimes nested). Unwrap the whole chain so the
 *  502 body and the log name the actual failure instead of the opaque wrapper.
 *  This is what separates "Anthropic is overloaded" (an HTTP 5xx we forward with
 *  a status) from "this machine can't reach api.anthropic.com" (a connection
 *  failure that surfaces here). */
function describeFetchError(err: unknown): string {
  const parts: string[] = []
  let cur: unknown = err
  const seen = new Set<unknown>()
  while (cur && !seen.has(cur) && parts.length < 6) {
    seen.add(cur)
    if (cur instanceof Error) {
      const code = (cur as { code?: string }).code
      parts.push(code ? `${cur.message} (${code})` : cur.message)
      cur = (cur as { cause?: unknown }).cause
    } else {
      parts.push(String(cur))
      break
    }
  }
  return parts.join(" ← ")
}

export interface NativeMitmHandle {
  port: number
  caCertPath: string
  stop: () => Promise<void>
  /** Abort every in-flight upstream request NOW (user pressed Stop). Ctrl+C to
   *  the CLI only stops the CLI's NEXT turn; the buffered SSE it already asked for
   *  keeps flushing until the CLI itself drops the socket. Aborting the upstream
   *  fetch here cancels that response immediately. */
  interrupt: () => void
}

export interface NativeMitmOptions {
  ptySessionId: string
  cliConversationId?: string
  upstreamProxyUrl?: string
  /** Session-level model the user picked (for side-quest detection by
   *  model-family comparison). */
  mainModel?: string
  onEntryUpdate: (entry: SynthAssistantEntry, isSideQuest: boolean) => void
  /** Rate-limit / overload surfaced to the client so the chat can show
   *  "retrying in Ns" instead of stalling silently during CLI backoff. */
  onStatus?: (info: { code: number; retryAfterMs?: number }) => void
  /** New streaming protocol (client protocolVersion>=1): full entry ONLY at
   *  structural boundaries (message/block start-stop, finalize). */
  onSnapshot?: (entry: SynthAssistantEntry) => void
  /** New streaming protocol: incremental text/thinking append between the
   *  boundary snapshots. */
  onDelta?: (d: { msgId: string; blockIdx: number; appendText: string; kind: 'text' | 'thinking' }) => void
}

// ── extra CA cache (bump.crt etc.) ───────────────────────────────────────────
let cachedExtraCA: Buffer[] | null = null
// Serialize concurrent first-loads: without this, two simultaneous getDispatcher
// calls both miss the cache, both fs.readFile, and the second (possibly a failed
// read → empty array) overwrites the first, poisoning the shared cache.
let extraCALoad: Promise<Buffer[]> | null = null

async function loadExtraCA(): Promise<Buffer[]> {
  if (cachedExtraCA) return cachedExtraCA
  if (extraCALoad) return extraCALoad
  extraCALoad = (async () => {
    const out: Buffer[] = []
    try {
      const { getSetting } = await import("../config/settings")
      const caPath = getSetting("caCert")
      if (caPath) out.push(await fs.readFile(caPath))
    } catch (e) {
      warnLog("[mitm] extra CA load failed", { error: String(e) })
    }
    cachedExtraCA = out
    return out
  })().finally(() => { extraCALoad = null })
  return extraCALoad
}

// ── undici dispatcher cache + stale-pool eviction ────────────────────────────
// One dispatcher per (proxyUrl, caBundle) so connection-pool reuse works
// between consecutive requests. ECONNRESET on a request → recycle the
// dispatcher so the next call opens a fresh TCP socket.
let cachedDispatcher: { key: string; dispatcher: Dispatcher } | null = null

// Cache the TLS SecureContext per host so we don't rebuild an OpenSSL SSL_CTX on
// EVERY handshake. Every session's CLI hits the same host (api.anthropic.com)
// with the same leaf cert, so one context is reused across all sessions +
// keep-alive reconnects. Keyed by the leaf's certPem so it auto-invalidates when
// the leaf rotates in the mint LRU.
const secureContextByHost = new Map<string, { certPem: string; ctx: tls.SecureContext }>()

async function getDispatcher(proxyUrl: string | undefined): Promise<Dispatcher> {
  const caBundle = await loadExtraCA()
  const caKey = caBundle.length > 0 ? caBundle.map(b => b.length).join(",") : "0"
  const key = (proxyUrl || "direct") + "|" + caKey
  if (cachedDispatcher && cachedDispatcher.key === key) return cachedDispatcher.dispatcher

  if (cachedDispatcher) {
    try { await cachedDispatcher.dispatcher.close() } catch { /* ignore */ }
  }

  const tlsOpts = caBundle.length > 0 ? { ca: caBundle } : undefined
  // Reverted to the pre-6.2.53 dispatcher options: 6.2.53 added TCP-level
  // keepalive (`connect.keepAlive` / `proxyTls.keepAlive` /
  // `requestTls.keepAlive` with 10s initial delay) hoping to stop corp
  // proxies dropping idle SSE streams. On at least one prod corp
  // network this broke API access entirely (TLS handshake failures),
  // which is a far worse regression than the original `terminated`
  // mid-stream issue that swallowing benign exceptions already
  // mitigates. Mirrors what Claude Code itself does (utils/proxy.ts).
  const dispatcher: Dispatcher = proxyUrl
    ? new ProxyAgent({
        uri: proxyUrl,
        ...(tlsOpts && { connect: tlsOpts, requestTls: tlsOpts }),
      })
    : new EnvHttpProxyAgent({
        ...(tlsOpts && { connect: tlsOpts, requestTls: tlsOpts }),
      })
  cachedDispatcher = { key, dispatcher }
  return dispatcher
}

/** Clear the cached dispatcher and extra-CA bundle. Called from settings.ts
 *  when the user uploads a new CA cert through the Settings UI. */
export function resetExtraTrustRoots(): void {
  cachedExtraCA = null
  extraCALoad = null
  if (cachedDispatcher) {
    void cachedDispatcher.dispatcher.close().catch(() => { /* ignore */ })
    cachedDispatcher = null
  }
}

function recycleDispatcherOnReset(err: unknown): void {
  const code = (err as { code?: string })?.code
    ?? ((err as { cause?: { code?: string } })?.cause)?.code
  if (code === "ECONNRESET" || code === "UND_ERR_SOCKET" || code === "EPIPE") {
    dropCachedDispatcher()
  }
}

/** Drop the shared pooled dispatcher so the NEXT request opens a fresh socket.
 *  recycleDispatcherOnReset only fires on reset CODES; a user Ctrl+C aborts the
 *  in-flight SSE fetch as an AbortError (not a reset code), leaving the pooled
 *  keep-alive socket torn. That wedged socket starves the tee→synth observation
 *  on the next turn — the CLI's own request still lands a response (console +
 *  JSONL keep working), but live word-by-word streaming stays dead until a
 *  manual reconnect. Forcing a fresh socket here makes streaming self-heal too.
 *  Only called on a genuine mid-stream interrupt (see the res-close guard), so
 *  clean turn-ends never pay the re-handshake. */
function dropCachedDispatcher(): void {
  if (cachedDispatcher) {
    void cachedDispatcher.dispatcher.close().catch(() => { /* ignore */ })
    cachedDispatcher = null
  }
}

// ── public entry point ───────────────────────────────────────────────────────
export async function startNativeMitm(opts: NativeMitmOptions): Promise<NativeMitmHandle> {
  const ca = await getOrCreateBridgeCA()
  // Warm the dispatcher up-front so the first CLI request doesn't pay for it.
  await getDispatcher(opts.upstreamProxyUrl).catch(() => { /* deferred */ })

  // Every in-flight upstream request's AbortController, so interrupt() can cancel
  // the streaming response the moment the user hits Stop. Each handleRequest adds
  // its controller and removes it when its `res` closes.
  const active = new Set<AbortController>()

  // Inner HTTPS server — terminates CLI's TLS and parses HTTP/1.1 for us.
  // ALPN advertised as h1.1 only; CLI's undici falls back gracefully on the
  // inner hop. The outer hop (us → anthropic via undici) negotiates h2 itself.
  const httpsServer = https.createServer({
    SNICallback: (sni, cb) => {
      const targetHost = sni || ANTHROPIC_HOST
      // Guard against a hung mint (e.g. entropy starvation on a fresh container):
      // a never-resolving SNICallback stalls the TLS handshake forever and pins
      // the socket's fd. Fail the handshake after 8s so the CLI can retry.
      let settled = false
      const timer = setTimeout(() => {
        if (settled) return
        settled = true
        cb(new Error("cert minting timed out"))
      }, 8_000)
      mintLeafCert(targetHost, ca).then((leaf) => {
        if (settled) return
        settled = true
        clearTimeout(timer)
        // Reuse the SecureContext for this host+leaf across handshakes/sessions;
        // only rebuild the OpenSSL context if the leaf rotated.
        let entry = secureContextByHost.get(targetHost)
        if (!entry || entry.certPem !== leaf.certPem) {
          entry = { certPem: leaf.certPem, ctx: tls.createSecureContext({ cert: leaf.certPem, key: leaf.keyPem }) }
          secureContextByHost.set(targetHost, entry)
        }
        cb(null, entry.ctx)
      }).catch((e) => {
        if (settled) return
        settled = true
        clearTimeout(timer)
        warnLog("[mitm] cert minting failed", {
          sessionId: opts.ptySessionId, host: targetHost, error: String(e),
        })
        cb(e as Error)
      })
    },
    ALPNProtocols: ["http/1.1"],
  })

  httpsServer.on("request", (req, res) => {
    void handleRequest(req, res, opts, active).catch((err) => {
      // Name the REAL cause (err.cause chain), not undici's opaque "fetch failed".
      const detail = describeFetchError(err)
      warnLog("[mitm] request handler crashed", {
        sessionId: opts.ptySessionId,
        error: detail,
      })
      try {
        if (!res.headersSent) {
          res.statusCode = 502
          res.setHeader("content-type", "text/plain")
          res.end("Bridge upstream error: " + detail)
        } else {
          res.destroy(err as Error)
        }
      } catch { /* socket already gone */ }
    })
  })

  httpsServer.on("tlsClientError", (err: Error) => {
    // Common when CLI snaps a tab; not interesting unless a real handshake fails.
    const code = (err as { code?: string }).code
    if (code !== "ECONNRESET" && code !== "EPIPE") {
      warnLog("[mitm] tlsClientError", {
        sessionId: opts.ptySessionId, code, msg: err.message,
      })
    }
  })

  // Outer raw-TCP server — handles CLI's CONNECT request, then hands off.
  const CONNECT_MAX_HEADER = 16 * 1024 // cap the pre-TLS CONNECT buffer (DoS guard)
  const CONNECT_TIMEOUT_MS = 15_000 // a client that never completes CONNECT frees its fd
  const server = net.createServer((clientSock) => {
    let buffer = Buffer.alloc(0)
    // Idle-timeout ONLY the CONNECT phase; cleared before handoff so a live
    // multi-minute SSE tunnel is never killed by it. Without this a client that
    // opens a socket and never sends \r\n\r\n pins a file descriptor forever.
    clientSock.setTimeout(CONNECT_TIMEOUT_MS)
    clientSock.once("timeout", () => { try { clientSock.destroy() } catch { /* gone */ } })
    const onData = (chunk: Buffer) => {
      buffer = Buffer.concat([buffer, chunk])
      // Bound the buffer: a client dribbling headers without a terminator would
      // otherwise grow the heap unbounded (indexOf rescans O(n) each chunk too).
      if (buffer.length > CONNECT_MAX_HEADER) {
        clientSock.removeListener("data", onData)
        try { clientSock.write("HTTP/1.1 431 Request Header Fields Too Large\r\nContent-Length: 0\r\n\r\n") } catch { /* gone */ }
        clientSock.end()
        return
      }
      const idx = buffer.indexOf("\r\n\r\n")
      if (idx < 0) return
      const headerStr = buffer.slice(0, idx).toString("utf8")
      const rest = buffer.slice(idx + 4)
      clientSock.removeListener("data", onData)
      const firstLine = headerStr.split("\r\n", 1)[0] ?? ""
      const m = /^CONNECT\s+(\S+)\s+HTTP\/1\.[01]$/i.exec(firstLine)
      if (!m) {
        clientSock.write("HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n")
        clientSock.end()
        return
      }
      // Handshake accepted — cancel the CONNECT-phase idle timeout so the live
      // TLS tunnel (which can carry a >90s thinking stream) isn't reaped.
      clientSock.setTimeout(0)
      clientSock.write("HTTP/1.1 200 Connection established\r\n\r\n")
      if (rest.length > 0) clientSock.unshift(rest)
      httpsServer.emit("connection", clientSock)
    }
    clientSock.on("data", onData)
    clientSock.on("error", () => { /* client drops happen, no log */ })
  })

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject)
    server.listen(0, "127.0.0.1", () => {
      server.removeListener("error", reject)
      resolve()
    })
  })

  const port = (server.address() as net.AddressInfo).port

  return {
    port,
    caCertPath: ca.certPath,
    interrupt: () => {
      for (const c of active) { try { c.abort() } catch { /* already settled */ } }
    },
    stop: async () => {
      await new Promise<void>((r) => server.close(() => r()))
      await new Promise<void>((r) => httpsServer.close(() => r()))
      // NB: do NOT close the module-shared `cachedDispatcher` here. It's a
      // process-wide undici connection pool used by EVERY live session's proxy;
      // closing it when one tab stops drained every other session's in-flight
      // pool and forced a TLS/H2 re-handshake on their next request. It already
      // self-recycles on ECONNRESET (recycleDispatcherOnReset) and on CA change
      // (resetExtraTrustRoots), and idle keep-alive sockets time out on their
      // own — so leaving it open is correct, not a leak.
    },
  }
}

// ── per-request handler ─────────────────────────────────────────────────────
async function handleRequest(
  req: http.IncomingMessage,
  res: http.ServerResponse,
  opts: NativeMitmOptions,
  active: Set<AbortController>,
): Promise<void> {
  const host = (req.headers.host || ANTHROPIC_HOST).split(":")[0]!
  const targetUrl = `https://${host}${req.url ?? "/"}`

  // Strip per-hop headers that mustn't be forwarded.
  const upstreamHeaders: Record<string, string> = {}
  for (const [k, v] of Object.entries(req.headers)) {
    if (v === undefined) continue
    const lk = k.toLowerCase()
    if (lk === "host" || lk === "connection" || lk === "keep-alive"
        || lk === "proxy-authorization" || lk === "proxy-connection"
        || lk === "transfer-encoding" || lk === "upgrade"
        || lk === "content-length") continue
    upstreamHeaders[k] = Array.isArray(v) ? v.join(", ") : v
  }
  // Anthropic requires the right Host header.
  upstreamHeaders["host"] = host

  // Body: undici.fetch accepts a web ReadableStream produced from the Node
  // request.
  const hasBody = req.method !== "GET" && req.method !== "HEAD"

  // ── Gated request diagnostic (KAMIN_MITM_REQDIAG=1) ──────────────────────
  // Off by default. When on, buffer a /v1/messages POST body ONCE so we can log
  // the shape that distinguishes a SUBAGENT request from the main agent's — the
  // discriminator needed to tag streaming sidechain turns at the source (the
  // prompt text alone is ambiguous: it appears both as a teammate's first user
  // message AND inside the main agent's tool_use history). Buffered (not
  // streamed) only under the flag; the buffer is then forwarded verbatim.
  const isMessagesPostReq = (req.url ?? "").startsWith(ANTHROPIC_MESSAGES_PATH) && req.method === "POST"
  let bufferedBody: Buffer | undefined
  if (hasBody && isMessagesPostReq && process.env.KAMIN_MITM_REQDIAG) {
    const chunks: Buffer[] = []
    for await (const c of req) chunks.push(c as Buffer)
    bufferedBody = Buffer.concat(chunks)
    try {
      const j = JSON.parse(bufferedBody.toString("utf8")) as Record<string, any>
      const sys = Array.isArray(j.system)
        ? j.system.map((s: any) => (typeof s === "string" ? s : String(s?.text ?? "")).slice(0, 90))
        : typeof j.system === "string" ? [j.system.slice(0, 120)] : undefined
      const msgs = Array.isArray(j.messages) ? j.messages : []
      const m0 = msgs[0]
      const m0Head = m0
        ? (typeof m0.content === "string" ? m0.content.slice(0, 160) : JSON.stringify(m0.content).slice(0, 240))
        : ""
      warnLog("[mitm][reqdiag]", {
        session: opts.ptySessionId,
        model: j.model,
        msgCount: msgs.length,
        xAgentId: req.headers["x-claude-code-agent-id"],
        parentSessionId: (() => { try { return JSON.parse(String(j.metadata?.user_id ?? "{}")).parent_session_id } catch { return undefined } })(),
        m0Role: m0?.role,
        m0Head: m0Head.slice(0, 60),
        userAgent: req.headers["user-agent"],
      })
    } catch {
      warnLog("[mitm][reqdiag] body parse failed", { session: opts.ptySessionId, len: bufferedBody.length })
    }
  }

  const body = hasBody ? (bufferedBody ?? Readable.toWeb(req)) : undefined

  const dispatcher = await getDispatcher(opts.upstreamProxyUrl)

  // Abort the upstream request when the CLI hangs up. Destroying the Node
  // Readable wrapper alone does NOT reliably cancel undici's underlying socket
  // (it can stay pinned in the pool on a silent mid-stream stall); an explicit
  // abort() releases it. Wired to res 'close' below.
  const ac = new AbortController()
  // Track for interrupt() (user Stop) and release on close — res always emits
  // 'close' on completion or teardown, so this covers every return path. Abort
  // HERE too, not only in the branch-specific close handlers below: those are
  // registered AFTER `await undiciFetch`, so a client that disconnects while the
  // fetch is still in flight would otherwise slip through — this listener would
  // remove `ac` from `active` (unreachable by interrupt()) while the real abort
  // handler never gets registered, leaking the upstream request. Aborting on any
  // close is idempotent: on normal completion the fetch is already done.
  active.add(ac)
  res.on("close", () => { active.delete(ac); try { ac.abort() } catch { /* already settled */ } })

  let upstream: Awaited<ReturnType<typeof undiciFetch>>
  try {
    const init: UndiciRequestInit = {
      method: req.method,
      headers: upstreamHeaders,
      body: body as UndiciRequestInit["body"],
      dispatcher,
      duplex: "half",
      signal: ac.signal,
    }
    upstream = await undiciFetch(targetUrl, init)
  } catch (err) {
    recycleDispatcherOnReset(err)
    throw err
  }

  // Status + headers. Strip:
  //   - content-encoding: undici already gunzipped/inflated/brotli-decoded the
  //     body; forwarding the original header would make CLI try to decompress
  //     plain text → ZlibError.
  //   - content-length: post-decompression length differs from the upstream
  //     header. Letting Node's http server compute chunked encoding instead.
  //   - transfer-encoding / connection: hop-by-hop, must not propagate.
  res.statusCode = upstream.status
  for (const [k, v] of upstream.headers.entries()) {
    const lk = k.toLowerCase()
    if (lk === "content-encoding" || lk === "content-length"
        || lk === "transfer-encoding" || lk === "connection"
        || lk === "keep-alive") continue
    res.setHeader(k, v)
  }

  // Surface rate-limit (429) / overload (5xx) to the client. The CLI retries
  // these itself, so without a signal the chat just stalls silently — the only
  // hint is the raw terminal. `retry-after` is seconds per HTTP spec.
  if (opts.onStatus && (upstream.status === 429 || upstream.status >= 500)) {
    const ra = upstream.headers.get("retry-after")
    const raNum = ra != null ? Number(ra) : NaN
    opts.onStatus({ code: upstream.status, ...(Number.isFinite(raNum) ? { retryAfterMs: raNum * 1000 } : {}) })
  }

  if (!upstream.body) {
    res.end()
    return
  }

  const contentType = upstream.headers.get("content-type") || ""
  const isSse = contentType.includes("text/event-stream")
  const isMessages = (req.url ?? "").startsWith(ANTHROPIC_MESSAGES_PATH) && req.method === "POST"

  // Plain (non-SSE) response — straight pipe.
  if (!isSse || !isMessages) {
    // DIAG (#73): a /v1/messages POST that comes back NON-SSE means the CLI
    // still gets its full response (→ JSONL → chat) but nothing is tee'd →
    // no word-by-word streaming this turn. This is the ONE architectural path
    // where "console+JSONL work, streaming doesn't" — matches the intermittent
    // "первый заход без стрима, второй раз ок" report.
    //
    // Filter the noise: `/v1/messages/count_tokens` is legitimately non-SSE,
    // and the CLI also fires non-streaming /v1/messages calls (title/quota).
    // A real chat completion asks for the stream via `Accept: text/event-stream`
    // — so `wantedSse && !isSse` on a messages POST is the true #73 failure.
    if (isMessages && !isSse && !(req.url ?? "").includes("/count_tokens")) {
      const accept = String(req.headers["accept"] ?? "")
      warnLog("[mitm][diag#73] non-SSE messages response", {
        status: upstream.status, contentType, url: req.url,
        wantedSse: accept.includes("text/event-stream"), accept: accept.slice(0, 48),
        session: opts.ptySessionId,
      })
    }
    const src = Readable.fromWeb(upstream.body)
    // pipe() auto-destroys res on source error, but an unhandled 'error' on
    // `src` would still throw; handle it + recycle the pool on a reset.
    src.on("error", (err: Error) => {
      recycleDispatcherOnReset(err)
      if (!res.writableEnded && !res.destroyed) try { res.destroy(err) } catch { /* gone */ }
    })
    // A write to a dead CLI socket emits 'error' on res; without this listener it
    // becomes an uncaughtException (the SSE branch already guards this at :537).
    res.on("error", (err: Error) => { warnLog("[mitm] res stream error (non-sse)", { error: err.message }) })
    res.on("close", () => { try { ac.abort() } catch { /* ignore */ } if (!src.destroyed) try { src.destroy() } catch { /* ignore */ } })
    src.pipe(res)
    return
  }

  // SSE — observe the bytes for entry synthesis while forwarding to CLI.
  // Two things to get right (verified against claude-code-src behaviour):
  //
  //  1. **Decouple upstream pace from CLI's write rate.** Chained
  //     `pipe(upstream → tee → res)` propagates backpressure: if CLI's
  //     local TLS write is slow (client webview busy, PTY buffer full),
  //     `res` slows tee, tee slows upstream, undici reads slower, and the
  //     corp proxy sees a "stalled" connection and RSTs it. We instead
  //     pull from upstream at full speed and fan-out to tee + res with
  //     `res.write()` whose return value we deliberately ignore. SSE
  //     responses are tens-to-hundreds of KB; node's write queue is fine.
  //
  //  2. **`:keepalive` SSE comments pass through verbatim.** Tee only
  //     OBSERVES bytes for synthesis (reading via `tee.on('data')`); the
  //     forwarding to CLI is a separate `res.write()` of the same raw
  //     buffer. Anthropic's heartbeat comments reach CLI unmodified, so
  //     CLI's own activity-tracking won't time the connection out.
  //
  // NOTE: previously a 90s idle watchdog destroyed upstream on silence.
  // That backfired on long thinking/extended-context responses (opus-4-8
  // with 600k cache_read can think > 90s) — the abort left CLI with a
  // partial assistant entry (stop_reason: null) in JSONL, then CLI wrote
  // <synthetic> UUID placeholders and used those as previous_message_id
  // on the next turn, getting permanent 400s from Anthropic. We now rely
  // purely on TCP/TLS errors and Anthropic's own :keepalive heartbeat.
  // Subagent attribution — CHEAP + RELIABLE via one request header. The CLI
  // stamps `x-claude-code-agent-id` (`<name>-<n>@<team>`) on every teammate/
  // subagent request and NOTHING on the main agent's. A teammate runs on a
  // non-Haiku model, so the response-side side-quest heuristics can't tell its
  // stream from a main turn — which is why a teammate's live tokens used to
  // stream into the MAIN chat and then vanish when the canonical (isSidechain)
  // JSONL twin replaced them. Marking the synth sidechain here fixes it AT THE
  // SOURCE: the stub is tagged from its first byte, so the client hides it from
  // the main chat (and can route it to that agent). No body parsing.
  const hdr = req.headers["x-claude-code-agent-id"]
  let subagentId = Array.isArray(hdr) ? hdr[0] : hdr
  // Фолбэк-дискриминатор субагента: заголовок x-claude-code-agent-id несёт
  // НЕ каждый Task-tool субагент (только часть). Тогда его ход шёл в основной
  // чат как main (isSidechain=false), т.к. response-эвристика ловит лишь Haiku.
  // metadata.user_id.parent_session_id есть у КАЖДОГО субагентского запроса
  // (его нет у главного) — используем как id, если заголовка нет.
  if (!subagentId && bufferedBody) {
    try {
      const j = JSON.parse(bufferedBody.toString("utf8")) as { metadata?: { user_id?: unknown } }
      const meta = typeof j.metadata?.user_id === "string" ? JSON.parse(j.metadata.user_id) : j.metadata?.user_id
      // Наличие parent_session_id = это субагент (у главного его нет). Значение
      // указывает на главного, поэтому id субагента строим от НЕГО + модели
      // запроса недостаточно — берём сам psid как ключ ветки (все ходы одного
      // субагента делят parent, но панель группирует по нему же, что ок).
      const psid = (meta as { parent_session_id?: string } | undefined)?.parent_session_id
      if (psid) subagentId = `psid:${psid}`
    } catch { /* не JSON — не субагент */ }
  }
  const synth = new EntrySynthesizer({
    ptySessionId: opts.ptySessionId,
    cliConversationId: opts.cliConversationId,
    mainModel: opts.mainModel,
    ...(subagentId ? { markSidechain: true, subagentId } : {}),
    callbacks: {
      onUpdate: (entry) => opts.onEntryUpdate(entry, !!entry.isSidechain),
      ...(opts.onSnapshot ? { onSnapshot: opts.onSnapshot } : {}),
      ...(opts.onDelta ? { onDelta: opts.onDelta } : {}),
    },
  })
  const parser = new SseStreamParser()
  const tee = new PassThrough()
  // DIAG (#73): per-turn tee accounting — if a turn is SSE but ends with 0
  // events fed, the parser/synth is the culprit; if bytes+events flowed here
  // yet the chat showed nothing, the break is downstream (delivery/client).
  let teeBytes = 0
  let teeEvents = 0
  tee.on("data", (chunk: Buffer) => {
    teeBytes += chunk.length
    try {
      const events = parser.push(chunk.toString("utf8"))
      if (events.length > 0) { teeEvents += events.length; synth.feed(events) }
    } catch (e) {
      warnLog("[mitm] sse feed error", { error: e instanceof Error ? e.message : String(e) })
    }
  })
  tee.on("end", () => {
    try {
      const tail = parser.end()
      if (tail.length > 0) synth.feed(tail)
    } catch { /* ignore */ }
  })
  // CRITICAL: without this listener, tee.destroy(err) on a mid-stream upstream
  // abort emits an unhandled 'error' → uncaughtException. If the message misses
  // the benign allowlist in src/index.ts, the global handler calls shutdown(),
  // killing EVERY session on the server. Consume it here (verified: a
  // PassThrough with no 'error' listener throws on destroy(err)). The entry is
  // finalized via synth.abort() on the upstream-error / res-close paths.
  tee.on("error", () => { /* consumed */ })

  // Distinguishes a clean SSE finish from a mid-stream interrupt: only the
  // latter (user Ctrl+C → res 'close' while this is still false) must recycle
  // the shared pooled socket, so normal turn-ends never pay a re-handshake.
  let sseCompleted = false
  const upstreamNode = Readable.fromWeb(upstream.body)
  upstreamNode.on("data", (chunk: Buffer) => {
    // Feed our entry synthesizer (observation-only — never blocks).
    try { tee.write(chunk) } catch { /* tee already destroyed */ }
    // Forward to CLI without awaiting drain. Backpressure on `res` would
    // otherwise pull on `upstreamNode` (Readable.fromWeb attaches its own
    // backpressure to the underlying ReadableStream) and slow upstream.
    if (!res.writableEnded && !res.destroyed) {
      try { res.write(chunk) } catch { /* res already gone */ }
    }
  })
  upstreamNode.on("end", () => {
    sseCompleted = true
    try { tee.end() } catch { /* ignore */ }
    if (!res.writableEnded) try { res.end() } catch { /* ignore */ }
    // DIAG (#73): summarise what this SSE turn tee'd. `events:0` on a completed
    // SSE turn = parser/synth problem; healthy turns show hundreds of events.
    warnLog("[mitm][diag#73] SSE turn done", { session: opts.ptySessionId, teeBytes, teeEvents })
  })
  upstreamNode.on("error", (err: Error) => {
    warnLog("[mitm] upstream stream error", { error: err.message })
    // Recycle the pool so the next request opens a fresh socket instead of
    // reusing the reset one; finalize the synth entry explicitly rather than
    // relying on the res.destroy → res-close chain.
    recycleDispatcherOnReset(err)
    try { synth.abort("upstream-error", true) } catch { /* ignore */ }
    try { tee.destroy(err) } catch { /* ignore */ }
    if (!res.writableEnded && !res.destroyed) try { res.destroy(err) } catch { /* ignore */ }
  })
  // A late async write to a dead CLI socket emits 'error' on res; without this
  // listener it becomes an uncaughtException gated on the benign allowlist.
  res.on("error", (err: Error) => { warnLog("[mitm] res stream error", { error: err.message }) })
  res.on("close", () => {
    // `hard` only when the SSE had NOT completed — a genuine mid-stream interrupt.
    // A normal turn-end also fires 'close' and must not latch the synth off (that
    // races the async tee tail and truncates the response).
    try { synth.abort("client-closed", !sseCompleted) } catch { /* ignore */ }
    // Abort the upstream fetch so undici releases the socket (a silent
    // mid-stream stall would otherwise pin it in the pool); pause first so
    // already-queued 'data' events don't fire into the aborted synth.
    try { ac.abort() } catch { /* ignore */ }
    if (!upstreamNode.destroyed) { try { upstreamNode.pause() } catch { /* ignore */ } try { upstreamNode.destroy() } catch { /* ignore */ } }
    // A mid-stream close (user Ctrl+C) aborts the fetch as an AbortError, which
    // recycleDispatcherOnReset does NOT match — leaving a torn keep-alive socket
    // in the shared pool that starves the NEXT turn's tee→synth (streaming dies
    // until reconnect, while console+JSONL keep working). Drop the pool so the
    // next request opens a fresh socket. Guarded on !sseCompleted so a clean
    // turn-end (the normal case) never forces a re-handshake on other sessions.
    if (!sseCompleted) dropCachedDispatcher()
  })
}
