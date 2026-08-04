// Import OAuth credentials that Claude Code has already negotiated with an
// MCP server. For servers like Figma that gate Dynamic Client Registration
// behind signed attestations, we can't obtain a clientId/secret ourselves —
// but if the user has run Claude Code on the same machine, its
// `~/.claude/.credentials.json` contains a valid registration we can reuse.
//
// Layout (per user's own notes + observed format):
//   {
//     "mcpOAuth": {
//       "figma|<hashOfUrl>": {
//         "serverName": "figma",
//         "serverUrl": "https://mcp.figma.com/mcp",
//         "accessToken": "figu_…",
//         "refreshToken": "figur_…",
//         "expiresAt": 1784477677168,
//         "clientId": "…",
//         "clientSecret": "…",
//         "discoveryState": { "authorizationServerUrl": "https://api.figma.com", … }
//       }
//     }
//   }

import fs from 'fs'
import path from 'path'
import os from 'os'

export interface ClaudeCodeMcpOAuthEntry {
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

function readCredentialsFile(): Record<string, ClaudeCodeMcpOAuthEntry> | null {
  const file = path.join(os.homedir(), '.claude', '.credentials.json')
  let raw: string
  try { raw = fs.readFileSync(file, 'utf-8') } catch { return null }
  try {
    const parsed = JSON.parse(raw)
    const map = parsed?.mcpOAuth
    if (map && typeof map === 'object') return map as Record<string, ClaudeCodeMcpOAuthEntry>
    return null
  } catch {
    return null
  }
}

/** Find an entry by the MCP server's URL. Claude Code keys entries as
 *  `<serverName>|<hash>`, so we match on the entry body's `serverUrl`
 *  rather than parsing the key. */
export function findClaudeCodeEntryByServerUrl(serverUrl: string): ClaudeCodeMcpOAuthEntry | null {
  const map = readCredentialsFile()
  if (!map) return null
  const target = serverUrl.replace(/\/$/, '')
  for (const entry of Object.values(map)) {
    if (!entry || typeof entry !== 'object') continue
    const url = typeof entry.serverUrl === 'string' ? entry.serverUrl.replace(/\/$/, '') : ''
    if (url && url === target) return entry
  }
  return null
}
