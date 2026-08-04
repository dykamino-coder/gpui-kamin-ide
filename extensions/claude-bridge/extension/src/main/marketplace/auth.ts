import fs from 'fs'
import path from 'path'
import { assertValidName } from '../validators'
import { redactUrl } from './url-auth'
import { readKnownMarketplaces, writeKnownMarketplaces, knownMarketplacesPath } from './known-store'
import { runGit } from '../lib/git-async'

export interface SetAuthResult {
  ok: boolean
  error?: string
  warning?: string
}

// Rewrite the stored URL for a marketplace to embed a fresh user/token,
// and sync the local clone's `origin` remote so subsequent refreshes hit
// the authenticated URL too. `auth.token` empty/missing → strip all
// credentials (anonymise). For marketplaces cloned as `github` shortform
// we convert to `git` with a full URL so the token has somewhere to live.
//
// Unlike the add-marketplace flow, we DON'T re-clone. That means an
// immediate refresh still uses the old `.git/config` origin until we
// run `git remote set-url`. We do that right before returning so the
// caller's next `refreshMarketplace` already hits the new URL.
export async function setMarketplaceAuth(
  name: string,
  auth: { username?: string; token?: string } | null,
): Promise<SetAuthResult> {
  assertValidName(name, 'marketplace name')
  if (!fs.existsSync(knownMarketplacesPath())) return { ok: false, error: 'known_marketplaces.json not found' }

  let known: any
  try { known = readKnownMarketplaces() }
  catch (err: any) { return { ok: false, error: `Could not parse known_marketplaces.json: ${err?.message || err}` } }

  const entry = known[name]
  if (!entry) return { ok: false, error: `Marketplace "${name}" not found` }

  let currentUrl = ''
  const source = entry.source || {}
  if (source.source === 'git' && typeof source.url === 'string') currentUrl = source.url
  else if (source.source === 'github' && typeof source.repo === 'string') currentUrl = `https://github.com/${source.repo}.git`
  else if (source.source === 'directory') return { ok: false, error: 'Directory-backed marketplaces do not use authentication' }
  else if (typeof source.url === 'string') currentUrl = source.url

  if (!currentUrl) return { ok: false, error: 'Could not determine source URL for this marketplace' }

  // Strip any existing credentials so we don't accumulate `user:***@user:new@host`
  // garbage from repeated edits.
  let newUrl: string
  try {
    const u = new URL(currentUrl)
    u.username = ''
    u.password = ''
    const token = auth?.token?.trim() ?? ''
    const username = auth?.username?.trim() || 'oauth2'
    if (token) {
      u.username = encodeURIComponent(username)
      u.password = encodeURIComponent(token)
    }
    newUrl = u.toString()
  } catch (err: any) {
    return { ok: false, error: `Malformed URL "${currentUrl}": ${err?.message || err}` }
  }

  // Rewrite source → always `git` with the authed URL. GitHub shortform
  // can't carry credentials; converting keeps the downstream clone/pull
  // logic uniform.
  known[name] = {
    ...entry,
    source: { source: 'git', url: newUrl },
    lastUpdated: new Date().toISOString(),
  }
  writeKnownMarketplaces(known)

  // Update the clone's own origin so `git -C <loc> pull` picks up the new
  // URL immediately. Wrapped in try so a missing clone (cache wiped by
  // user) doesn't mask the successful settings write.
  if (entry.installLocation && fs.existsSync(path.join(entry.installLocation, '.git'))) {
    try {
      await runGit(['remote', 'set-url', 'origin', newUrl], { cwd: entry.installLocation, timeoutMs: 15_000 })
    } catch (err: any) {
      const stderrRaw = typeof err?.stderr === 'string' ? err.stderr : ''
      return { ok: true, warning: `URL saved, but couldn't update local clone's origin: ${redactUrl(stderrRaw)}` }
    }
  }

  return { ok: true }
}
