// IPC handlers: install / uninstall / install-from-local-folder.
// Extracted from `electron/main/ipc/plugins.ts` (Sprint 2 / Stage C, C2).

import { ipcMain, BrowserWindow, type IpcMainInvokeEvent } from '@kaminide/host-compat'
import path from 'path'
import fs from 'fs'
import os from 'os'
import { collectPluginHooks, hashPluginHook } from '../../sync/plugin-snapshot'
import {
  assertUniqueEnabledPluginNames,
  listEnabledInstalledPlugins,
  readEffectivePluginManifest,
  readEffectivePluginManifestSync,
  readPluginManifest,
} from '../../plugin-helpers'
import semver from 'semver'
import crypto from 'crypto'
import { resolvePluginSource } from './shared'

const SETTINGS_FILE = path.join(os.homedir(), '.claude', 'settings.json')
const INSTALLED_FILE = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')

function writeJsonAtomic(filePath: string, value: unknown): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true })
  const tmp = `${filePath}.${process.pid}.tmp`
  fs.writeFileSync(tmp, JSON.stringify(value, null, 2), 'utf-8')
  fs.renameSync(tmp, filePath)
}

function readInstalledData(): any {
  try {
    const parsed = JSON.parse(fs.readFileSync(INSTALLED_FILE, 'utf-8'))
    return parsed?.version === 2 && parsed.plugins ? parsed : { version: 2, plugins: parsed?.plugins ?? {} }
  } catch {
    return { version: 2, plugins: {} }
  }
}

interface PluginDependency { id: string; version?: string }

function pluginDependencies(manifest: any, marketplace: string): PluginDependency[] {
  if (!Array.isArray(manifest?.dependencies)) return []
  const out: PluginDependency[] = []
  for (const raw of manifest.dependencies) {
    let name = ''
    let declaredMarketplace = ''
    let version: string | undefined
    if (typeof raw === 'string') name = raw
    else if (raw && typeof raw === 'object') {
      if (typeof raw.name === 'string') name = raw.name
      if (typeof raw.marketplace === 'string') declaredMarketplace = raw.marketplace
      if (typeof raw.version === 'string' && raw.version.trim()) version = raw.version.trim()
    }
    if (!name) continue
    const id = name.includes('@') ? name : `${name}@${declaredMarketplace || marketplace}`
    if (!out.some(dep => dep.id === id && dep.version === version)) out.push({ id, version })
  }
  return out
}

function marketplaceEntryManifest(pluginName: string, marketplace: string): Record<string, unknown> {
  try {
    const known = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json'), 'utf-8'))
    const root = known?.[marketplace]?.installLocation
    if (typeof root !== 'string') return {}
    const manifest = JSON.parse(fs.readFileSync(path.join(root, '.claude-plugin', 'marketplace.json'), 'utf-8'))
    const entry = manifest?.plugins?.find((plugin: any) => plugin?.name === pluginName)
    return entry && typeof entry === 'object' ? entry : {}
  } catch { return {} }
}

function marketplaceInstallRoot(marketplace: string): string | null {
  try {
    const known = JSON.parse(fs.readFileSync(path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json'), 'utf-8'))
    return typeof known?.[marketplace]?.installLocation === 'string' ? known[marketplace].installLocation : null
  } catch { return null }
}

function crossMarketplaceDependencyAllowed(rootMarketplace: string, targetMarketplace: string): boolean {
  if (!rootMarketplace || rootMarketplace === targetMarketplace) return true
  try {
    const root = marketplaceInstallRoot(rootMarketplace)
    if (!root) return false
    const manifest = JSON.parse(fs.readFileSync(path.join(root, '.claude-plugin', 'marketplace.json'), 'utf-8'))
    return Array.isArray(manifest?.allowCrossMarketplaceDependenciesOn)
      && manifest.allowCrossMarketplaceDependenciesOn.includes(targetMarketplace)
  } catch {
    return false
  }
}

function copyPluginDirectory(src: string, dest: string): void {
  const skipDirs = new Set(['node_modules', '.git', '__pycache__', '.venv'])
  fs.mkdirSync(dest, { recursive: true })
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    if (skipDirs.has(entry.name)) continue
    const srcPath = path.join(src, entry.name)
    const destPath = path.join(dest, entry.name)
    if (entry.isDirectory()) copyPluginDirectory(srcPath, destPath)
    else if (entry.isFile()) {
      fs.copyFileSync(srcPath, destPath)
      try { fs.chmodSync(destPath, fs.statSync(srcPath).mode) } catch { /* best effort */ }
    }
  }
}

export function mergeMarketplaceDependencies(pluginName: string, marketplace: string, manifest: any): any {
  const marketplaceEntry = marketplaceEntryManifest(pluginName, marketplace)
  const dependencies = [
    ...(Array.isArray(marketplaceEntry.dependencies) ? marketplaceEntry.dependencies : []),
    ...(Array.isArray(manifest?.dependencies) ? manifest.dependencies : []),
  ]
  return dependencies.length > 0 ? { ...manifest, dependencies } : manifest
}

function pluginDefaultEnabled(pluginName: string, marketplace: string, manifest: any): boolean {
  const marketplaceEntry = marketplaceEntryManifest(pluginName, marketplace)
  if (typeof marketplaceEntry.defaultEnabled === 'boolean') return marketplaceEntry.defaultEnabled
  return typeof manifest?.defaultEnabled === 'boolean' ? manifest.defaultEnabled : true
}

function installedManifest(data: any, pluginId: string): any {
  const root = data?.plugins?.[pluginId]?.[0]?.installPath
  if (typeof root !== 'string') return null
  try { return JSON.parse(fs.readFileSync(path.join(root, '.claude-plugin', 'plugin.json'), 'utf-8')) } catch { return {} }
}

function resolveDependencyClosure(data: any, pluginId: string, manifestOverride?: any): string[] {
  const result: string[] = []
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (id: string, manifest?: any): void => {
    if (visited.has(id)) return
    if (visiting.has(id)) throw new Error(`Plugin dependency cycle detected at ${id}`)
    visiting.add(id)
    const marketplace = id.includes('@') ? id.slice(id.lastIndexOf('@') + 1) : ''
    const pluginName = id.includes('@') ? id.slice(0, id.lastIndexOf('@')) : id
    const effectiveManifest = mergeMarketplaceDependencies(pluginName, marketplace, manifest ?? installedManifest(data, id))
    for (const dependency of pluginDependencies(effectiveManifest, marketplace)) {
      const depId = dependency.id
      if (!data?.plugins?.[depId]?.[0]?.installPath) {
        throw new Error(`Missing dependency ${depId} required by ${id}. Install it first.`)
      }
      if (dependency.version) {
        const installedVersion = String(data.plugins[depId][0].version ?? '')
        if (!semver.validRange(dependency.version)) throw new Error(`Invalid dependency range ${dependency.version} for ${depId}`)
        if (!semver.valid(installedVersion) || !semver.satisfies(installedVersion, dependency.version, { includePrerelease: true })) {
          throw new Error(`Dependency ${depId}@${installedVersion || 'unknown'} does not satisfy ${dependency.version} required by ${id}`)
        }
      }
      visit(depId)
      if (!result.includes(depId)) result.push(depId)
    }
    visiting.delete(id)
    visited.add(id)
  }
  visit(pluginId, manifestOverride)
  return result
}

/** Install missing dependency plugins from configured marketplaces, validate
 * version ranges/cycles, and return dependencies in enable order. */
async function ensureDependencyClosure(data: any, pluginId: string, manifestOverride?: any): Promise<string[]> {
  const result: string[] = []
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const rootAt = pluginId.lastIndexOf('@')
  const rootMarketplace = rootAt > 0 ? pluginId.slice(rootAt + 1) : ''

  const visit = async (id: string, manifest?: any): Promise<void> => {
    if (visited.has(id)) return
    if (visiting.has(id)) throw new Error(`Plugin dependency cycle detected at ${id}`)
    visiting.add(id)
    const at = id.lastIndexOf('@')
    const pluginName = at > 0 ? id.slice(0, at) : id
    const marketplace = at > 0 ? id.slice(at + 1) : ''
    const effectiveManifest = mergeMarketplaceDependencies(pluginName, marketplace, manifest ?? installedManifest(data, id))

    for (const dependency of pluginDependencies(effectiveManifest, marketplace)) {
      const depId = dependency.id
      const depAt = depId.lastIndexOf('@')
      const depName = depAt > 0 ? depId.slice(0, depAt) : depId
      const depMarketplace = depAt > 0 ? depId.slice(depAt + 1) : marketplace
      let installed = data?.plugins?.[depId]?.[0]

      if (!installed?.installPath) {
        if (!crossMarketplaceDependencyAllowed(rootMarketplace, depMarketplace)) {
          throw new Error(
            `Cross-marketplace dependency ${depId} required by ${id} is not allowed; `
            + `add ${depMarketplace} to allowCrossMarketplaceDependenciesOn in ${rootMarketplace}`,
          )
        }
        const marketplaceRoot = marketplaceInstallRoot(depMarketplace)
        if (!marketplaceRoot) {
          throw new Error(`Missing dependency ${depId} required by ${id}; marketplace is not configured`)
        }
        const catalogEntry = marketplaceEntryManifest(depName, depMarketplace)
        if (!catalogEntry.name) {
          throw new Error(`Missing dependency ${depId} required by ${id}; plugin is not in the configured marketplace`)
        }
        const source = await resolvePluginSource(catalogEntry, depMarketplace, marketplaceRoot)
        if (!source.dir || !fs.existsSync(source.dir)) {
          throw new Error(`Unable to resolve dependency ${depId}: ${source.error || 'source directory not found'}`)
        }
        const localManifest = await readPluginManifest(source.dir) ?? {}
        const version = String(localManifest.version ?? catalogEntry.version ?? '1.0.0')
        if (dependency.version) {
          if (!semver.validRange(dependency.version)) throw new Error(`Invalid dependency range ${dependency.version} for ${depId}`)
          if (!semver.valid(version) || !semver.satisfies(version, dependency.version, { includePrerelease: true })) {
            throw new Error(`Dependency ${depId}@${version} does not satisfy ${dependency.version} required by ${id}`)
          }
        }
        const cacheDir = path.join(os.homedir(), '.claude', 'plugins', 'cache', depMarketplace, depName, version)
        fs.rmSync(cacheDir, { recursive: true, force: true })
        copyPluginDirectory(source.dir, cacheDir)
        const now = new Date().toISOString()
        data.plugins[depId] = [{
          scope: 'user',
          installPath: cacheDir,
          version,
          installedAt: now,
          lastUpdated: now,
        }]
        installed = data.plugins[depId][0]
      }

      if (dependency.version) {
        const installedVersion = String(installed.version ?? '')
        if (!semver.validRange(dependency.version)) throw new Error(`Invalid dependency range ${dependency.version} for ${depId}`)
        if (!semver.valid(installedVersion) || !semver.satisfies(installedVersion, dependency.version, { includePrerelease: true })) {
          throw new Error(`Dependency ${depId}@${installedVersion || 'unknown'} does not satisfy ${dependency.version} required by ${id}`)
        }
      }

      await visit(depId)
      if (!result.includes(depId)) result.push(depId)
    }
    visiting.delete(id)
    visited.add(id)
  }

  await visit(pluginId, manifestOverride)
  return result
}

function enabledDependants(data: any, targetId: string): string[] {
  let enabled: Record<string, boolean> = {}
  try { enabled = JSON.parse(fs.readFileSync(SETTINGS_FILE, 'utf-8'))?.enabledPlugins ?? {} } catch {}
  const result: string[] = []
  for (const id of Object.keys(data?.plugins ?? {})) {
    if (id === targetId || enabled[id] === false) continue
    if (!Object.prototype.hasOwnProperty.call(enabled, id)) {
      const root = data?.plugins?.[id]?.[0]?.installPath
      const at = id.lastIndexOf('@')
      if (typeof root === 'string') {
        const manifest = readEffectivePluginManifestSync(
          root,
          at > 0 ? id.slice(0, at) : id,
          at > 0 ? id.slice(at + 1) : '',
        )
        if (manifest.defaultEnabled === false) continue
      }
    }
    try {
      if (resolveDependencyClosure(data, id).includes(targetId)) result.push(id)
    } catch { /* a separately broken graph must not hide direct valid dependants */ }
  }
  return result
}

function setPluginEnabled(pluginId: string, enabled: boolean | null): void {
  let settings: Record<string, any> = {}
  try { settings = JSON.parse(fs.readFileSync(SETTINGS_FILE, 'utf-8')) } catch { /* new/malformed file */ }
  if (!settings.enabledPlugins || typeof settings.enabledPlugins !== 'object') settings.enabledPlugins = {}
  if (enabled === null) delete settings.enabledPlugins[pluginId]
  else settings.enabledPlugins[pluginId] = enabled
  writeJsonAtomic(SETTINGS_FILE, settings)
}

async function assertCanEnablePlugins(pluginIds: readonly string[]): Promise<void> {
  if (pluginIds.length === 0) return
  const activeIds = (await listEnabledInstalledPlugins()).map(plugin => plugin.key)
  assertUniqueEnabledPluginNames([...activeIds, ...pluginIds])
}

export interface PendingPluginHookApproval {
  pluginId: string
  hooks: Array<{ event: string; matcher?: string; handler: any; hash: string }>
  approvedHashes: string[]
}

function hookReviewMarker(hashes: string[]): string {
  const digest = crypto.createHash('sha256').update([...hashes].sort().join(','), 'utf-8').digest('hex').slice(0, 16)
  return `reviewed:${digest}`
}

async function collectHookApprovalEntries(pluginId: string, pluginRoot: string): Promise<PendingPluginHookApproval> {
  const at = pluginId.lastIndexOf('@')
  const manifest = await readEffectivePluginManifest(pluginRoot, pluginId.slice(0, at), pluginId.slice(at + 1))
  const hooks = await collectPluginHooks(pluginRoot, manifest)
  const flat: PendingPluginHookApproval['hooks'] = []
  for (const [event, matchers] of Object.entries(hooks)) {
    if (!Array.isArray(matchers)) continue
    for (const m of matchers as Array<{ matcher?: string; hooks: any[] }>) {
      for (const handler of m.hooks ?? []) {
        flat.push({ event, matcher: m.matcher, handler, hash: hashPluginHook(event, m.matcher, handler) })
      }
    }
  }
  let approvedHashes: string[] = []
  try {
    const approvalFile = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'plugin-hook-approvals.json')
    approvedHashes = JSON.parse(fs.readFileSync(approvalFile, 'utf-8'))?.[pluginId] ?? []
  } catch {}
  return { pluginId, hooks: flat, approvedHashes }
}

export async function getPendingHookApproval(pluginId: string, pluginRoot: string): Promise<PendingPluginHookApproval | null> {
  const approval = await collectHookApprovalEntries(pluginId, pluginRoot)
  const approved = new Set(approval.approvedHashes)
  const marker = hookReviewMarker(approval.hooks.map(hook => hook.hash))
  if (approval.hooks.length === 0 || approved.has(marker) || approval.hooks.every(hook => approved.has(hook.hash))) return null
  return approval
}

export async function getHookApprovalReviewMarker(pluginId: string): Promise<string | null> {
  const plugin = (await listEnabledInstalledPlugins()).find(candidate => candidate.key === pluginId)
  if (!plugin) return null
  const approval = await collectHookApprovalEntries(pluginId, plugin.installPath)
  return hookReviewMarker(approval.hooks.map(hook => hook.hash))
}

export async function listPendingHookApprovals(): Promise<PendingPluginHookApproval[]> {
  const pending = await Promise.all((await listEnabledInstalledPlugins())
    .map(plugin => getPendingHookApproval(plugin.key, plugin.installPath)))
  return pending.filter((entry): entry is PendingPluginHookApproval => entry !== null)
}

export async function requestHookApproval(pluginId: string, pluginRoot: string): Promise<void> {
  const pending = await getPendingHookApproval(pluginId, pluginRoot)
  if (!pending) return
  const win = BrowserWindow.getAllWindows().find(w => !w.isDestroyed()) ?? null
  if (win) win.webContents.send('plugin-hooks:awaiting-approval', pending)
}

async function requestDependencyHookApprovals(data: any, dependencyIds: readonly string[]): Promise<void> {
  for (const dependencyId of dependencyIds) {
    const root = data?.plugins?.[dependencyId]?.[0]?.installPath
    if (typeof root === 'string') {
      try { await requestHookApproval(dependencyId, root) } catch { /* approval remains discoverable from the modal scan */ }
    }
  }
}

export async function enableRequiredPluginDependencies(pluginId: string, manifest: any): Promise<string[]> {
  const data = readInstalledData()
  const dependencies = await ensureDependencyClosure(data, pluginId, manifest)
  await assertCanEnablePlugins(dependencies)
  writeJsonAtomic(INSTALLED_FILE, data)
  for (const dependencyId of dependencies) setPluginEnabled(dependencyId, true)
  await requestDependencyHookApprovals(data, dependencies)
  return dependencies
}

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

    const installedFile = INSTALLED_FILE
    const data = readInstalledData()

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

    const sourceManifest = mergeMarketplaceDependencies(pluginName, marketplace, await readPluginManifest(pluginPath) ?? {})
    const key = `${pluginName}@${marketplace}`
    const dependencyClosure = await ensureDependencyClosure(data, key, sourceManifest)
    const defaultEnabled = pluginDefaultEnabled(pluginName, marketplace, sourceManifest)
    await assertCanEnablePlugins([...dependencyClosure, ...(defaultEnabled ? [key] : [])])

    const cacheDir = path.join(os.homedir(), '.claude', 'plugins', 'cache', marketplace, pluginName, version)
    fs.rmSync(cacheDir, { recursive: true, force: true })
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
    copyDir(pluginPath, cacheDir)

    const now = new Date().toISOString()
    data.plugins[key] = [{
      scope: 'user',
      installPath: cacheDir,
      version,
      installedAt: now,
      lastUpdated: now,
    }]

    writeJsonAtomic(installedFile, data)
    for (const dependencyId of dependencyClosure) setPluginEnabled(dependencyId, true)
    setPluginEnabled(key, defaultEnabled)

    // Plugin-hook approval flow: if the freshly installed plugin declares
    // hooks (in plugin.json or hooks.json), require explicit user consent
    // before they can fire. Compute a sha256 hash per hook and store the
    // approved set; on re-install the same hashes auto-approve, anything
    // changed re-prompts.
    try { await requestHookApproval(key, cacheDir) } catch { /* approval flow is best-effort */ }
    await requestDependencyHookApprovals(data, dependencyClosure)

    // Pick up plugin-declared MCP servers immediately without restart.
    await reloadMcp()
    // Fire bridge-emit event so user-registered hooks can react to the install.
    try {
      const { emitBridgeHookEvent } = await import('../../hooks/emit-bridge-event')
      emitBridgeHookEvent('PluginInstalled', { pluginId: key, version, installPath: cacheDir })
    } catch { /* ignore */ }
  })

  ipcMain.handle('plugins:install-local', async () => {
    const { dialog } = await import('@kaminide/host-compat')
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory'],
      title: 'Select plugin folder',
    })
    if (result.canceled || result.filePaths.length === 0) return null

    const pluginDir = result.filePaths[0]
    let pluginName = path.basename(pluginDir)
    const marketplace = 'local'
    let version = '1.0.0'

    try {
      const pj = path.join(pluginDir, '.claude-plugin', 'plugin.json')
      if (fs.existsSync(pj)) {
        const manifest = JSON.parse(fs.readFileSync(pj, 'utf-8'))
        if (typeof manifest.name === 'string' && manifest.name.trim()) pluginName = manifest.name.trim()
        if (manifest.version) version = manifest.version
      }
    } catch {}

    // Validate and resolve dependencies before replacing an existing cache.
    // A missing/cyclic/unsatisfied dependency must leave the currently
    // installed version intact instead of deleting it and then throwing.
    const installedFile = INSTALLED_FILE
    const data = readInstalledData()
    const key = `${pluginName}@${marketplace}`
    const localManifest = await readPluginManifest(pluginDir) ?? {}
    const dependencyClosure = await ensureDependencyClosure(data, key, localManifest)
    const defaultEnabled = pluginDefaultEnabled(pluginName, marketplace, localManifest)
    await assertCanEnablePlugins([...dependencyClosure, ...(defaultEnabled ? [key] : [])])

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
          try { fs.chmodSync(destPath, fs.statSync(srcPath).mode) } catch { /* best effort */ }
        }
      }
    }
    copyDir(pluginDir, cacheDir)

    const now = new Date().toISOString()
    data.plugins[key] = [{
      scope: 'user',
      installPath: cacheDir,
      version,
      installedAt: now,
      lastUpdated: now,
    }]
    writeJsonAtomic(installedFile, data)
    for (const dependencyId of dependencyClosure) setPluginEnabled(dependencyId, true)
    setPluginEnabled(key, defaultEnabled)
    try { await requestHookApproval(key, cacheDir) } catch { /* approval flow is best-effort */ }
    await requestDependencyHookApprovals(data, dependencyClosure)
    await reloadMcp()

    return { name: pluginName, marketplace, version, installPath: cacheDir }
  })

  ipcMain.handle('plugins:uninstall', async (_event: IpcMainInvokeEvent, pluginName: string, marketplace: string) => {
    const installedFile = INSTALLED_FILE
    if (!fs.existsSync(installedFile)) return

    try {
      const data = JSON.parse(fs.readFileSync(installedFile, 'utf-8'))
      const key = `${pluginName}@${marketplace}`
      const dependants = enabledDependants(data, key)
      if (dependants.length > 0) {
        throw new Error(`Cannot uninstall ${key}; required by enabled plugin(s): ${dependants.join(', ')}`)
      }
      if (data.plugins?.[key]) {
        const entry = data.plugins[key][0]
        if (entry?.installPath && fs.existsSync(entry.installPath)) {
          fs.rmSync(entry.installPath, { recursive: true, force: true })
        }
        delete data.plugins[key]
        writeJsonAtomic(installedFile, data)
        setPluginEnabled(key, null)
      }
    } catch (err) {
      if (err instanceof Error) throw err
      throw new Error(String(err))
    }
    // Drop plugin-sourced MCP entries immediately after uninstall.
    await reloadMcp()
    try {
      const { emitBridgeHookEvent } = await import('../../hooks/emit-bridge-event')
      emitBridgeHookEvent('PluginUninstalled', { pluginId: `${pluginName}@${marketplace}` })
    } catch { /* ignore */ }
  })

  ipcMain.handle('plugins:set-enabled', async (_event: IpcMainInvokeEvent, pluginId: string, enabled: boolean) => {
    if (typeof pluginId !== 'string' || !pluginId.includes('@') || typeof enabled !== 'boolean') {
      return { ok: false, error: 'Invalid plugin id or enabled state' }
    }
    const data = readInstalledData()
    if (!data.plugins?.[pluginId]) return { ok: false, error: `Plugin ${pluginId} is not installed` }
    if (enabled) {
      const dependencies = resolveDependencyClosure(data, pluginId)
      try {
        await assertCanEnablePlugins([...dependencies, pluginId])
      } catch (err) {
        return { ok: false, error: err instanceof Error ? err.message : String(err) }
      }
      for (const dependencyId of dependencies) setPluginEnabled(dependencyId, true)
      setPluginEnabled(pluginId, true)
      await requestDependencyHookApprovals(data, dependencies)
      const root = data.plugins[pluginId]?.[0]?.installPath
      if (typeof root === 'string') await requestHookApproval(pluginId, root)
    } else {
      const dependants = enabledDependants(data, pluginId)
      if (dependants.length > 0) {
        return { ok: false, error: `Required by enabled plugin(s): ${dependants.join(', ')}` }
      }
      setPluginEnabled(pluginId, false)
    }
    await reloadMcp()
    return { ok: true, restartRequired: true }
  })
}
