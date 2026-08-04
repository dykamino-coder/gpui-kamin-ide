// Unified OAuth token storage — shared with Claude Code CLI.
//
// Previously we kept MCP OAuth tokens in `<userData>/mcp-oauth-tokens.json`
// encrypted via Electron `safeStorage`. That isolated us from the CLI and
// meant the user had to authorize twice. Now we read/write the same file
// Claude Code uses: `~/.claude/.credentials.json → mcpOAuth`.
//
// Entry shape (matches what Claude Code writes):
//   "mcpOAuth": {
//     "<serverName>|<hashOfUrl>": {
//       "serverName": "figma",
//       "serverUrl": "https://mcp.figma.com/mcp",
//       "accessToken": "figu_…",
//       "refreshToken": "figur_…",
//       "expiresAt": 1784477677168,
//       "clientId": "…",
//       "clientSecret": "…",
//       "discoveryState": {
//         "authorizationServerUrl": "https://api.figma.com",
//         "resourceMetadataUrl": "https://mcp.figma.com/.well-known/oauth-protected-resource",
//         "oauthMetadataFound": true
//       }
//     }
//   }
//
// We look up entries by `serverUrl` match — that way we find tokens no matter
// which tool produced them, regardless of how each tool hashes the URL into
// the key suffix.

import fs from 'fs'
import path from 'path'
import os from 'os'
import crypto from 'crypto'
import { app, safeStorage } from 'electron'
import type { OAuthTokens } from './oauth-flow'

export interface StoredTokens extends OAuthTokens {
  /** Issuer/authorization server URL this token belongs to. */
  authServerUrl?: string
  /** Last time the tokens were refreshed (epoch ms). */
  lastRefreshAt?: number
  /** Copied through so callers that have only the serverId can still access
   *  the dynamically-registered clientId/clientSecret used to refresh. */
  clientId?: string
  clientSecret?: string
}

export interface TokenContext {
  serverName: string
  serverUrl: string
  authServerUrl?: string
}

interface ClaudeCodeEntry {
  serverName?: string
  serverUrl?: string
  accessToken?: string
  refreshToken?: string
  expiresAt?: number
  clientId?: string
  clientSecret?: string
  discoveryState?: {
    authorizationServerUrl?: string
    resourceMetadataUrl?: string
    oauthMetadataFound?: boolean
  }
}

interface CredentialsFile {
  mcpOAuth?: Record<string, ClaudeCodeEntry>
  [key: string]: unknown
}

const CREDS_PATH = path.join(os.homedir(), '.claude', '.credentials.json')

function readCreds(): CredentialsFile {
  try {
    const raw = fs.readFileSync(CREDS_PATH, 'utf-8')
    const parsed = JSON.parse(raw)
    if (parsed && typeof parsed === 'object') return parsed as CredentialsFile
  } catch { /* missing or malformed — treat as empty */ }
  return {}
}

function writeCreds(data: CredentialsFile): void {
  fs.mkdirSync(path.dirname(CREDS_PATH), { recursive: true })
  fs.writeFileSync(CREDS_PATH, JSON.stringify(data, null, 2), 'utf-8')
  // Best-effort chmod 600 — silently ignored on Windows where the ACL model differs.
  try { fs.chmodSync(CREDS_PATH, 0o600) } catch { /* noop */ }
}

function hashUrl(url: string): string {
  return crypto.createHash('sha256').update(url).digest('hex').slice(0, 16)
}

function normalizeUrl(url: string): string {
  return url.replace(/\/$/, '')
}

/** Prefer the key an existing entry uses (so we don't collide with Claude
 *  Code's hash algorithm). Falls back to our own hash when no match. */
function findKeyByServerUrl(map: Record<string, ClaudeCodeEntry>, serverUrl: string): string | null {
  const target = normalizeUrl(serverUrl)
  for (const [k, v] of Object.entries(map)) {
    const u = typeof v?.serverUrl === 'string' ? normalizeUrl(v.serverUrl) : ''
    if (u === target) return k
  }
  return null
}

export function saveTokens(ctx: TokenContext, tokens: StoredTokens): void {
  const creds = readCreds()
  if (!creds.mcpOAuth) creds.mcpOAuth = {}
  const mcpOAuth = creds.mcpOAuth as Record<string, ClaudeCodeEntry>
  const existingKey = findKeyByServerUrl(mcpOAuth, ctx.serverUrl)
  const key = existingKey ?? `${ctx.serverName}|${hashUrl(ctx.serverUrl)}`
  const prev = mcpOAuth[key] ?? {}

  const entry: ClaudeCodeEntry = {
    serverName: ctx.serverName,
    serverUrl: ctx.serverUrl,
    accessToken: tokens.access_token,
  }
  if (tokens.refresh_token !== undefined) entry.refreshToken = tokens.refresh_token
  if (tokens.expires_at !== undefined) entry.expiresAt = tokens.expires_at
  // Preserve existing clientId/secret if the incoming tokens object doesn't carry them.
  entry.clientId = tokens.clientId ?? prev.clientId
  entry.clientSecret = tokens.clientSecret ?? prev.clientSecret
  entry.discoveryState = {
    ...(prev.discoveryState ?? {}),
    ...(ctx.authServerUrl ? { authorizationServerUrl: ctx.authServerUrl } : {}),
  }

  mcpOAuth[key] = entry
  writeCreds(creds)
}

export function loadTokens(serverUrl: string): StoredTokens | null {
  const creds = readCreds()
  if (!creds.mcpOAuth) return null
  const map = creds.mcpOAuth as Record<string, ClaudeCodeEntry>
  const key = findKeyByServerUrl(map, serverUrl)
  if (!key) return null
  const e = map[key]
  if (!e?.accessToken || typeof e.accessToken !== 'string') return null
  const out: StoredTokens = { access_token: e.accessToken }
  if (e.refreshToken) out.refresh_token = e.refreshToken
  if (typeof e.expiresAt === 'number') out.expires_at = e.expiresAt
  if (e.clientId) out.clientId = e.clientId
  if (e.clientSecret) out.clientSecret = e.clientSecret
  if (e.discoveryState?.authorizationServerUrl) out.authServerUrl = e.discoveryState.authorizationServerUrl
  return out
}

export function deleteTokens(serverUrl: string): void {
  const creds = readCreds()
  if (!creds.mcpOAuth) return
  const map = creds.mcpOAuth as Record<string, ClaudeCodeEntry>
  const key = findKeyByServerUrl(map, serverUrl)
  if (!key) return
  delete map[key]
  writeCreds(creds)
}

export function hasTokens(serverUrl: string): boolean {
  return loadTokens(serverUrl) !== null
}

/** One-shot migration from the legacy `<userData>/mcp-oauth-tokens.json`
 *  encrypted store into the shared Claude Code credentials file. Reads each
 *  entry, decrypts via `safeStorage`, and rewrites into `~/.claude/.credentials.json`.
 *  Safe to call repeatedly — if the source file is absent, nothing happens. */
export function migrateLegacyStore(resolver: (tokens: StoredTokens) => TokenContext | null): number {
  const legacyPath = path.join(app.getPath('userData'), 'mcp-oauth-tokens.json')
  let raw: string
  try { raw = fs.readFileSync(legacyPath, 'utf-8') } catch { return 0 }
  let parsed: Record<string, string>
  try { parsed = JSON.parse(raw) } catch { return 0 }
  if (!parsed || typeof parsed !== 'object') return 0

  let moved = 0
  for (const [, blob] of Object.entries(parsed)) {
    if (typeof blob !== 'string') continue
    let plain: string | null = null
    if (blob.startsWith('enc:') && safeStorage.isEncryptionAvailable()) {
      try { plain = safeStorage.decryptString(Buffer.from(blob.slice(4), 'base64')) } catch { /* skip */ }
    } else if (blob.startsWith('b64:')) {
      try { plain = Buffer.from(blob.slice(4), 'base64').toString('utf-8') } catch { /* skip */ }
    }
    if (!plain) continue
    let tokens: StoredTokens
    try { tokens = JSON.parse(plain) as StoredTokens } catch { continue }
    // The legacy file is keyed by the in-memory serverId which changes every
    // run. Resolve via the decoded token body instead (authServerUrl match).
    const ctx = resolver(tokens)
    if (!ctx) continue
    saveTokens(ctx, tokens)
    moved++
  }

  // Rename the old file so we don't re-run migration on every startup.
  try { fs.renameSync(legacyPath, legacyPath + '.migrated') } catch { /* noop */ }
  return moved
}
