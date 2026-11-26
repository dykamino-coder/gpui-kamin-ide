// ============================================================================
// Why codex cannot reach OpenAI, when codex itself does not say.
//
// Field lessons: behind a proxy that answers 407, and behind a TLS-intercepting
// proxy whose CA Bridge does not trust, codex 0.160.1 reports both as
// "workspace routing discovery failed"; a proxy that refuses OpenAI's domains
// and OpenAI refusing the exit region both reach the user as a bare 403. This
// probe repeats the connections codex makes — same proxy, NO_PROXY and extra
// CA from the codex environment, certificate verification on — to every host
// codex needs, and network-verdict.ts names who answered.
// ============================================================================

import fs from 'node:fs'
import tls from 'node:tls'
import { EnvHttpProxyAgent, request, type Dispatcher } from 'undici'
import { CODEX_CA_ENV } from './codex-home'
import type { CodexErrorInfo } from './codex-errors'
import {
  CODEX_HOSTS,
  summarizeVerdicts,
  verdictFromAnswer,
  verdictFromError,
  type HostVerdict,
} from './network-verdict'

const PROBE_TIMEOUT_MS = 5_000
/** Enough for a proxy error page title or OpenAI's JSON error code. */
const BODY_CAP = 16 * 1024

/** api.openai.com checks the region before the key: /v1/models answers 401, or 403 with the region code. */
export function codexProbeUrl(host: string): string {
  return host === 'api.openai.com' ? `https://${host}/v1/models` : `https://${host}/`
}

export interface NetworkProbeOptions {
  hosts?: readonly string[]
  /** Replaces codexProbeUrl (tests point every host at a local server). */
  urlFor?: (host: string) => string
  timeoutMs?: number
}

export interface CodexNetworkReport {
  checkedAt: number
  viaProxy: boolean
  /** An extra CA (CODEX_CA_CERTIFICATE / SSL_CERT_FILE) was given to codex. */
  customCa: boolean
  hosts: HostVerdict[]
  /** The cause to show the user; null when every host answered. */
  problem: CodexErrorInfo | null
}

function proxyFor(env: NodeJS.ProcessEnv): string | undefined {
  return env.HTTPS_PROXY || env.https_proxy || env.ALL_PROXY || env.all_proxy || undefined
}

function caFile(env: NodeJS.ProcessEnv): string | undefined {
  return env[CODEX_CA_ENV] || env.SSL_CERT_FILE || undefined
}

/** Native roots plus codex's extra CA (CODEX_CA_CERTIFICATE, fallback SSL_CERT_FILE). */
function trustRoots(env: NodeJS.ProcessEnv): string[] {
  const file = caFile(env)
  const roots = [...tls.rootCertificates]
  if (file) {
    try {
      roots.push(fs.readFileSync(file, 'utf8'))
    } catch {
      // An unreadable CA file leaves the native roots; the probe then reports TLS.
    }
  }
  return roots
}

function errorText(error: unknown): string {
  const parts: string[] = []
  for (let e = error as { message?: string; code?: string; cause?: unknown } | undefined; e; e = e.cause as typeof e) {
    if (e.message) parts.push(e.message)
    if (e.code) parts.push(e.code)
    if (parts.length > 8) break
  }
  return parts.join(' ← ')
}

async function readCapped(body: AsyncIterable<Buffer>): Promise<string> {
  const chunks: Buffer[] = []
  let size = 0
  for await (const chunk of body) {
    chunks.push(chunk)
    size += chunk.length
    if (size >= BODY_CAP) break
  }
  return Buffer.concat(chunks).subarray(0, BODY_CAP).toString('utf8')
}

async function probeHost(host: string, url: string, dispatcher: Dispatcher, timeoutMs: number): Promise<HostVerdict> {
  try {
    const res = await request(url, {
      method: 'GET',
      dispatcher,
      headers: { 'user-agent': 'open-claude-bridge-codex-probe', accept: '*/*' },
      signal: AbortSignal.timeout(timeoutMs),
    })
    const body = await readCapped(res.body)
    return verdictFromAnswer(host, { status: res.statusCode, headers: res.headers, body })
  } catch (error) {
    return verdictFromError(host, errorText(error))
  }
}

/** Checks every host codex needs, in parallel, the way codex connects. */
export async function probeCodexNetwork(
  env: NodeJS.ProcessEnv,
  options: NetworkProbeOptions = {},
): Promise<CodexNetworkReport> {
  const proxy = proxyFor(env)
  const connect = { ca: trustRoots(env), rejectUnauthorized: true }
  // Empty strings, not undefined: undici would fall back to this server's own env.
  const dispatcher = new EnvHttpProxyAgent({
    httpProxy: '',
    httpsProxy: proxy ?? '',
    noProxy: env.NO_PROXY ?? env.no_proxy ?? '',
    connect,
    requestTls: connect,
  })
  const urlFor = options.urlFor ?? codexProbeUrl
  const timeoutMs = options.timeoutMs ?? PROBE_TIMEOUT_MS
  try {
    const hosts = await Promise.all(
      (options.hosts ?? CODEX_HOSTS).map((host) => probeHost(host, urlFor(host), dispatcher, timeoutMs)),
    )
    return {
      checkedAt: Date.now(),
      viaProxy: Boolean(proxy),
      customCa: Boolean(caFile(env)),
      hosts,
      problem: summarizeVerdicts(hosts, Boolean(proxy)),
    }
  } finally {
    void dispatcher.close().catch(() => undefined)
  }
}
