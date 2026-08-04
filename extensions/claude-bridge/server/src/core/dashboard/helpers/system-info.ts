// ============================================================================
// System info helpers for dashboard endpoints
// ============================================================================

import * as os from 'os'
import fs from 'fs'
import path from 'path'

/** Read Claude Code local files for account/token info */
export function readClaudeLocalInfo(): { email?: string; plan?: string; expiresAt?: number; displayName?: string } {
  const home = process.env.HOME || os.homedir()
  const result: { email?: string; plan?: string; expiresAt?: number; displayName?: string } = {}

  // .claude.json -- has oauthAccount with email
  try {
    const cj = JSON.parse(fs.readFileSync(path.join(home, '.claude.json'), 'utf8'))
    if (cj.oauthAccount?.emailAddress) result.email = cj.oauthAccount.emailAddress
    if (cj.oauthAccount?.displayName) result.displayName = cj.oauthAccount.displayName
  } catch {}

  // .claude/.credentials.json -- has expiresAt, subscriptionType
  try {
    const cred = JSON.parse(fs.readFileSync(path.join(home, '.claude', '.credentials.json'), 'utf8'))
    const oauth = cred.claudeAiOauth
    if (oauth?.expiresAt) result.expiresAt = oauth.expiresAt
    if (oauth?.subscriptionType) result.plan = oauth.subscriptionType
  } catch {}

  return result
}

export function getSDKVersion(): string {
  return 'n/a (PTY mode)'
}
