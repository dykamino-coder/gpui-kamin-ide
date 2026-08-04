// IPC handlers: plugin content discovery (preview + installed list).
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import { countPluginContents, countPluginLspServers, listPluginContents } from './content-scan'

export function registerContentHandlers(): void {
  ipcMain.handle('plugins:get-contents', (_event: IpcMainInvokeEvent, pluginDir: string) => {
    if (typeof pluginDir !== 'string' || !pluginDir) return null
    return listPluginContents(pluginDir)
  })

  ipcMain.handle('plugins:list-installed', () => {
    const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    const enabledMap = new Map<string, boolean>()
    try {
      const settings = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'settings.json'), 'utf-8'))
      const ep = settings?.enabledPlugins
      if (ep && typeof ep === 'object') {
        for (const [k, v] of Object.entries(ep)) {
          if (typeof v === 'boolean') enabledMap.set(k, v)
        }
      }
    } catch {}
    try {
      const data = JSON.parse(fs.readFileSync(pluginsFile, 'utf-8'))
      if (data.version === 2 && data.plugins) {
        const installedKeys = new Set<string>(Object.keys(data.plugins))
        const installedBareNames = new Set<string>(
          Object.keys(data.plugins).map(k => k.split('@')[0]!)
        )
        return Object.entries(data.plugins).map(([key, entries]: [string, any]) => {
          const [name, marketplace] = key.split('@')
          const entry = entries[0]
          let description = ''
          let rawDeps: unknown = null
          if (entry?.installPath) {
            try {
              const pj = path.join(entry.installPath, '.claude-plugin', 'plugin.json')
              if (fs.existsSync(pj)) {
                const manifest = JSON.parse(fs.readFileSync(pj, 'utf-8'))
                description = manifest.description || ''
                rawDeps = manifest.dependencies
              }
            } catch {}
            if (!description && marketplace) {
              try {
                const mktJson = path.join(os.homedir(), '.claude', 'plugins', 'marketplaces', marketplace, '.claude-plugin', 'marketplace.json')
                if (fs.existsSync(mktJson)) {
                  const mkt = JSON.parse(fs.readFileSync(mktJson, 'utf-8'))
                  const found = mkt.plugins?.find((p: any) => p.name === name)
                  if (found) description = found.description || ''
                }
              } catch {}
            }
          }
          const installPath = entry?.installPath || ''
          const isCached = installPath ? fs.existsSync(installPath) : false
          let contentDir = isCached ? installPath : ''
          if (!contentDir && marketplace) {
            const mktRoot = path.join(os.homedir(), '.claude', 'plugins', 'marketplaces', marketplace)
            for (const candidate of [
              path.join(mktRoot, 'plugins', name),
              path.join(mktRoot, 'plugins', 'mcp', name),
              path.join(mktRoot, 'external_plugins', name),
            ]) {
              if (fs.existsSync(candidate)) { contentDir = candidate; break }
            }
          }
          const counts = countPluginContents(contentDir)
          counts.lspServers = countPluginLspServers(name, marketplace || '', contentDir)

          const depList: Array<{ name: string; requested: string; installed: boolean; enabled: boolean }> = []
          if (Array.isArray(rawDeps)) {
            for (const rawDep of rawDeps) {
              const depRaw = typeof rawDep === 'string' ? rawDep
                : (rawDep && typeof rawDep === 'object' && typeof (rawDep as any).name === 'string')
                  ? String((rawDep as any).name) : null
              if (!depRaw) continue
              const qualifiedForm = depRaw.includes('@')
                ? depRaw
                : (marketplace ? `${depRaw}@${marketplace}` : depRaw)
              const bareForm = depRaw.split('@')[0]!
              const installed = installedKeys.has(qualifiedForm) || installedBareNames.has(bareForm)
              const enabled = installed
                && enabledMap.get(qualifiedForm) !== false
                && (!installedKeys.has(qualifiedForm) ? enabledMap.get(bareForm) !== false : true)
              depList.push({ name: bareForm, requested: depRaw, installed, enabled })
            }
          }

          return {
            name,
            marketplace: marketplace || '',
            version: entry?.version || 'unknown',
            scope: entry?.scope || 'user',
            installPath,
            installedAt: entry?.installedAt || '',
            description,
            isCached,
            counts,
            dependencies: depList,
          }
        })
      }
      return []
    } catch { return [] }
  })
}
