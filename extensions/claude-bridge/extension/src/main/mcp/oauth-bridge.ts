// OAuth lifecycle helpers extracted from `manager.ts` (Sprint 2 / Stage C, C3).
// All functions are pure — they take the manager's `servers` Map (and other
// deps) explicitly so the class methods become thin one-liners that delegate
// here. Keeps lifecycle (connect / disconnect / reload) logic separate from
// token / refresh / metadata persistence noise in the main file.
//
// State that LIVES IN HERE: `inflightRefresh` map — shared across all calls,
// hidden behind module scope so concurrent HTTP requests with expiring tokens
// coalesce on a single refresh promise.

import {
  discoverAuthorizationServerMetadata,
  refreshAccessToken,
  type OAuthServerMetadata,
} from './oauth-flow'
import { loadTokens, saveTokens, deleteTokens, type StoredTokens } from './oauth-store'
import type { McpServerState } from './transports/context'

export interface TokenCtx {
  serverName: string
  serverUrl: string
  authServerUrl?: string
}

/** Tokens live in `~/.claude/.credentials.json` keyed by serverUrl, so every
 *  call needs the URL. This helper centralises the lookup + the name/url
 *  context object `saveTokens` expects. Returns null when the server has no
 *  HTTP URL (stdio servers don't use OAuth). */
export function buildTokenCtx(state: McpServerState | undefined): TokenCtx | null {
  if (!state?.config.url) return null
  return {
    serverName: state.config.name,
    serverUrl: state.config.url,
    authServerUrl: state.config.oauth?.authServerUrl,
  }
}

/** Persist a discovered OAuth Authorization Server URL into the server's
 *  config (with empty clientId so the UI prompts the user to enter it before
 *  running authorize). Called by the HTTP transport when it sees a 401 with
 *  an RFC 9728 challenge. Mutates `state.config.oauth` in place. */
export function setDiscoveredOAuthMetadata(
  state: McpServerState | undefined,
  asUrl: string,
  scopes?: string[],
): boolean {
  if (!state) return false
  state.config.oauth = {
    authServerUrl: asUrl,
    clientId: state.config.oauth?.clientId ?? '',
    ...(state.config.oauth?.clientSecret ? { clientSecret: state.config.oauth.clientSecret } : {}),
    ...(scopes && scopes.length ? { scope: scopes.join(' ') } : {}),
    ...(state.config.oauth?.callbackPort ? { callbackPort: state.config.oauth.callbackPort } : {}),
  }
  return true
}

/** Save freshly acquired tokens (called from the OAuth flow launcher).
 *  No-ops silently when the server has no URL (stdio). */
export function saveOAuthTokensFor(
  state: McpServerState | undefined,
  tokens: StoredTokens,
): void {
  const ctx = buildTokenCtx(state)
  if (!ctx) return
  saveTokens(ctx, {
    ...tokens,
    clientId: tokens.clientId ?? state?.config.oauth?.clientId,
    clientSecret: tokens.clientSecret ?? state?.config.oauth?.clientSecret,
  })
}

/** Merge newly registered OAuth credentials (Dynamic Client Registration)
 *  into the server's persisted config. Called by the oauth-connect handler
 *  after a successful RFC 7591 registration. Returns true if state was
 *  modified (caller should call saveConfigs/notifyChanged). */
export function setOAuthClientCredentialsFor(
  state: McpServerState | undefined,
  clientId: string,
  clientSecret?: string,
): boolean {
  if (!state?.config.oauth) return false
  state.config.oauth = {
    ...state.config.oauth,
    clientId,
    ...(clientSecret ? { clientSecret } : {}),
  }
  return true
}

/** Remove stored tokens for a server (logout). */
export function clearOAuthTokensFor(state: McpServerState | undefined): void {
  const url = state?.config.url
  if (url) deleteTokens(url)
}

/** Check whether a server currently has valid-looking stored tokens. */
export function hasOAuthTokensFor(state: McpServerState | undefined): boolean {
  const url = state?.config.url
  return !!url && loadTokens(url) !== null
}

/** Async — fetch the OAuth metadata (token_endpoint, authorization_endpoint
 *  etc.) from the server's authServerUrl. Returns null when the server has
 *  no OAuth config. */
export async function getOAuthMetadataFor(
  state: McpServerState | undefined,
): Promise<OAuthServerMetadata | null> {
  if (!state?.config.oauth) return null
  try {
    return await discoverAuthorizationServerMetadata(state.config.oauth.authServerUrl)
  } catch (err) {
    console.error('[MCP OAuth] discoverAuthorizationServerMetadata failed:', err instanceof Error ? err.message : err)
    return null
  }
}

// ---------------------------------------------------------------------------
// Refresh coordination — concurrent callers coalesce on the same promise so
// a single refresh fires per (server-id × parallel-request burst).
// ---------------------------------------------------------------------------

const inflightRefresh = new Map<string, Promise<boolean>>()

/** Returns true if refresh succeeded and tokens were updated. Coalesces
 *  concurrent calls per `id` into a single underlying refresh request. */
export function tryRefreshTokens(
  id: string,
  state: McpServerState | undefined,
): Promise<boolean> {
  const existing = inflightRefresh.get(id)
  if (existing) return existing
  const p = doRefreshTokens(state).finally(() => {
    inflightRefresh.delete(id)
  })
  inflightRefresh.set(id, p)
  return p
}

async function doRefreshTokens(state: McpServerState | undefined): Promise<boolean> {
  const ctx = buildTokenCtx(state)
  if (!state?.config.oauth || !ctx) return false
  const current = loadTokens(ctx.serverUrl)
  if (!current?.refresh_token) return false

  try {
    const metadata = await discoverAuthorizationServerMetadata(state.config.oauth.authServerUrl)
    const refreshed = await refreshAccessToken(
      metadata.token_endpoint,
      state.config.oauth.clientId,
      current.refresh_token,
      state.config.oauth.clientSecret,
    )
    const next: StoredTokens = {
      ...current,
      ...refreshed,
      authServerUrl: state.config.oauth.authServerUrl,
      lastRefreshAt: Date.now(),
      clientId: state.config.oauth.clientId,
      clientSecret: state.config.oauth.clientSecret,
    }
    saveTokens(ctx, next)
    return true
  } catch (err) {
    console.error(`[MCP OAuth] Refresh failed for "${state.config.name}":`, err instanceof Error ? err.message : err)
    return false
  }
}
