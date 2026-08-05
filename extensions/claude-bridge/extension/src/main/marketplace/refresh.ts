import fs from 'fs'
import path from 'path'
import type { BrowserWindow } from '@kaminide/host-compat'
import { assertValidName, assertAbsolutePath } from '../validators'
import { redactUrl } from './url-auth'
import { knownMarketplacesPath, readKnownMarketplaces, writeKnownMarketplaces } from './known-store'
import { pullAllSubClones, syncPluginCacheFromSubClone } from '../plugins/sub-clone'
import { runGit } from '../lib/git-async'

export interface RefreshResult {
  ok: boolean
  lastUpdated?: string
  error?: string
  changed?: boolean
}

// Pull an existing marketplace checkout. Blocks stdin prompts and captures
// git stderr so the UI can surface auth failures (common on corporate
// GitLab). `source: directory` marketplaces need no update — the user
// edits files in place — so we no-op. Returns the refreshed metadata so
// the caller can update its lastUpdated display without re-listing all
// marketplaces.
export async function refreshMarketplaceOnce(name: string): Promise<RefreshResult> {
  if (!fs.existsSync(knownMarketplacesPath())) return { ok: false, error: 'known_marketplaces.json not found' }
  assertValidName(name, 'marketplace name')
  const known = readKnownMarketplaces()
  const entry = known[name] as any
  if (!entry?.installLocation) return { ok: false, error: `Marketplace "${name}" not found` }

  // Directory-backed marketplaces are live; skip.
  if (entry.source?.source === 'directory') {
    entry.lastUpdated = new Date().toISOString()
    writeKnownMarketplaces(known)
    return { ok: true, lastUpdated: entry.lastUpdated, changed: false }
  }

  const loc: string = entry.installLocation
  assertAbsolutePath(loc, 'installLocation')

  try {
    const { stdout: out } = await runGit(['pull', '--ff-only'], { cwd: loc, timeoutMs: 60_000 })
    const changed = !/Already up to date\.?/i.test(out)
    entry.lastUpdated = new Date().toISOString()
    writeKnownMarketplaces(known)
    // After the marketplace itself is up to date, pull every sub-cloned
    // plugin under `<loc>/plugins/<name>/` (git-sourced plugins populated
    // by `resolvePluginSource`). Best-effort — per-plugin failures are
    // logged but don't mark the marketplace refresh as failed. When a
    // sub-clone actually receives new commits and the plugin is installed,
    // the cached copy under `~/.claude/plugins/cache/...` is refreshed too.
    try {
      const subResults = await pullAllSubClones(loc)
      for (const r of subResults) {
        if (!r.ok) {
          console.warn(`[marketplaces] ${name}: sub-clone pull failed for "${r.pluginName}" — ${r.error}`)
          continue
        }
        if (r.changed) {
          const sync = syncPluginCacheFromSubClone(r.pluginName, name, loc)
          if (!sync.ok) {
            console.warn(`[marketplaces] ${name}: cache sync failed for "${r.pluginName}" — ${sync.error}`)
          }
        }
      }
    } catch (err) {
      console.warn(`[marketplaces] ${name}: sub-clone sweep threw —`, err instanceof Error ? err.message : err)
    }
    return { ok: true, lastUpdated: entry.lastUpdated, changed }
  } catch (err: any) {
    const stderrRaw = typeof err?.stderr === 'string' ? err.stderr : err?.stderr?.toString() || ''
    const stderr = redactUrl(stderrRaw).slice(0, 2000)
    let hint = ''
    if (/authentication failed|could not read (Username|Password)|unable to access/i.test(stderrRaw)) {
      hint = 'Authentication required. The stored URL probably lost its token — re-add the marketplace with a Personal Access Token.'
    } else if (/non-fast-forward|diverged/i.test(stderrRaw)) {
      hint = 'Local marketplace checkout diverged from remote. Remove and re-add the marketplace, or manually reset with git.'
    } else if (/terminal prompts disabled/i.test(stderrRaw)) {
      hint = 'Git needs credentials but none are available. Re-add the marketplace with a token.'
    }
    const message = [
      `git pull failed: ${err?.message || 'unknown error'}`,
      hint && `Hint: ${hint}`,
      stderr && `--- git stderr ---\n${stderr}`,
    ].filter(Boolean).join('\n')
    return { ok: false, error: message }
  }
}

// Refresh every marketplace whose `autoUpdate` flag is not explicitly
// false. Runs sequentially so we don't hammer the remote, but fire-and-
// forget from the caller's perspective — results are pushed to the UI via
// an `mcp-servers-changed`-style broadcast (`marketplaces:updated`) so
// the marketplace bar refreshes its chips when pulls finish.
export async function refreshAllMarketplaces(window?: BrowserWindow): Promise<void> {
  if (!fs.existsSync(knownMarketplacesPath())) return
  const known = readKnownMarketplaces() as any
  const names = Object.keys(known).filter(n => known[n]?.autoUpdate !== false)
  for (const name of names) {
    try { await refreshMarketplaceOnce(name) } catch { /* logged inside */ }
    if (window && !window.isDestroyed()) {
      window.webContents.send('marketplaces:updated', { name })
    }
    try {
      const { emitBridgeHookEvent } = await import('../hooks/emit-bridge-event')
      emitBridgeHookEvent('MarketplaceUpdated', { name })
    } catch { /* ignore */ }
  }
}
