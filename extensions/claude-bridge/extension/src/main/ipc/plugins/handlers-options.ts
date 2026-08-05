// IPC handlers: plugin userConfig schema + values (read/write).
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import { readEffectivePluginManifest } from '../../plugin-helpers'

function writeJsonAtomic(filePath: string, value: unknown): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true })
  const tmp = `${filePath}.${process.pid}.tmp`
  fs.writeFileSync(tmp, JSON.stringify(value, null, 2), 'utf-8')
  fs.renameSync(tmp, filePath)
}

function validateOption(key: string, value: unknown, spec: Record<string, any>): void {
  if (value === undefined || value === null || value === '') return
  const type = spec.type
  if (type === 'string' && spec.multiple === true) {
    if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) {
      throw new Error(`${key} must be an array of strings`)
    }
    return
  }
  if (type === 'boolean' && typeof value !== 'boolean') throw new Error(`${key} must be a boolean`)
  if (type === 'number') {
    if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error(`${key} must be a finite number`)
    if (typeof spec.min === 'number' && value < spec.min) throw new Error(`${key} must be at least ${spec.min}`)
    if (typeof spec.max === 'number' && value > spec.max) throw new Error(`${key} must be at most ${spec.max}`)
  }
  if (['string', 'directory', 'file'].includes(type) && typeof value !== 'string') {
    throw new Error(`${key} must be a string`)
  }
  const allowed = Array.isArray(spec.enum) ? spec.enum : Array.isArray(spec.options) ? spec.options : null
  if (allowed && !allowed.some((candidate: unknown) => Object.is(candidate, value))) {
    throw new Error(`${key} must be one of: ${allowed.map(String).join(', ')}`)
  }
}

export function registerOptionsHandlers(reloadRuntime: () => Promise<void>): void {
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
    const at = pluginId.lastIndexOf('@')
    const manifest = await readEffectivePluginManifest(installPath, pluginId.slice(0, at), pluginId.slice(at + 1))
    const schema = (manifest?.userConfig && typeof manifest.userConfig === 'object') ? manifest.userConfig : {}

    const nonSensitive: Record<string, unknown> = {}
    try {
      const settings = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'settings.json'), 'utf-8'))
      const opts = settings?.pluginConfigs?.[pluginId]?.options
      if (opts && typeof opts === 'object') Object.assign(nonSensitive, opts)
    } catch {}
    const sensitiveKeys: string[] = []
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

  ipcMain.handle('plugins:save-options', async (_event: IpcMainInvokeEvent, pluginId: string, values: Record<string, unknown>) => {
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
      const at = pluginId.lastIndexOf('@')
      const manifest = await readEffectivePluginManifest(installPath, pluginId.slice(0, at), pluginId.slice(at + 1))
      if (manifest.userConfig && typeof manifest.userConfig === 'object') schema = manifest.userConfig
    }

    for (const key of Object.keys(values)) {
      if (!Object.prototype.hasOwnProperty.call(schema, key)) throw new Error(`Unknown plugin option: ${key}`)
      validateOption(key, values[key], schema[key] ?? {})
    }

    let existingOptions: Record<string, unknown> = {}
    let existingSecrets: Record<string, unknown> = {}
    try {
      existingOptions = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'settings.json'), 'utf-8'))
        ?.pluginConfigs?.[pluginId]?.options ?? {}
    } catch {}
    try {
      existingSecrets = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', '.credentials.json'), 'utf-8'))
        ?.pluginSecrets?.[pluginId] ?? {}
    } catch {}
    for (const [key, spec] of Object.entries(schema)) {
      if (!spec || typeof spec !== 'object' || spec.required !== true) continue
      const candidate = Object.prototype.hasOwnProperty.call(values, key)
        ? values[key]
        : existingSecrets[key] ?? existingOptions[key] ?? spec.default
      if (candidate === undefined || candidate === null || candidate === '' || (Array.isArray(candidate) && candidate.length === 0)) {
        throw new Error(`Required plugin option is missing: ${key}`)
      }
      validateOption(key, candidate, spec)
    }

    const settingsPath = path.join(os.homedir(), '.claude', 'settings.json')
    let settings: any = {}
    if (fs.existsSync(settingsPath)) {
      try { settings = JSON.parse(fs.readFileSync(settingsPath, 'utf-8')) } catch {}
    }
    if (!settings.pluginConfigs) settings.pluginConfigs = {}
    if (!settings.pluginConfigs[pluginId]) settings.pluginConfigs[pluginId] = {}
    const nextOptions: Record<string, unknown> = { ...(settings.pluginConfigs[pluginId].options || {}) }
    // Sensitive options belong exclusively in the credential store. Purge
    // plaintext shadows left by older versions even when this save does not
    // submit a replacement secret.
    for (const [key, spec] of Object.entries(schema)) {
      if (spec?.sensitive === true) delete nextOptions[key]
    }
    for (const [key, value] of Object.entries(values)) {
      if (schema[key]?.sensitive === true) continue
      if (value === null) delete nextOptions[key]
      else nextOptions[key] = value
    }
    settings.pluginConfigs[pluginId].options = nextOptions
    writeJsonAtomic(settingsPath, settings)

    const credsPath = path.join(os.homedir(), '.claude', '.credentials.json')
    let creds: any = {}
    if (fs.existsSync(credsPath)) {
      try { creds = JSON.parse(fs.readFileSync(credsPath, 'utf-8')) } catch {}
    }
    if (!creds.pluginSecrets) creds.pluginSecrets = {}
    const nextSecrets: Record<string, unknown> = { ...(creds.pluginSecrets[pluginId] || {}) }
    // Migrate a legacy plaintext shadow before purging it from settings. A
    // blank sensitive input is omitted by the UI and means "keep existing";
    // losing the only stored value during an unrelated option save would be a
    // destructive migration. An explicit null still means delete.
    for (const [key, spec] of Object.entries(schema)) {
      if (spec?.sensitive !== true) continue
      if (!Object.prototype.hasOwnProperty.call(values, key)
          && !Object.prototype.hasOwnProperty.call(nextSecrets, key)
          && Object.prototype.hasOwnProperty.call(existingOptions, key)) {
        nextSecrets[key] = existingOptions[key]
      }
    }
    // If a plugin changes an option from sensitive to non-sensitive, a stale
    // secret must not keep overriding the visible settings value forever.
    for (const [key, spec] of Object.entries(schema)) {
      if (spec?.sensitive !== true) delete nextSecrets[key]
    }
    for (const [key, value] of Object.entries(values)) {
      if (schema[key]?.sensitive !== true) continue
      if (value === null) delete nextSecrets[key]
      else nextSecrets[key] = value
    }
    if (Object.keys(nextSecrets).length > 0) creds.pluginSecrets[pluginId] = nextSecrets
    else delete creds.pluginSecrets[pluginId]
    writeJsonAtomic(credsPath, creds)
    try { fs.chmodSync(credsPath, 0o600) } catch {}

    await reloadRuntime()
    return { ok: true, restartRequired: true }
  })
}
