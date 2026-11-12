// ============================================================================
// Why codex cannot reach OpenAI, when codex itself does not say.
//
// Field lesson (acceptance 2026-11-12): behind a corporate proxy that answers
// 407, and behind a TLS-intercepting proxy whose CA Bridge does not trust,
// codex 0.160.1 reports both as "workspace routing discovery failed" — no
// status, no certificate error. Bridge then showed "ChatGPT is not signed in"
// and opened new GPT chats on Claude. This probe repeats the connection codex
// makes (same proxy and extra CA from the codex environment, certificate
// verification on) and names the cause in the words of classifyCodexError.
// ============================================================================

import fs from 'node:fs'
import tls from 'node:tls'
import { Agent, ProxyAgent, request, type Dispatcher } from 'undici'
import { CODEX_CA_ENV } from './codex-home'
import { classifyCodexError, type CodexErrorInfo } from './codex-errors'

export const CODEX_PROBE_URL = 'https://chatgpt.com/'
const PROBE_TIMEOUT_MS = 5_000

export interface NetworkProbeOptions {
  url?: string
  timeoutMs?: number
}

function proxyFor(env: NodeJS.ProcessEnv): string | undefined {
  return env.HTTPS_PROXY || env.https_proxy || env.ALL_PROXY || env.all_proxy || undefined
}

/** Native roots plus codex's extra CA (CODEX_CA_CERTIFICATE, fallback SSL_CERT_FILE). */
function trustRoots(env: NodeJS.ProcessEnv): string[] {
  const file = env[CODEX_CA_ENV] || env.SSL_CERT_FILE
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

/** The proxy/TLS/network problem between this server and OpenAI, or null when
 *  the connection works (or fails for a reason this probe cannot name). */
export async function probeCodexNetwork(
  env: NodeJS.ProcessEnv,
  options: NetworkProbeOptions = {},
): Promise<CodexErrorInfo | null> {
  const proxy = proxyFor(env)
  const connect = { ca: trustRoots(env), rejectUnauthorized: true }
  const dispatcher: Dispatcher = proxy ? new ProxyAgent({ uri: proxy, requestTls: connect }) : new Agent({ connect })
  try {
    const res = await request(options.url ?? CODEX_PROBE_URL, {
      method: 'HEAD',
      dispatcher,
      signal: AbortSignal.timeout(options.timeoutMs ?? PROBE_TIMEOUT_MS),
    })
    await res.body.dump()
    return null
  } catch (error) {
    const info = classifyCodexError(errorText(error))
    return info.kind === 'proxy-auth' || info.kind === 'tls' || info.kind === 'network' ? info : null
  } finally {
    void dispatcher.close().catch(() => undefined)
  }
}
