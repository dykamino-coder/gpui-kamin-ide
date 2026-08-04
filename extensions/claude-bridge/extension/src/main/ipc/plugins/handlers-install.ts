// IPC handlers: install / uninstall / install-from-local-folder.
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import crypto from 'crypto'

export function registerInstallHandlers(reloadMcp: () => Promise<void>): void {
  ipcMain.handle('plugins:install', async (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string, pluginPath: string) => {
    // Guard: reject if source looks like a marketplace, not a plugin.
    // Criterion: has `.claude-plugin/marketplace.json` but no `plugin.json`.
    // Previously this mis-install silently broke — Claude Code CLI ignores such
    // entries (no commands/agents/skills at root), leaving a phantom plugin
    // card in the UI. Stop it at the source.
    try {
      const hasPluginJson = fs.existsSync(path.join(pluginPath, '.claude-plugin', 'plugin.json'))
      const hasMarketplaceJson = fs.existsSync(path.join(pluginPath, '.claude-plugin', 'marketplace.json'))
      if (!hasPluginJson && hasMarketplaceJson) {
        throw new Error(
          `"${pluginName}" is a marketplace, not a plugin. ` +
          `Use "Add Marketplace" instead, then install individual sub-plugins from it.`
        )
      }
    } catch (err: any) {
      if (err instanceof Error && err.message.includes('is a marketplace')) throw err
      // Any other FS error — fall through to normal install path (best-effort).
    }

    const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    let data: any = { version: 2, plugins: {} }
    if (fs.existsSync(installedFile)) {
      try { data = JSON.parse(fs.readFileSync(installedFile, 'utf-8')) } catch {}
      if (!data.version) data = { version: 2, plugins: data.plugins || {} }
    }

    let version = '1.0.0'
    try {
      const pj = path.join(pluginPath, '.claude-plugin', 'plugin.json')
      if (fs.existsSync(pj)) {
        const manifest = JSON.parse(fs.readFileSync(pj, 'utf-8'))
        if (manifest.version) version = manifest.version
      } else {
        const knownFile = path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
        if (fs.existsSync(knownFile)) {
          const known = JSON.parse(fs.readFileSync(knownFile, 'utf-8'))
          const mktInfo = known[marketplace]
          if (mktInfo?.installLocation) {
            const mktJsonPath = path.join(mktInfo.installLocation, '.claude-plugin', 'marketplace.json')
            if (fs.existsSync(mktJsonPath)) {
              const mktData = JSON.parse(fs.readFileSync(mktJsonPath, 'utf-8'))
              const found = mktData.plugins?.find((p: any) => p.name === pluginName)
              if (found?.version) version = found.version
            }
          }
        }
      }
    } catch {}

    const cacheDir = path.join(os.homedir(), '.claude', 'plugins', 'cache', marketplace, pluginName, version)
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
        }
      }
    }
    copyDir(pluginPath, cacheDir)

    const key = `${pluginName}@${marketplace}`
    const now = new Date().toISOString()
    data.plugins[key] = [{
      scope: 'user',
      installPath: cacheDir,
      version,
      installedAt: now,
      lastUpdated: now,
    }]

    fs.mkdirSync(path.dirname(installedFile), { recursive: true })
    fs.writeFileSync(installedFile, JSON.stringify(data, null, 2), 'utf-8')

    // Plugin-hook approval flow: if the freshly installed plugin declares
    // hooks (in plugin.json or hooks.json), require explicit user consent
    // before they can fire. Compute a sha256 hash per hook and store the
    // approved set; on re-install the same hashes auto-approve, anything
    // changed re-prompts.
    try {
      const candidates = [
        path.join(cacheDir, '.claude-plugin', 'plugin.json'),
        path.join(cacheDir, 'hooks.json'),
        path.join(cacheDir, '.claude-plugin', 'hooks.json'),
      ]
      let hooks: any = null
      for (const cand of candidates) {
        if (fs.existsSync(cand)) {
          try { hooks = JSON.parse(fs.readFileSync(cand, 'utf-8')).hooks; if (hooks) break } catch { /* skip */ }
        }
      }
      if (hooks && Object.keys(hooks).length > 0) {
        const flat: Array<{ event: string; matcher?: string; handler: any; hash: string }> = []
        for (const [event, matchers] of Object.entries(hooks)) {
          if (!Array.isArray(matchers)) continue
          for (const m of matchers as Array<{ matcher?: string; hooks: any[] }>) {
            for (const h of m.hooks ?? []) {
              const json = JSON.stringify({ event, matcher: m.matcher, handler: h })
              const hash = crypto.createHash('sha256').update(json).digest('hex').slice(0, 16)
              flat.push({ event, matcher: m.matcher, handler: h, hash })
            }
          }
        }

        const win = BrowserWindow.getAllWindows().find(w => !w.isDestroyed()) ?? null
        if (win && flat.length > 0) {
          win.webContents.send('plugin-hooks:awaiting-approval', { pluginId: key, hooks: flat })
        }
      }
    } catch { /* approval flow is best-effort */ }

    // Pick up plugin-declared MCP servers immediately without restart.
    await reloadMcp()
    // Fire bridge-emit event so user-registered hooks can react to the install.
    try {
      const { emitBridgeHookEvent } = await import('../../hooks/emit-bridge-event')
      emitBridgeHookEvent('PluginInstalled', { pluginId: key, version, installPath: cacheDir })
    } catch { /* ignore */ }
  })

  ipcMain.handle('plugins:install-local', async () => {
    const { dialog } = await import('electron')
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory'],
      title: 'Select plugin folder',
    })
    if (result.canceled || result.filePaths.length === 0) return null

    const pluginDir = result.filePaths[0]
    const pluginName = path.basename(pluginDir)
    const marketplace = 'local'
    let version = '1.0.0'

    try {
      const pj = path.join(pluginDir, '.claude-plugin', 'plugin.json')
      if (fs.existsSync(pj)) {
        const manifest = JSON.parse(fs.readFileSync(pj, 'utf-8'))
        if (manifest.version) version = manifest.version
      }
    } catch {}

    const cacheDir = path.join(os.homedir(), '.claude', 'plugins', 'cache', marketplace, pluginName, version)
    const parentDir = path.join(os.homedir(), '.claude', 'plugins', 'cache', marketplace, pluginName)
    if (fs.existsSync(parentDir)) {
      fs.rmSync(parentDir, { recursive: true, force: true })
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
        }
      }
    }
    copyDir(pluginDir, cacheDir)

    const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    let data: any = { version: 2, plugins: {} }
    if (fs.existsSync(installedFile)) {
      try { data = JSON.parse(fs.readFileSync(installedFile, 'utf-8')) } catch {}
      if (!data.version) data = { version: 2, plugins: data.plugins || {} }
    }
    const key = `${pluginName}@${marketplace}`
    const now = new Date().toISOString()
    data.plugins[key] = [{
      scope: 'user',
      installPath: cacheDir,
      version,
      installedAt: now,
      lastUpdated: now,
    }]
    fs.mkdirSync(path.dirname(installedFile), { recursive: true })
    fs.writeFileSync(installedFile, JSON.stringify(data, null, 2), 'utf-8')
    await reloadMcp()

    return { name: pluginName, marketplace, version, installPath: cacheDir }
  })

  ipcMain.handle('plugins:uninstall', async (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string) => {
    const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    if (!fs.existsSync(installedFile)) return

    try {
      const data = JSON.parse(fs.readFileSync(installedFile, 'utf-8'))
      const key = `${pluginName}@${marketplace}`
      if (data.plugins?.[key]) {
        const entry = data.plugins[key][0]
        if (entry?.installPath && fs.existsSync(entry.installPath)) {
          fs.rmSync(entry.installPath, { recursive: true, force: true })
        }
        delete data.plugins[key]
        fs.writeFileSync(installedFile, JSON.stringify(data, null, 2), 'utf-8')
      }
    } catch {}
    // Drop plugin-sourced MCP entries immediately after uninstall.
    await reloadMcp()
    try {
      const { emitBridgeHookEvent } = await import('../../hooks/emit-bridge-event')
      emitBridgeHookEvent('PluginUninstalled', { pluginId: `${pluginName}@${marketplace}` })
    } catch { /* ignore */ }
  })
}
