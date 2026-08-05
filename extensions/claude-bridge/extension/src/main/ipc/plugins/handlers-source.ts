// IPC handlers: plugin source path resolution + cache sync + sub-clone pull.
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import { resolvePluginSource } from './shared'
import { pullSubClone, syncPluginCacheFromSubClone } from '../../plugins/sub-clone'
import { enableRequiredPluginDependencies, mergeMarketplaceDependencies, requestHookApproval } from './handlers-install'
import { readPluginManifest } from '../../plugin-helpers'

export function registerSourceHandlers(reloadMcp: () => Promise<void>): void {
  ipcMain.handle('plugins:get-source-path', async (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string): Promise<string | null> => {
    const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
    if (!fs.existsSync(knownFile)) return null
    try {
      const known = JSON.parse(fs.readFileSync(knownFile, 'utf-8'))
      const mktInfo = known[marketplace]
      if (!mktInfo?.installLocation) return null
      const baseDir = mktInfo.installLocation
      const mktJsonPath = path.join(baseDir, '.claude-plugin', 'marketplace.json')
      if (fs.existsSync(mktJsonPath)) {
        const mktData = JSON.parse(fs.readFileSync(mktJsonPath, 'utf-8'))
        const found = mktData.plugins?.find((p: any) => p.name === pluginName)
        if (found?.source) {
          const resolved = await resolvePluginSource(found, marketplace, baseDir)
          if (resolved.dir && fs.existsSync(resolved.dir)) return resolved.dir
        }
      }
      const subDir = path.join(baseDir, pluginName)
      if (fs.existsSync(subDir)) return subDir
      return baseDir
    } catch { return null }
  })

  ipcMain.handle('plugins:sync-cache', async (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string) => {
    const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
    if (!fs.existsSync(knownFile)) throw new Error('No marketplaces configured')
    const known = JSON.parse(fs.readFileSync(knownFile, 'utf-8'))
    const mktInfo = known[marketplace]
    if (!mktInfo?.installLocation) throw new Error(`Marketplace "${marketplace}" not found`)
    const baseDir = mktInfo.installLocation

    let pluginSourcePath = ''
    let mktVersion = ''
    const mktJsonPath = path.join(baseDir, '.claude-plugin', 'marketplace.json')
    if (fs.existsSync(mktJsonPath)) {
      try {
        const mktData = JSON.parse(fs.readFileSync(mktJsonPath, 'utf-8'))
        if (Array.isArray(mktData.plugins)) {
          const found = mktData.plugins.find((p: any) => p.name === pluginName)
          if (found?.source) {
            const resolved = await resolvePluginSource(found, marketplace, baseDir)
            pluginSourcePath = resolved.dir || ''
          }
          if (found?.version) {
            mktVersion = found.version
          }
        }
      } catch {}
    }
    if (!pluginSourcePath || !fs.existsSync(pluginSourcePath)) {
      pluginSourcePath = path.join(baseDir, pluginName)
    }
    if (!fs.existsSync(pluginSourcePath)) {
      pluginSourcePath = baseDir
    }

    let version = '1.0.0'
    try {
      const pj = path.join(pluginSourcePath, '.claude-plugin', 'plugin.json')
      if (fs.existsSync(pj)) {
        const manifest = JSON.parse(fs.readFileSync(pj, 'utf-8'))
        if (manifest.version) version = manifest.version
      } else if (mktVersion) {
        version = mktVersion
      }
    } catch {
      if (mktVersion) version = mktVersion
    }

    const pluginCacheParent = path.join(os.homedir(), '.claude', 'plugins', 'cache', marketplace, pluginName)
    const cacheDir = path.join(pluginCacheParent, version)

    if (fs.existsSync(pluginCacheParent)) {
      fs.rmSync(pluginCacheParent, { recursive: true, force: true })
    }
    fs.mkdirSync(cacheDir, { recursive: true })

    const skipDirs = new Set(['node_modules', '.git', '__pycache__', '.venv'])
    function copyDir(src: string, dest: string) {
      fs.mkdirSync(dest, { recursive: true })
      const entries = fs.readdirSync(src, { withFileTypes: true })
      for (const entry of entries) {
        if (skipDirs.has(entry.name)) continue
        const srcPath = path.join(src, entry.name)
        const destPath = path.join(dest, entry.name)
        if (entry.isDirectory()) {
          copyDir(srcPath, destPath)
        } else {
          fs.copyFileSync(srcPath, destPath)
          try { fs.chmodSync(destPath, fs.statSync(srcPath).mode) } catch { /* best effort */ }
        }
      }
    }
    copyDir(pluginSourcePath, cacheDir)

    const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    let data: any = { version: 2, plugins: {} }
    if (fs.existsSync(installedFile)) {
      try { data = JSON.parse(fs.readFileSync(installedFile, 'utf-8')) } catch {}
    }
    const key = `${pluginName}@${marketplace}`
    const dependencyManifest = mergeMarketplaceDependencies(pluginName, marketplace, await readPluginManifest(pluginSourcePath) ?? {})
    const now = new Date().toISOString()
    const existing = data.plugins[key]?.[0]
    data.plugins[key] = [{
      scope: existing?.scope || 'user',
      installPath: cacheDir,
      version,
      installedAt: existing?.installedAt || now,
      lastUpdated: now,
    }]
    fs.writeFileSync(installedFile, JSON.stringify(data, null, 2), 'utf-8')
    await enableRequiredPluginDependencies(key, dependencyManifest)
    await requestHookApproval(key, cacheDir)
    await reloadMcp()

    return { version, cacheDir }
  })

  // Force-retry a failed sub-clone: remove the stale dir (if any) so the
  // next browse does a fresh clone. Used by the error-card Retry button.
  ipcMain.handle('plugins:retry-plugin-source', (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string) => {
    try {
      const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
      if (!fs.existsSync(knownFile)) return { ok: false, error: 'No marketplaces configured' }
      const known = JSON.parse(fs.readFileSync(knownFile, 'utf-8'))
      const mktInfo = known[marketplace]
      if (!mktInfo?.installLocation) return { ok: false, error: `Marketplace "${marketplace}" not found` }
      const targetDir = path.join(mktInfo.installLocation, 'plugins', pluginName)
      if (fs.existsSync(targetDir)) fs.rmSync(targetDir, { recursive: true, force: true })
      return { ok: true }
    } catch (err: any) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // Pull a git-sourced plugin's sub-clone and refresh its installed cache.
  // UI "Update" button on the plugin card invokes this. Returns:
  //   { ok, changed?, version?, error? }
  ipcMain.handle('plugins:refresh-plugin-source', async (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string) => {
    const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
    if (!fs.existsSync(knownFile)) return { ok: false, error: 'No marketplaces configured' }
    let known: any
    try { known = JSON.parse(fs.readFileSync(knownFile, 'utf-8')) }
    catch { return { ok: false, error: 'known_marketplaces.json malformed' } }
    const mktInfo = known[marketplace]
    if (!mktInfo?.installLocation) return { ok: false, error: `Marketplace "${marketplace}" not found` }
    const baseDir: string = mktInfo.installLocation

    const pullResult = await pullSubClone(baseDir, pluginName)
    if (!pullResult.ok) {
      return { ok: false, error: pullResult.error || 'git pull failed' }
    }
    if (pullResult.skipped) {
      return { ok: true, changed: false, skipped: pullResult.skipped, note: 'Plugin source is not a git checkout — nothing to pull' }
    }
    const syncRes = syncPluginCacheFromSubClone(pluginName, marketplace, baseDir)
    if (!syncRes.ok) {
      return { ok: false, changed: pullResult.changed, error: `Pull OK but cache sync failed: ${syncRes.error}` }
    }
    if (pullResult.changed) {
      const key = `${pluginName}@${marketplace}`
      let refreshedRoot = ''
      try {
        refreshedRoot = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json'), 'utf-8'))
          ?.plugins?.[key]?.[0]?.installPath ?? ''
      } catch {}
      if (refreshedRoot) {
        await enableRequiredPluginDependencies(key, mergeMarketplaceDependencies(pluginName, marketplace, await readPluginManifest(refreshedRoot) ?? {}))
        await requestHookApproval(key, refreshedRoot)
      }
      // Publish the refreshed runtime only after dependency validation and
      // hook approval metadata have both caught up with the new cache.
      await reloadMcp()
    }
    return { ok: true, changed: pullResult.changed, version: syncRes.version }
  })
}
