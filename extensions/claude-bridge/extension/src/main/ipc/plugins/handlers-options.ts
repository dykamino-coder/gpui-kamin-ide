// IPC handlers: plugin userConfig schema + values (read/write).
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import { readPluginManifest } from '../../plugin-helpers'

export function registerOptionsHandlers(): void {
  ipcMain.handle('plugins:get-options-schema', async (_event: IpcMainInvokeEvent, pluginId: string) => {
    if (typeof pluginId !== 'string' || !pluginId.includes('@')) {
      return { schema: {}, values: {}, sensitiveKeys: [] }
    }
    const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    let installPath = ''
    try {
      const data = JSON.parse(fs.readFileSync(pluginsFile, 'utf-8'))
      installPath = data?.plugins?.[pluginId]?.[0]?.installPath || ''
    } catch {}
    if (!installPath) return { schema: {}, values: {}, sensitiveKeys: [] }
    const manifest = await readPluginManifest(installPath)
    const schema = (manifest?.userConfig && typeof manifest.userConfig === 'object') ? manifest.userConfig : {}

    const nonSensitive: Record<string, unknown> = {}
    try {
      const settings = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'settings.json'), 'utf-8'))
      const opts = settings?.pluginConfigs?.[pluginId]?.options
      if (opts && typeof opts === 'object') Object.assign(nonSensitive, opts)
    } catch {}
    const sensitiveKeys: string[] = []
    try {
      const creds = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', '.credentials.json'), 'utf-8'))
      const secrets = creds?.pluginSecrets?.[pluginId]
      if (secrets && typeof secrets === 'object') {
        for (const k of Object.keys(secrets)) sensitiveKeys.push(k)
      }
    } catch {}
    for (const [k, s] of Object.entries(schema)) {
      if (s && typeof s === 'object' && (s as any).sensitive === true && !sensitiveKeys.includes(k)) {
        sensitiveKeys.push(k)
      }
    }

    const values: Record<string, unknown> = { ...nonSensitive }
    for (const k of sensitiveKeys) {
      if (k in values) delete values[k]
    }
    return { schema, values, sensitiveKeys }
  })

  ipcMain.handle('plugins:save-options', (_event: IpcMainInvokeEvent, pluginId: string, values: Record<string, unknown>) => {
    if (typeof pluginId !== 'string' || !pluginId.includes('@')) {
      throw new Error('Invalid pluginId')
    }
    if (!values || typeof values !== 'object') throw new Error('Invalid values')

    const pluginsFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    let installPath = ''
    try {
      const data = JSON.parse(fs.readFileSync(pluginsFile, 'utf-8'))
      installPath = data?.plugins?.[pluginId]?.[0]?.installPath || ''
    } catch {}
    let schema: Record<string, any> = {}
    if (installPath) {
      try {
        const pj = path.join(installPath, '.claude-plugin', 'plugin.json')
        if (fs.existsSync(pj)) {
          const m = JSON.parse(fs.readFileSync(pj, 'utf-8'))
          if (m?.userConfig && typeof m.userConfig === 'object') schema = m.userConfig
        }
      } catch {}
    }

    const sensitive: Record<string, unknown> = {}
    const nonSensitive: Record<string, unknown> = {}
    for (const [k, v] of Object.entries(values)) {
      if (v === null) continue
      const isSensitive = schema[k]?.sensitive === true
      if (isSensitive) sensitive[k] = v
      else nonSensitive[k] = v
    }

    const settingsPath = path.join(os.homedir(), '.claude', 'settings.json')
    let settings: any = {}
    if (fs.existsSync(settingsPath)) {
      try { settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8')) } catch {}
    }
    if (!settings.pluginConfigs) settings.pluginConfigs = {}
    if (!settings.pluginConfigs[pluginId]) settings.pluginConfigs[pluginId] = {}
    settings.pluginConfigs[pluginId].options = {
      ...(settings.pluginConfigs[pluginId].options || {}),
      ...nonSensitive,
    }
    fs.mkdirSync(path.dirname(settingsPath), { recursive: true })
    fs.writeFileSync(settingsPath, JSON.stringify(settings, null, 2), 'utf-8')

    if (Object.keys(sensitive).length > 0) {
      const credsPath = path.join(os.homedir(), '.claude', '.credentials.json')
      let creds: any = {}
      if (fs.existsSync(credsPath)) {
        try { creds = JSON.parse(fs.readFileSync(credsPath, 'utf-8')) } catch {}
      }
      if (!creds.pluginSecrets) creds.pluginSecrets = {}
      creds.pluginSecrets[pluginId] = {
        ...(creds.pluginSecrets[pluginId] || {}),
        ...sensitive,
      }
      fs.mkdirSync(path.dirname(credsPath), { recursive: true })
      fs.writeFileSync(credsPath, JSON.stringify(creds, null, 2), 'utf-8')
      try { fs.chmodSync(credsPath, 0o600) } catch {}
    }

    return { ok: true }
  })
}
