import TurndownService from 'turndown'
import { lookup } from 'node:dns/promises'
import net from 'node:net'

export type McpResult = { content: Array<{ type: string; text: string }> }

function textResult(text: string): McpResult {
  return { content: [{ type: 'text', text }] }
}

function errorResult(message: string): McpResult {
  return { content: [{ type: 'text', text: `Error: ${message}` }] }
}

const MAX_CONTENT_BYTES = 500 * 1024 // 500 KB — well under the CLI's ~720KB MCP-response budget
const MAX_RAW_HTML_BYTES = 5 * 1024 * 1024 // 5 MB — cap input before regex to prevent ReDoS

/**
 * Strip HTML tags and convert to readable text.
 * Simple regex-based approach for when turndown isn't available.
 */
function htmlToText(html: string): string {
  let text = html

  // Remove script and style blocks entirely
  text = text.replace(/<script[\s\S]*?<\/script>/gi, '')
  text = text.replace(/<style[\s\S]*?<\/style>/gi, '')
  text = text.replace(/<noscript[\s\S]*?<\/noscript>/gi, '')

  // Convert common block elements to newlines
  text = text.replace(/<\/?(p|div|br|hr|h[1-6]|li|tr|blockquote|pre|section|article|header|footer|nav|main|aside)[^>]*>/gi, '\n')

  // Convert list items
  text = text.replace(/<li[^>]*>/gi, '\n- ')

  // Convert headings - add markdown-style markers
  text = text.replace(/<h1[^>]*>(.*?)<\/h1>/gi, '\n# $1\n')
  text = text.replace(/<h2[^>]*>(.*?)<\/h2>/gi, '\n## $1\n')
  text = text.replace(/<h3[^>]*>(.*?)<\/h3>/gi, '\n### $1\n')

  // Convert links
  text = text.replace(/<a[^>]*href="([^"]*)"[^>]*>(.*?)<\/a>/gi, '[$2]($1)')

  // Remove all remaining tags
  text = text.replace(/<[^>]+>/g, '')

  // Decode HTML entities
  text = text.replace(/&amp;/g, '&')
  text = text.replace(/&lt;/g, '<')
  text = text.replace(/&gt;/g, '>')
  text = text.replace(/&quot;/g, '"')
  text = text.replace(/&#39;/g, "'")
  text = text.replace(/&nbsp;/g, ' ')

  // Clean up whitespace
  text = text.replace(/[ \t]+/g, ' ')
  text = text.replace(/\n{3,}/g, '\n\n')
  text = text.trim()

  return text
}

/**
 * Trim boilerplate before markdown conversion so the model gets the article,
 * not the chrome. Conservative + dependency-free (no readability/jsdom):
 *   1. drop script/style/noscript,
 *   2. prefer a semantic <main> block when it holds real content (>200 chars),
 *   3. drop nav/footer/aside blocks (NOT <header> — can hold an article title;
 *      NOT <form> — can hold search results).
 * Any leftover stray tags (e.g. an orphan </nav> from nested chrome) are
 * removed downstream by turndown/htmlToText. Regexes verified before shipping.
 */
function extractMainContent(html: string): string {
  let s = html
  s = s.replace(/<script[\s\S]*?<\/script>/gi, '')
  s = s.replace(/<style[\s\S]*?<\/style>/gi, '')
  s = s.replace(/<noscript[\s\S]*?<\/noscript>/gi, '')
  const mainMatch = s.match(/<main\b[^>]*>([\s\S]*?)<\/main>/i)
  if (mainMatch && mainMatch[1] && mainMatch[1].trim().length > 200) {
    s = mainMatch[1]
  }
  s = s.replace(/<(nav|footer|aside)\b[^>]*>[\s\S]*?<\/\1>/gi, '')
  return s
}

/**
 * Try to use Turndown for better HTML→Markdown conversion.
 * Falls back to regex-based stripping if not available.
 */
function convertHtml(html: string): string {
  // Statically imported above so Vite bundles it. The previous dynamic
  // `require('turndown')` silently broke in packaged builds: forge-vite
  // drops node_modules from the staged app, so the require failed and we
  // fell back to the regex htmlToText() — markdown formatting lost.
  try {
    const turndown = new TurndownService({
      headingStyle: 'atx',
      codeBlockStyle: 'fenced',
    })
    return turndown.turndown(html)
  } catch {
    return htmlToText(html)
  }
}

// ── SSRF protection ─────────────────────────────────────────────────────────
// The URL is attacker-controlled (the remote CLI asks us to fetch it), so the
// block list must resist every bypass: DNS names that resolve to internal IPs,
// IPv6 (loopback / ULA / link-local / v4-mapped), non-dotted-quad literals
// (integer / hex), and 3xx redirects to any of the above. We therefore resolve
// the host to the IPs it would actually connect to and reject if ANY is
// private, and we re-run this check on every redirect hop.

const MAX_REDIRECTS = 5

function ipv4IsPrivate(ip: string): boolean {
  const o = ip.split('.').map(Number)
  if (o.length !== 4 || o.some(n => !Number.isInteger(n) || n < 0 || n > 255)) return true // malformed → block
  const [a, b] = o as [number, number, number, number]
  if (a === 0 || a === 10 || a === 127) return true                 // this-net / private / loopback
  if (a === 169 && b === 254) return true                           // link-local + AWS/GCP metadata
  if (a === 172 && b >= 16 && b <= 31) return true                  // 172.16.0.0/12
  if (a === 192 && b === 168) return true                           // 192.168.0.0/16
  if (a === 100 && b >= 64 && b <= 127) return true                 // CGNAT 100.64.0.0/10
  if (a === 192 && b === 0 && o[2] === 0) return true               // 192.0.0.0/24
  if (a >= 224) return true                                         // multicast / reserved
  return false
}

function ipv6IsPrivate(ip: string): boolean {
  let s = ip.toLowerCase()
  if (s.startsWith('[') && s.endsWith(']')) s = s.slice(1, -1)
  const zone = s.indexOf('%'); if (zone >= 0) s = s.slice(0, zone) // strip fe80::1%eth0 zone id
  if (s === '::1' || s === '::') return true                        // loopback / unspecified
  // v4-mapped `::ffff:a.b.c.d`. The WHATWG URL parser normalises this to the HEX
  // hextet form (`::ffff:7f00:1`), so a dotted-decimal-only match was dead code
  // and `::ffff:127.0.0.1` / `::ffff:169.254.169.254` slipped straight through.
  const mapped = s.match(/^::ffff:(.+)$/)
  if (mapped) {
    const tail = mapped[1]!
    if (/^\d+\.\d+\.\d+\.\d+$/.test(tail)) return ipv4IsPrivate(tail)
    const g = tail.split(':')
    if (g.length === 2) {
      const hi = parseInt(g[0]!, 16), lo = parseInt(g[1]!, 16)
      if (Number.isFinite(hi) && Number.isFinite(lo)) {
        return ipv4IsPrivate([(hi >> 8) & 255, hi & 255, (lo >> 8) & 255, lo & 255].join('.'))
      }
    }
    return true // unparseable v4-mapped → fail closed
  }
  const head = s.split(':')[0] || ''
  if (/^f[cd]/.test(head)) return true                             // ULA fc00::/7
  if (/^fe[89ab]/.test(head)) return true                          // link-local fe80::/10
  return false
}

/** Decode the non-dotted-quad IPv4 literals browsers/curl accept (decimal
 *  `2130706433`, hex `0x7f000001`) into a dotted quad, else null. */
function decodeIntHost(host: string): string | null {
  let n: number | null = null
  if (/^\d+$/.test(host)) n = Number(host)
  else if (/^0x[0-9a-f]+$/i.test(host)) n = parseInt(host, 16)
  if (n === null || !Number.isFinite(n) || n < 0 || n > 0xffffffff) return null
  return [(n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].join('.')
}

/** True if the host resolves to (or literally is) a private/internal address. */
async function resolvesToPrivate(hostname: string): Promise<boolean> {
  const host = hostname.replace(/^\[|\]$/g, '')
  const kind = net.isIP(host)
  if (kind === 4) return ipv4IsPrivate(host)
  if (kind === 6) return ipv6IsPrivate(host)
  const asInt = decodeIntHost(host)
  if (asInt) return ipv4IsPrivate(asInt)
  try {
    const addrs = await lookup(host, { all: true })
    if (!addrs.length) return true
    return addrs.some(a => (a.family === 4 ? ipv4IsPrivate(a.address) : ipv6IsPrivate(a.address)))
  } catch {
    return false // DNS failure — let fetch surface the network error, not a false block
  }
}

/** Returns a human reason string if the URL must be blocked, else null. */
async function blockReason(parsed: URL): Promise<string | null> {
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return 'non-HTTP protocol'
  const host = parsed.hostname.toLowerCase().replace(/\.$/, '') // strip FQDN trailing dot (`localhost.`)
  if (host === 'localhost' || host.endsWith('.localhost')) return 'localhost'
  if (await resolvesToPrivate(host)) return 'private/internal address'
  return null
}

/** Read a response body, aborting once `maxBytes` is buffered. Enforces the cap
 *  even for chunked / no-Content-Length responses (where a post-hoc slice would
 *  already have materialised the whole body — the OOM the declared-length bail
 *  can't catch). */
async function readCapped(resp: Response, maxBytes: number): Promise<string> {
  const reader = resp.body?.getReader()
  if (!reader) return (await resp.text()).slice(0, maxBytes)
  const chunks: Uint8Array[] = []
  let total = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    if (value) {
      chunks.push(value)
      total += value.length
      if (total >= maxBytes) { try { await reader.cancel() } catch { /* already closed */ } break }
    }
  }
  const buf = Buffer.concat(chunks, total)
  return new TextDecoder('utf-8', { fatal: false }).decode(buf.subarray(0, maxBytes))
}

/** Fetch following up to MAX_REDIRECTS hops, re-validating each hop's target
 *  against blockReason so a public URL can't 302 into an internal address. */
async function fetchGuarded(start: URL, headers: Record<string, string>, signal: AbortSignal): Promise<Response | McpResult> {
  let current = start
  for (let hop = 0; ; hop++) {
    const reason = await blockReason(current)
    if (reason) return errorResult(`URL targets a blocked address (${reason})`)
    const resp = await fetch(current.toString(), { headers, signal, redirect: 'manual' })
    const isRedirect = resp.status >= 300 && resp.status < 400 && resp.headers.get('location')
    if (!isRedirect) return resp
    if (hop >= MAX_REDIRECTS) return errorResult(`Too many redirects fetching ${start.toString()}`)
    try { await resp.body?.cancel() } catch { /* ignore */ }
    try { current = new URL(resp.headers.get('location')!, current) } catch { return errorResult('Invalid redirect location') }
  }
}

/** Heuristic: does this look like an internal host that may legitimately serve
 *  plain http (no public TLS)? Single-label hosts (http://wiki) or the common
 *  internal TLDs. Used to skip the https auto-upgrade when a TLS handshake
 *  would just fail. Private-IP literals are still hard-blocked by isBlockedUrl. */
function isLikelyIntranetHost(hostname: string): boolean {
  if (!hostname.includes('.')) return true // single-label, e.g. http://wiki
  return /\.(local|internal|lan|corp|home|intranet)$/.test(hostname)
}

export async function webFetch(input: Record<string, unknown>): Promise<McpResult> {
  const url = input.url as string
  if (!url) return errorResult('url is required')

  const prompt = (input.prompt as string) || 'Extract the main content from this page'
  const allowHttp = input.allow_http === true

  try {
    // Validate URL
    let parsedUrl: URL
    try {
      parsedUrl = new URL(url)
    } catch {
      return errorResult(`Invalid URL: ${url}`)
    }

    // Auto-upgrade http → https, EXCEPT when the caller opts out (allow_http)
    // or the host looks like an intranet name that has no public TLS (an https
    // handshake there would just fail). SSRF is enforced per-hop by fetchGuarded.
    if (parsedUrl.protocol === 'http:' && !allowHttp && !isLikelyIntranetHost(parsedUrl.hostname.toLowerCase())) {
      parsedUrl.protocol = 'https:'
    }

    // Fetch with per-hop SSRF revalidation (a public URL must not 302 into an
    // internal address) — returns an McpResult on block/redirect-limit.
    const fetched = await fetchGuarded(parsedUrl, {
      'User-Agent': 'Mozilla/5.0 (compatible; OpenClaudeBridge/1.0)',
      'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    }, AbortSignal.timeout(30_000))
    if (!(fetched instanceof Response)) return fetched
    const response = fetched

    if (!response.ok) {
      return errorResult(`HTTP ${response.status} ${response.statusText} for ${url}`)
    }

    // Bail before buffering a server-declared oversize body.
    const declaredLen = Number(response.headers.get('content-length') || '0')
    if (declaredLen > MAX_RAW_HTML_BYTES * 4) {
      return errorResult(`Response too large (${Math.round(declaredLen / 1024 / 1024)} MB) for ${url} — fetch a more specific URL.`)
    }

    const contentType = response.headers.get('content-type') || ''
    // Stream-read with a hard byte cap — enforces the ceiling even when the
    // server sends no Content-Length or a chunked body.
    const body = await readCapped(response, MAX_RAW_HTML_BYTES)

    let content: string
    if (contentType.includes('text/html') || contentType.includes('application/xhtml')) {
      content = convertHtml(extractMainContent(body))
    } else {
      content = body
    }

    // Truncate if too large
    if (content.length > MAX_CONTENT_BYTES) {
      content = content.slice(0, MAX_CONTENT_BYTES) + `\n\n[Content truncated at ${Math.round(MAX_CONTENT_BYTES / 1024)}KB]`
    }

    return textResult(
      `URL: ${parsedUrl.toString()}\n` +
      `Prompt: ${prompt}\n\n` +
      `--- Content ---\n${content}`
    )
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    if (msg.includes('AbortError') || msg.includes('timeout')) {
      return errorResult(`Request timed out fetching ${url}`)
    }
    return errorResult(`Failed to fetch ${url}: ${msg}`)
  }
}
