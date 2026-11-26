// ============================================================================
// Who answered the codex network probe, and what the user must change.
//
// Field lesson (2026-11): behind a TLS-intercepting Squid, `codex login`
// printed "error sending request" (CA missing), then "device code request
// failed with status 403" — the 403 page came from the proxy (domain not in
// its allowlist), while the direct path got 403 from OpenAI itself
// (`unsupported_country_region_territory`). codex names none of the three.
// OpenAI's hosts sit behind Cloudflare, so a genuine answer carries `cf-ray`;
// an error status without it came from a hop in between.
// ============================================================================

import { classifyCodexError, type CodexErrorInfo } from './codex-errors'

/** Hosts codex needs: device-code sign-in, the ChatGPT backend, the OpenAI API. */
export const CODEX_HOSTS = ['auth.openai.com', 'chatgpt.com', 'api.openai.com'] as const

export type HostVerdictKind = 'ok' | 'proxy-auth' | 'proxy-denied' | 'region-blocked' | 'tls' | 'network' | 'other'

export interface HostVerdict {
  host: string
  kind: HostVerdictKind
  /** HTTP status of the answer (or of the refused CONNECT); null without one. */
  status: number | null
  detail: string
}

export interface ProbeAnswer {
  status: number
  headers: Record<string, string | string[] | undefined>
  body: string
}

const REGION = /unsupported_country_region_territory|unsupported_country/i

function header(headers: ProbeAnswer['headers'], name: string): string {
  const value = headers[name]
  return Array.isArray(value) ? value.join(', ') : (value ?? '')
}

function fromTarget(headers: ProbeAnswer['headers']): boolean {
  if (header(headers, 'cf-ray') || /cloudflare/i.test(header(headers, 'server'))) return true
  return Object.keys(headers).some((key) => /^(x-)?openai-/i.test(key))
}

/** What identifies the hop that answered instead of OpenAI (Squid: Server, X-Squid-Error). */
function hopSignature(answer: ProbeAnswer): string {
  const parts: string[] = []
  for (const name of ['server', 'via', 'x-squid-error']) {
    const value = header(answer.headers, name)
    if (value) parts.push(`${name}: ${value}`)
  }
  const title = /<title[^>]*>([^<]{1,120})<\/title>/i.exec(answer.body)?.[1]?.trim()
  if (title) parts.push(`page "${title}"`)
  return parts.length ? `; ${parts.join('; ')}` : ''
}

export function verdictFromAnswer(host: string, answer: ProbeAnswer): HostVerdict {
  const { status } = answer
  if (status >= 400 && REGION.test(answer.body)) {
    return { host, kind: 'region-blocked', status, detail: `HTTP ${status} unsupported_country_region_territory` }
  }
  if (fromTarget(answer.headers)) return { host, kind: 'ok', status, detail: `HTTP ${status} from ${host}` }
  if (status === 407) return { host, kind: 'proxy-auth', status, detail: `HTTP 407${hopSignature(answer)}` }
  if (status >= 400) {
    return { host, kind: 'proxy-denied', status, detail: `HTTP ${status} without cf-ray${hopSignature(answer)}` }
  }
  return { host, kind: 'ok', status, detail: `HTTP ${status}` }
}

/** undici reports a refused tunnel as "Proxy response (403) !== 200 when HTTP Tunneling". */
export function verdictFromError(host: string, text: string): HostVerdict {
  const tunnel = /Proxy response \((\d{3})\)/.exec(text)
  if (tunnel) {
    const status = Number(tunnel[1])
    const kind = status === 407 ? 'proxy-auth' : 'proxy-denied'
    return { host, kind, status, detail: `proxy refused CONNECT ${host}:443 with HTTP ${status}` }
  }
  const { kind } = classifyCodexError(text)
  const named = kind === 'tls' || kind === 'network' || kind === 'proxy-auth'
  return { host, kind: named ? kind : 'other', status: null, detail: text }
}

const ALLOWLIST = CODEX_HOSTS.join(', ')

function hint(kind: Exclude<HostVerdictKind, 'ok' | 'other'>, hosts: string, viaProxy: boolean): string {
  switch (kind) {
    case 'proxy-auth':
      return `The corporate proxy requires authentication (HTTP 407) for ${hosts}. Put the credentials into the proxy URL in Bridge network settings.`
    case 'tls':
      return `TLS verification failed for ${hosts}: the certificate${viaProxy ? ' presented through the proxy' : ''} is not trusted. Upload the corporate CA certificate (PEM) in the dashboard config under Network / Proxy → CA Certificate (or set CLAUDE_PROXY_CA_CERT); codex receives it as CODEX_CA_CERTIFICATE.`
    case 'proxy-denied':
      return viaProxy
        ? `The proxy refuses ${hosts} (the answer came from the proxy, not from OpenAI). Add ${ALLOWLIST} to the proxy's allowed domains.`
        : `A network filter between Bridge and ${hosts} answered instead of OpenAI. Allow ${ALLOWLIST} on that filter.`
    case 'region-blocked':
      return `OpenAI does not accept this network's exit address (HTTP 403 unsupported_country_region_territory from ${hosts}). Route Bridge through a proxy or network that exits in a region OpenAI supports.`
    case 'network':
      return `Bridge cannot connect to ${hosts} (connection or DNS failure${viaProxy ? ' via the proxy' : ''}). Check the proxy address and the network and DNS access of the Bridge server.`
  }
}

/** Most actionable first: a fix earlier in the list can change the answers below it. */
const PRIORITY = ['proxy-auth', 'tls', 'proxy-denied', 'region-blocked', 'network'] as const

/** The one cause to show, naming every host it hits; null when all hosts answered. */
export function summarizeVerdicts(verdicts: HostVerdict[], viaProxy: boolean): CodexErrorInfo | null {
  const detail = verdicts
    .filter((v) => v.kind !== 'ok')
    .map((v) => `${v.host}: ${v.detail}`)
    .join('; ')
  for (const kind of PRIORITY) {
    const hosts = verdicts.filter((v) => v.kind === kind).map((v) => v.host)
    if (hosts.length) return { kind, message: hint(kind, hosts.join(', '), viaProxy), detail }
  }
  return null
}
