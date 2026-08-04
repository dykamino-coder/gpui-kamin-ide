// Shared helpers + types used across the plugins/* IPC modules.
// Extracted from the original `electron/main/ipc/plugins.ts` god-file (1449 LOC)
// as part of Sprint 2 / Stage C — split god-files (C2).

import path from 'path'
import fs from 'fs'
import os from 'os'
import { redactUrl } from '../../marketplace/url-auth'
import { runGit } from '../../lib/git-async'

/** Context passed to registerPluginsIPC — provides side-effect entry points
 *  the handlers need (e.g. notifying MCP about plugin enable/disable). */
export interface PluginsIPCContext {
  /** Trigger an MCP re-discovery so plugin-sourced servers become live after
   *  install / sync / uninstall / refresh. Returns a promise when supplied. */
  reloadMcpFromPlugins?: () => Promise<void> | void
}

/** Result of `resolvePluginSource` — either a directory we can read from or
 *  a structured error so the UI can surface a redacted git failure on the
 *  plugin card without leaking credentials. */
export interface ResolveResult {
  dir: string | null
  /** Non-null when dir is null and we have a meaningful reason (clone failed,
   *  source unsupported, etc). UI shows this on an error card. */
  error?: string
  /** Remote URL that was being cloned (redacted) — shown in the error card so
   *  the user knows which repo failed. */
  url?: string
  ref?: string
}

/** When a marketplace clone uses HTTP-Basic creds (oauth2 token in URL),
 *  the same creds usually authorize the plugin sub-repos on the same host —
 *  propagate the token so clones succeed without the user re-entering it. */
export function propagateMarketplaceAuth(pluginUrl: string, marketplaceName: string): string {
  try {
    const plug = new URL(pluginUrl)
    if (plug.protocol !== 'http:' && plug.protocol !== 'https:') return pluginUrl
    const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
    if (!fs.existsSync(knownFile)) return pluginUrl
    const known = JSON.parse(fs.readFileSync(knownFile, 'utf-8'))
    const entry = known[marketplaceName]
    const mktUrl: string = entry?.source?.url || ''
    if (!mktUrl) return pluginUrl
    const mkt = new URL(mktUrl)
    if (mkt.host !== plug.host) return pluginUrl
    if (!mkt.username && !mkt.password) return pluginUrl
    if (plug.username || plug.password) return pluginUrl // don't clobber explicit creds
    plug.username = mkt.username
    plug.password = mkt.password
    return plug.toString()
  } catch { return pluginUrl }
}

/** Resolve a plugin's local directory. Handles three marketplace.json `source`
 *  shapes: legacy string path, `{source:'directory',path}`, and
 *  `{source:'url'|'git', url, ref?}` (lazy shallow-clone). Returns
 *  { dir, error?, url? }. Async because git invocations yield back to the
 *  event loop between chunks — execFileSync stalled the main process message
 *  pump for 10s+ per clone and flagged the window "Not Responding" on Win. */
export async function resolvePluginSource(p: any, marketplaceName: string, baseDir: string): Promise<ResolveResult> {
  // Legacy: `source` is a relative string path inside the marketplace repo.
  if (typeof p?.source === 'string') {
    const abs = path.resolve(baseDir, p.source)
    return fs.existsSync(abs) ? { dir: abs } : { dir: null, error: `Source path "${p.source}" not found inside marketplace` }
  }
  const src = p?.source
  if (!src || typeof src !== 'object') return fs.existsSync(baseDir) ? { dir: baseDir } : { dir: null, error: 'Plugin has no source field' }

  // Directory source — absolute local path.
  if (src.source === 'directory' && typeof src.path === 'string') {
    return fs.existsSync(src.path) ? { dir: src.path } : { dir: null, error: `Directory ${src.path} does not exist` }
  }

  // Git/URL source — shallow clone into `<baseDir>/plugins/<name>/` once.
  // Accepted forms: { source: 'url' | 'git' | 'github', url?|repo?, ref?|revision?|commit? }
  const isGitSource = src.source === 'url' || src.source === 'git' || src.source === 'github'
  const rawUrlSeed: string = src.source === 'github' && typeof src.repo === 'string'
    ? `https://github.com/${src.repo}.git`
    : (typeof src.url === 'string' ? src.url : '')
  if (isGitSource && rawUrlSeed) {
    const pluginName: string = p.name || 'unknown'
    const targetDir = path.join(baseDir, 'plugins', pluginName)
    // Already cloned sub-repo — reuse.
    if (fs.existsSync(path.join(targetDir, '.git'))) return { dir: targetDir }
    // Legacy inline plugin already shipped inside the marketplace repo at
    // the same path. Don't try to clone over it.
    if (fs.existsSync(targetDir)) return { dir: targetDir }
    const refRaw: string = (typeof src.ref === 'string' ? src.ref
      : typeof src.revision === 'string' ? src.revision
      : typeof src.commit === 'string' ? src.commit
      : '').trim()
    const ref: string | undefined = refRaw || undefined
    const isSha = !!ref && /^[0-9a-f]{7,40}$/i.test(ref)
    const authedUrl = propagateMarketplaceAuth(rawUrlSeed, marketplaceName)
    try { fs.mkdirSync(path.dirname(targetDir), { recursive: true }) } catch { /* ignore */ }
    const TIMEOUT = 120_000
    try {
      if (ref && !isSha) {
        // Branch / tag — shallow clone of that ref directly.
        await runGit(['clone', '--depth=1', '--branch', ref, '--', authedUrl, targetDir], { timeoutMs: TIMEOUT })
      } else if (isSha) {
        // Specific commit — --branch doesn't accept SHAs. Two-step: clone
        // shallow default branch, then fetch + checkout the SHA. Requires
        // server-side `uploadpack.allowReachableSHA1InWant` (enabled on GitHub
        // and modern GitLab).
        await runGit(['clone', '--no-checkout', '--filter=blob:none', '--', authedUrl, targetDir], { timeoutMs: TIMEOUT })
        await runGit(['fetch', '--depth=1', 'origin', ref!], { cwd: targetDir, timeoutMs: TIMEOUT })
        await runGit(['checkout', 'FETCH_HEAD'], { cwd: targetDir, timeoutMs: TIMEOUT })
      } else {
        // No ref — clone default branch (HEAD).
        await runGit(['clone', '--depth=1', '--', authedUrl, targetDir], { timeoutMs: TIMEOUT })
      }
      return { dir: targetDir }
    } catch (err: any) {
      const stderrRaw = typeof err?.stderr === 'string' ? err.stderr : ''
      const redactedErr = redactUrl(stderrRaw).slice(0, 300) || (err instanceof Error ? err.message : String(err))
      console.error(`[plugins] clone failed for ${pluginName} <${redactUrl(rawUrlSeed)}> ref=${ref || 'HEAD'}:`, redactedErr)
      // Clean up half-cloned directory so a retry doesn't trip over corrupt .git.
      try { fs.rmSync(targetDir, { recursive: true, force: true }) } catch { /* ignore */ }
      return { dir: null, error: redactedErr, url: redactUrl(rawUrlSeed), ref: ref }
    }
  }

  return fs.existsSync(baseDir) ? { dir: baseDir } : { dir: null, error: 'Unsupported source shape' }
}
