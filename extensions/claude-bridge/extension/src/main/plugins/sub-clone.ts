// ============================================================================
// Sub-clone update helpers for marketplace plugins whose `source` is a git URL.
// `<marketplaceBaseDir>/plugins/<pluginName>/` is populated by
// `resolvePluginSource` on first browse (shallow clone). These helpers handle
// the update side: `git pull --ff-only` and optional installed-cache refresh.
// ============================================================================

import fs from 'fs'
import path from 'path'
import os from 'os'
import { redactUrl } from '../marketplace/url-auth'
import { runGit } from '../lib/git-async'

export interface SubClonePullResult {
  pluginName: string
  ok: boolean
  changed?: boolean
  error?: string
  skipped?: 'no-git' | 'not-present'
}

/** Pull a single sub-clone. Silent if the plugin dir isn't a git checkout
 *  (legacy inline plugin, or plugin-as-directory source) — nothing to pull.
 *  Auth is already baked into `.git/config` from the initial clone.
 *  Async via spawn so the main-process event loop keeps responding. */
export async function pullSubClone(baseDir: string, pluginName: string): Promise<SubClonePullResult> {
  const pluginDir = path.join(baseDir, 'plugins', pluginName)
  if (!fs.existsSync(pluginDir)) {
    return { pluginName, ok: true, skipped: 'not-present' }
  }
  if (!fs.existsSync(path.join(pluginDir, '.git'))) {
    return { pluginName, ok: true, skipped: 'no-git' }
  }
  try {
    const { stdout } = await runGit(['pull', '--ff-only'], { cwd: pluginDir, timeoutMs: 60_000 })
    const changed = !/Already up to date\.?/i.test(stdout)
    return { pluginName, ok: true, changed }
  } catch (err: any) {
    const stderrRaw = typeof err?.stderr === 'string' ? err.stderr : ''
    return { pluginName, ok: false, error: redactUrl(stderrRaw).slice(0, 500) || (err instanceof Error ? err.message : String(err)) }
  }
}

/** Iterate every sub-clone in `<baseDir>/plugins/` and pull each. Used by
 *  marketplace refresh to keep git-sourced plugins current. Sequential to
 *  avoid hammering the remote, but yields between each clone so the main
 *  event loop stays responsive. */
export async function pullAllSubClones(baseDir: string): Promise<SubClonePullResult[]> {
  const pluginsDir = path.join(baseDir, 'plugins')
  if (!fs.existsSync(pluginsDir)) return []
  let entries: string[] = []
  try {
    entries = fs.readdirSync(pluginsDir, { withFileTypes: true })
      .filter(e => e.isDirectory())
      .map(e => e.name)
  } catch { return [] }
  const results: SubClonePullResult[] = []
  for (const name of entries) {
    results.push(await pullSubClone(baseDir, name))
  }
  return results
}

/** After a sub-clone pulls new commits, refresh the installed cache copy for
 *  that plugin (if it's installed) so the running session sees the new
 *  version on next session spawn. Mirrors `plugins:sync-cache`. */
export function syncPluginCacheFromSubClone(pluginName: string, marketplace: string, baseDir: string): { ok: boolean; version?: string; error?: string } {
  const pluginSourcePath = path.join(baseDir, 'plugins', pluginName)
  if (!fs.existsSync(pluginSourcePath)) {
    return { ok: false, error: 'plugin source not found' }
  }
  let version = '1.0.0'
  try {
    const pj = path.join(pluginSourcePath, '.claude-plugin', 'plugin.json')
    if (fs.existsSync(pj)) {
      const manifest = JSON.parse(fs.readFileSync(pj, 'utf-8'))
      if (manifest.version) version = manifest.version
    } else {
      const mktJsonPath = path.join(baseDir, '.claude-plugin', 'marketplace.json')
      if (fs.existsSync(mktJsonPath)) {
        const mktData = JSON.parse(fs.readFileSync(mktJsonPath, 'utf-8'))
        const found = mktData.plugins?.find((p: any) => p.name === pluginName)
        if (found?.version) version = found.version
      }
    }
  } catch { /* ignore */ }

  const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
  let data: any = { version: 2, plugins: {} }
  if (fs.existsSync(installedFile)) {
    try { data = JSON.parse(fs.readFileSync(installedFile, 'utf-8')) } catch {}
  }
  const key = `${pluginName}@${marketplace}`
  // Only refresh cache if the plugin is actually installed — otherwise
  // we'd create a phantom cache entry.
  if (!data.plugins?.[key]) {
    return { ok: true, version }
  }

  const pluginCacheParent = path.join(os.homedir(), '.claude', 'plugins', 'cache', marketplace, pluginName)
  const cacheDir = path.join(pluginCacheParent, version)
  try {
    if (fs.existsSync(pluginCacheParent)) {
      fs.rmSync(pluginCacheParent, { recursive: true, force: true })
    }
    fs.mkdirSync(cacheDir, { recursive: true })
    const skipDirs = new Set(['node_modules', '.git', '__pycache__', '.venv'])
    function copyDir(src: string, dest: string): void {
      fs.mkdirSync(dest, { recursive: true })
      for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
        if (skipDirs.has(entry.name)) continue
        const srcPath = path.join(src, entry.name)
        const destPath = path.join(dest, entry.name)
        if (entry.isDirectory()) copyDir(srcPath, destPath)
        else fs.copyFileSync(srcPath, destPath)
      }
    }
    copyDir(pluginSourcePath, cacheDir)
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) }
  }

  const now = new Date().toISOString()
  const existing = data.plugins[key]?.[0]
  data.plugins[key] = [{
    scope: existing?.scope || 'user',
    installPath: cacheDir,
    version,
    installedAt: existing?.installedAt || now,
    lastUpdated: now,
  }]
  try { fs.writeFileSync(installedFile, JSON.stringify(data, null, 2), 'utf-8') }
  catch (err) { return { ok: false, error: err instanceof Error ? err.message : String(err) } }
  return { ok: true, version }
}
