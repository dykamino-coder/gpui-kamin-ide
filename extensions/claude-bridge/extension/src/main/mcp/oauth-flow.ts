// MCP OAuth 2.0 authorization-code flow with PKCE.
//
// Standalone helpers — no host-runtime imports here, so the module can be tested
// against a mock OAuth server. Host integration (browser open and token
// storage) lives in `oauth-store.ts` and the UI wiring.

import { createServer, type Server } from 'http'
import { randomBytes, createHash } from 'crypto'

export interface OAuthServerMetadata {
  authorization_endpoint: string
  token_endpoint: string
  /** OAuth 2.1 draft — required if dynamic client registration is supported. */
  registration_endpoint?: string
  /** Space-separated string values; if omitted, servers fall back to provider defaults. */
  scopes_supported?: string[]
  code_challenge_methods_supported?: string[]
}

export interface OAuthTokens {
  access_token: string
  refresh_token?: string
  token_type?: string
  expires_in?: number
  /** Populated by the helpers: absolute epoch ms when access_token expires. */
  expires_at?: number
  scope?: string
}

export interface OAuthAuthorizeParams {
  metadata: OAuthServerMetadata
  clientId: string
  /** Optional — confidential clients. Public clients (PKCE-only) omit this. */
  clientSecret?: string
  scope?: string
  /** Callback port; 0 = pick any free port. */
  callbackPort?: number
  /** Hook invoked with the authorization URL so the caller can open a browser. */
  openAuthorizationUrl: (url: string) => void | Promise<void>
  /** Signal to cancel a pending flow. */
  signal?: AbortSignal
  /** Optional server identifier — mixed into the state so two concurrent
   *  OAuth flows can't have their codes swapped. */
  serverId?: string
}

export interface PkceChallenge {
  codeVerifier: string
  codeChallenge: string
  method: 'S256'
}

/** Generate a PKCE verifier + S256 challenge pair. */
export function generatePkce(): PkceChallenge {
  // 32 random bytes → 43-char base64url → meets RFC 7636 length requirements
  const verifier = base64UrlEncode(randomBytes(32))
  const challenge = base64UrlEncode(createHash('sha256').update(verifier).digest())
  return { codeVerifier: verifier, codeChallenge: challenge, method: 'S256' }
}

function base64UrlEncode(buf: Buffer): string {
  return buf.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

/**
 * Fetch the authorization server metadata.
 * Accepts either a full metadata URL or an issuer URL — falls back to
 * `<issuer>/.well-known/oauth-authorization-server`.
 */
export async function discoverAuthorizationServerMetadata(
  metadataOrIssuerUrl: string,
  fetchImpl: typeof fetch = fetch,
): Promise<OAuthServerMetadata> {
  // Normalize: if the URL doesn't end with a metadata path, append it.
  const url = /\.well-known\/(oauth-authorization-server|openid-configuration)/.test(metadataOrIssuerUrl)
    ? metadataOrIssuerUrl
    : metadataOrIssuerUrl.replace(/\/$/, '') + '/.well-known/oauth-authorization-server'

  const res = await fetchImpl(url)
  if (!res.ok) throw new Error(`OAuth metadata fetch failed: HTTP ${res.status} for ${url}`)
  const data = await res.json() as Partial<OAuthServerMetadata>
  if (!data.authorization_endpoint || !data.token_endpoint) {
    throw new Error(`OAuth metadata missing required fields from ${url}`)
  }
  return data as OAuthServerMetadata
}

/**
 * Run the full authorization-code flow:
 *  1. Generate PKCE verifier + challenge
 *  2. Start local HTTP server on `callbackPort`
 *  3. Invoke `openAuthorizationUrl(authUrl)` so caller can launch the browser
 *  4. Wait for the `/callback?code=...&state=...` redirect
 *  5. Exchange code for tokens at `token_endpoint`
 */
export async function authorize(params: OAuthAuthorizeParams, fetchImpl: typeof fetch = fetch): Promise<OAuthTokens> {
  const { metadata, clientId, clientSecret, scope, openAuthorizationUrl, signal } = params
  const pkce = generatePkce()
  // Bind serverId into the random state so a malicious AS can't swap a
  // code issued for server A onto server B's in-flight flow.
  const stateSuffix = params.serverId ? '.' + base64UrlEncode(Buffer.from(params.serverId, 'utf-8')) : ''
  const state = base64UrlEncode(randomBytes(16)) + stateSuffix

  const { server, port, codePromise } = await startCallbackServer(params.callbackPort ?? 0, state, signal)

  try {
    const redirectUri = `http://localhost:${port}/callback`
    const authUrl = buildAuthorizationUrl(metadata.authorization_endpoint, {
      response_type: 'code',
      client_id: clientId,
      redirect_uri: redirectUri,
      code_challenge: pkce.codeChallenge,
      code_challenge_method: pkce.method,
      state,
      ...(scope ? { scope } : {}),
    })

    await openAuthorizationUrl(authUrl)

    const { code } = await codePromise

    // Exchange code for tokens
    const tokens = await exchangeCodeForTokens(metadata.token_endpoint, {
      grant_type: 'authorization_code',
      code,
      redirect_uri: redirectUri,
      client_id: clientId,
      code_verifier: pkce.codeVerifier,
      ...(clientSecret ? { client_secret: clientSecret } : {}),
    }, fetchImpl)

    return tokens
  } finally {
    server.close()
  }
}

export function buildAuthorizationUrl(authorizeEndpoint: string, params: Record<string, string>): string {
  const u = new URL(authorizeEndpoint)
  for (const [k, v] of Object.entries(params)) u.searchParams.set(k, v)
  return u.toString()
}

export async function exchangeCodeForTokens(
  tokenEndpoint: string,
  body: Record<string, string>,
  fetchImpl: typeof fetch = fetch,
): Promise<OAuthTokens> {
  const form = new URLSearchParams()
  for (const [k, v] of Object.entries(body)) form.set(k, v)

  const res = await fetchImpl(tokenEndpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      'Accept': 'application/json',
    },
    body: form.toString(),
  })

  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`Token exchange failed: HTTP ${res.status} — ${text.slice(0, 200)}`)
  }

  const data = await res.json() as OAuthTokens
  // Strict shape check: some providers return 200 with a null / empty-string
  // access_token on soft failure — if we don't catch it here, the manager
  // later sends `Authorization: Bearer null` to the MCP server.
  if (typeof data.access_token !== 'string' || data.access_token.length === 0) {
    throw new Error('Token response missing or empty access_token')
  }
  if (data.refresh_token !== undefined && typeof data.refresh_token !== 'string') {
    throw new Error('Token response has non-string refresh_token')
  }
  if (data.expires_in) {
    data.expires_at = Date.now() + data.expires_in * 1000
  }
  return data
}

export interface DynamicClientRegistration {
  client_id: string
  client_secret?: string
  client_id_issued_at?: number
  client_secret_expires_at?: number
  redirect_uris?: string[]
}

/** Dynamic Client Registration (RFC 7591).
 *  Lets us obtain a clientId/clientSecret from the authorization server on
 *  the fly — needed for servers like Figma that don't publish a fixed
 *  public clientId. `redirectUris` should include the localhost callback
 *  we intend to listen on; if the AS pins the redirect, omit it and let
 *  the AS derive one. */
export async function registerOAuthClient(
  registrationEndpoint: string,
  metadata: {
    client_name: string
    redirect_uris?: string[]
    scope?: string
    token_endpoint_auth_method?: string
    grant_types?: string[]
    response_types?: string[]
  },
  fetchImpl: typeof fetch = fetch,
): Promise<DynamicClientRegistration> {
  // Figma (and some other ASes) reject calls that advertise a specific
  // token_endpoint_auth_method. Try the full form first; on HTTP 400/403
  // retry with a progressively smaller body so we don't spuriously fail
  // on servers that gate fields behind specific presence/absence rules.
  const attempts: Array<Record<string, unknown>> = [
    {
      // OIDC-flavoured form — most servers accept this.
      client_name: metadata.client_name,
      application_type: 'native',
      grant_types: metadata.grant_types ?? ['authorization_code', 'refresh_token'],
      response_types: metadata.response_types ?? ['code'],
      token_endpoint_auth_method: metadata.token_endpoint_auth_method ?? 'client_secret_post',
      ...(metadata.redirect_uris && metadata.redirect_uris.length > 0 ? { redirect_uris: metadata.redirect_uris } : {}),
      ...(metadata.scope ? { scope: metadata.scope } : {}),
    },
    {
      // Without application_type / auth method — RFC 7591 plain form.
      client_name: metadata.client_name,
      grant_types: metadata.grant_types ?? ['authorization_code', 'refresh_token'],
      response_types: metadata.response_types ?? ['code'],
      ...(metadata.redirect_uris && metadata.redirect_uris.length > 0 ? { redirect_uris: metadata.redirect_uris } : {}),
      ...(metadata.scope ? { scope: metadata.scope } : {}),
    },
    {
      // Minimal body — bare name + redirect.
      client_name: metadata.client_name,
      ...(metadata.redirect_uris && metadata.redirect_uris.length > 0 ? { redirect_uris: metadata.redirect_uris } : {}),
    },
  ]

  // Some ASes (Figma, Linear) gate DCR behind a Cloudflare-style bot check
  // that rejects "uncommon" User-Agents. Claude Code succeeds because it
  // identifies as an MCP client; anthropic's SDK sets `node` by default
  // which Figma's edge doesn't block. Mirror that.
  const endpointOrigin = (() => {
    try { return new URL(registrationEndpoint).origin } catch { return '' }
  })()
  const headerAttempts: Array<Record<string, string>> = [
    {
      'Content-Type': 'application/json',
      'Accept': 'application/json',
      'User-Agent': 'claude-code/1.0 (mcp)',
    },
    {
      'Content-Type': 'application/json',
      'Accept': 'application/json',
      'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
      ...(endpointOrigin ? { 'Origin': endpointOrigin, 'Referer': endpointOrigin + '/' } : {}),
    },
    {
      'Content-Type': 'application/json',
      'Accept': 'application/json',
    },
  ]

  let lastError = ''
  let lastStatus = 0
  outer: for (const body of attempts) {
    for (const headers of headerAttempts) {
      const res = await fetchImpl(registrationEndpoint, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
      })
      if (res.ok) {
        const data = await res.json() as DynamicClientRegistration
        if (typeof data.client_id !== 'string' || data.client_id.length === 0) {
          throw new Error('Registration response missing client_id')
        }
        return data
      }
      lastStatus = res.status
      lastError = await res.text().catch(() => '')
      // 401/404/5xx won't get better with a different header — stop the whole retry.
      if (res.status !== 400 && res.status !== 403 && res.status !== 415) break outer
    }
  }
  throw new Error(`DCR failed at ${registrationEndpoint}: HTTP ${lastStatus} — ${lastError.slice(0, 500)}`)
}

/** Refresh an access token using the refresh_token grant. */
export async function refreshAccessToken(
  tokenEndpoint: string,
  clientId: string,
  refreshToken: string,
  clientSecret?: string,
  fetchImpl: typeof fetch = fetch,
): Promise<OAuthTokens> {
  const tokens = await exchangeCodeForTokens(tokenEndpoint, {
    grant_type: 'refresh_token',
    refresh_token: refreshToken,
    client_id: clientId,
    ...(clientSecret ? { client_secret: clientSecret } : {}),
  }, fetchImpl)
  // Some providers omit refresh_token on refresh response — keep the old one.
  if (!tokens.refresh_token) tokens.refresh_token = refreshToken
  return tokens
}

interface CallbackResult {
  server: Server
  port: number
  codePromise: Promise<{ code: string; state: string }>
}

/**
 * Start a short-lived HTTP server that waits for the OAuth redirect.
 * Rejects on state mismatch or abort. Caller is responsible for `server.close()`.
 */
function startCallbackServer(
  port: number,
  expectedState: string,
  signal?: AbortSignal,
): Promise<CallbackResult> {
  return new Promise((resolveOuter, rejectOuter) => {
    let codeResolve: (value: { code: string; state: string }) => void
    let codeReject: (err: Error) => void
    const codePromise = new Promise<{ code: string; state: string }>((res, rej) => {
      codeResolve = res
      codeReject = rej
    })

    const server = createServer((req, res) => {
      try {
        if (!req.url || !req.url.startsWith('/callback')) {
          res.writeHead(404).end('Not found')
          return
        }
        const u = new URL(req.url, 'http://localhost')
        const code = u.searchParams.get('code')
        const state = u.searchParams.get('state')
        const error = u.searchParams.get('error')

        if (error) {
          res.writeHead(400, { 'Content-Type': 'text/html' })
          res.end(renderCallbackHtml(`Authorization failed: ${error}`))
          codeReject(new Error(`OAuth error: ${error}`))
          return
        }
        if (!code || !state) {
          res.writeHead(400, { 'Content-Type': 'text/html' })
          res.end(renderCallbackHtml('Missing code or state parameter'))
          codeReject(new Error('Missing code or state in callback'))
          return
        }
        if (state !== expectedState) {
          res.writeHead(400, { 'Content-Type': 'text/html' })
          res.end(renderCallbackHtml('State mismatch — request rejected'))
          codeReject(new Error('State mismatch'))
          return
        }

        res.writeHead(200, { 'Content-Type': 'text/html' })
        res.end(renderCallbackHtml('Authorization complete. You can close this window.'))
        codeResolve({ code, state })
      } catch (err) {
        res.writeHead(500).end('Server error')
        codeReject(err instanceof Error ? err : new Error(String(err)))
      }
    })

    if (signal) {
      signal.addEventListener('abort', () => {
        codeReject(new Error('OAuth flow aborted'))
        server.close()
      }, { once: true })
    }

    server.on('error', (err) => {
      rejectOuter(err)
      codeReject(err)
    })

    // Suppress unhandled-rejection warnings if the browser callback fires
    // before the caller has had a chance to `await codePromise`. The real
    // await inside `authorize()` still receives the rejection.
    codePromise.catch(() => { /* noop */ })

    server.listen(port, '127.0.0.1', () => {
      const addr = server.address()
      const actualPort = typeof addr === 'object' && addr ? addr.port : port
      resolveOuter({ server, port: actualPort, codePromise })
    })
  })
}

function renderCallbackHtml(message: string): string {
  const safe = message.replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[c]!))
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><title>Open Claude Bridge</title>
<style>body{font-family:system-ui,-apple-system,sans-serif;background:#1e1e2e;color:#cdd6f4;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}
.card{background:#181825;border:1px solid #313244;border-radius:12px;padding:32px;max-width:420px;text-align:center}
h1{margin:0 0 8px;font-size:18px}p{margin:0;color:#a6adc8;font-size:14px}</style>
</head><body><div class="card"><h1>Open Claude Bridge</h1><p>${safe}</p></div></body></html>`
}
