// IPC for the Hooks customize tab. Acts as a thin wrapper around the
// bridge's `/api/hooks/*` endpoints (read-only views) plus host-side
// settings.json mutations (install template, toggle, delete).

import { ipcMain, type IpcMainInvokeEvent } from 'electron'
import fs from 'fs'
import os from 'os'
import path from 'path'
import type { ConfigStore } from '../config/store'

export interface HooksIpcContext {
  configStore: ConfigStore
}

interface MinimalHookCmd { type: string; command?: string; prompt?: string; url?: string; host?: string; if?: string; matcher?: string; timeout?: number }
interface MinimalMatcher { matcher?: string; hooks: MinimalHookCmd[] }
interface MinimalSettings { hooks?: Record<string, MinimalMatcher[]> }

const USER_SETTINGS = path.join(os.homedir(), '.claude', 'settings.json')

function readSettings(): MinimalSettings {
  try {
    if (!fs.existsSync(USER_SETTINGS)) return {}
    return JSON.parse(fs.readFileSync(USER_SETTINGS, 'utf-8'))
  } catch {
    return {}
  }
}

interface FlatHook {
  event: string
  matcherIdx: number
  matcher?: string
  handler: MinimalHookCmd
  source: { kind: string; pluginId?: string; manifestPath?: string; projectPath?: string }
}

function readHooksAtPath(filePath: string): Record<string, MinimalMatcher[]> | null {
  try {
    if (!fs.existsSync(filePath)) return null
    const raw = fs.readFileSync(filePath, 'utf-8')
    const parsed = JSON.parse(raw) as MinimalSettings
    return parsed.hooks ?? null
  } catch {
    return null
  }
}

function loadEnabledPluginsHost(): Array<{ pluginId: string; installPath: string }> {
  try {
    const installedFile = path.join(os.homedir(), '.claude', 'plugins', 'installed_plugins.json')
    if (!fs.existsSync(installedFile)) return []
    const parsed = JSON.parse(fs.readFileSync(installedFile, 'utf-8')) as {
      plugins?: Record<string, Array<{ installPath?: string }>>
    }
    const settingsFile = USER_SETTINGS
    let enabledMap: Record<string, boolean> = {}
    try {
      if (fs.existsSync(settingsFile)) {
        enabledMap = (JSON.parse(fs.readFileSync(settingsFile, 'utf-8')) as { enabledPlugins?: Record<string, boolean> }).enabledPlugins ?? {}
      }
    } catch { /* ignore */ }
    const out: Array<{ pluginId: string; installPath: string }> = []
    for (const [key, entries] of Object.entries(parsed.plugins ?? {})) {
      if (enabledMap[key] === false) continue
      const entry = Array.isArray(entries) ? entries[0] : null
      if (!entry?.installPath) continue
      out.push({ pluginId: key, installPath: entry.installPath })
    }
    return out
  } catch {
    return []
  }
}

function readPluginHooksHost(installPath: string): Record<string, MinimalMatcher[]> | null {
  // 1. Standard discovery paths the CLI scans regardless of manifest.
  for (const cand of [
    path.join(installPath, '.claude-plugin', 'plugin.json'),
    // CLI supports a legacy `<dir>/plugin.json` fallback when the
    // .claude-plugin directory is absent.
    path.join(installPath, 'plugin.json'),
    path.join(installPath, 'hooks.json'),
    path.join(installPath, '.claude-plugin', 'hooks.json'),
    // Canonical CLI convention — most plugins (redact, etc.) ship hooks here.
    path.join(installPath, 'hooks', 'hooks.json'),
  ]) {
    const hooks = readHooksAtPath(cand)
    if (hooks && Object.keys(hooks).length > 0) return hooks
  }
  // 2. Manifest-declared `hooks` field as a path string or array of paths
  //    (CLI: manifest.hooks accepts inline objects OR file references).
  for (const mp of [
    path.join(installPath, '.claude-plugin', 'plugin.json'),
    path.join(installPath, 'plugin.json'),
  ]) {
    if (!fs.existsSync(mp)) continue
    try {
      const m = JSON.parse(fs.readFileSync(mp, 'utf-8'))
      const hooksField = m?.hooks
      if (typeof hooksField === 'string') {
        const refPath = path.isAbsolute(hooksField) ? hooksField : path.join(installPath, hooksField)
        const hooks = readHooksAtPath(refPath)
        if (hooks && Object.keys(hooks).length > 0) return hooks
      } else if (Array.isArray(hooksField)) {
        const merged: Record<string, MinimalMatcher[]> = {}
        for (const ref of hooksField) {
          if (typeof ref !== 'string') continue
          const refPath = path.isAbsolute(ref) ? ref : path.join(installPath, ref)
          const hooks = readHooksAtPath(refPath)
          if (!hooks) continue
          for (const [evt, matchers] of Object.entries(hooks)) {
            merged[evt] = (merged[evt] ?? []).concat(matchers)
          }
        }
        if (Object.keys(merged).length > 0) return merged
      }
    } catch { /* malformed manifest — skip */ }
  }
  return null
}

function mergeHooksOnHost(cwd?: string): { hooks: FlatHook[] } {
  const flat: FlatHook[] = []

  function pushAll(hooks: Record<string, MinimalMatcher[]>, source: FlatHook['source']): void {
    for (const [event, matchers] of Object.entries(hooks)) {
      if (!Array.isArray(matchers)) continue
      matchers.forEach((m, mi) => {
        m.hooks.forEach(h => {
          flat.push({ event, matcherIdx: mi, matcher: m.matcher, handler: h, source })
        })
      })
    }
  }

  // 1. user-global
  const userHooks = readHooksAtPath(USER_SETTINGS)
  if (userHooks) pushAll(userHooks, { kind: 'user' })

  // 2. project
  if (cwd) {
    const projectHooks = readHooksAtPath(path.join(cwd, '.claude', 'settings.json'))
    if (projectHooks) pushAll(projectHooks, { kind: 'project', projectPath: cwd })
  }

  // 3. plugins
  for (const p of loadEnabledPluginsHost()) {
    const hooks = readPluginHooksHost(p.installPath)
    if (!hooks) continue
    const manifestPath = path.join(p.installPath, '.claude-plugin', 'plugin.json')
    pushAll(hooks, { kind: 'plugin', pluginId: p.pluginId, manifestPath })
  }

  return { hooks: flat }
}

function writeSettings(settings: MinimalSettings): void {
  fs.mkdirSync(path.dirname(USER_SETTINGS), { recursive: true })
  fs.writeFileSync(USER_SETTINGS, JSON.stringify(settings, null, 2))
}

async function bridgeFetch(ctx: HooksIpcContext, suffix: string, init?: RequestInit): Promise<Response> {
  const cfg = ctx.configStore.get()
  if (!cfg?.serverUrl || !cfg?.token) throw new Error('Server URL or token not configured')
  // ws://… → http://… for HTTP API
  const httpBase = cfg.serverUrl.replace(/^wss?:\/\//, m => m === 'wss://' ? 'https://' : 'http://').replace(/\/ws.*$/, '')
  return fetch(`${httpBase}${suffix}`, {
    ...init,
    headers: { ...(init?.headers ?? {}), Authorization: `Bearer ${cfg.token}` },
  })
}

export function registerHooksIPC(ctx: HooksIpcContext): void {
  // ── Read user-defined hooks straight from host settings.json. ──
  ipcMain.handle('hooks:list', async () => {
    const settings = readSettings()
    return { hooks: settings.hooks ?? {} }
  })

  // ── Merged view of user + project + plugins (with source labels). ──
  // We do the merge on the Electron side, reading host files directly. The
  // bridge container only sees synced copies of these files, and freshly
  // installed user-hooks haven't propagated there yet, so a server-side
  // merge would lag the host's true state.
  ipcMain.handle('hooks:list-all', async (_e: IpcMainInvokeEvent, cwd?: string) => {
    try {
      return mergeHooksOnHost(cwd)
    } catch {
      return { hooks: [] }
    }
  })

  // ── Library templates (static catalog from bridge). ──
  ipcMain.handle('hooks:get-library', async () => {
    try {
      const r = await bridgeFetch(ctx, '/api/hooks/library')
      if (!r.ok) return { templates: [] }
      return await r.json()
    } catch {
      return { templates: [] }
    }
  })

  // ── Audit log — recent executions ──
  ipcMain.handle('hooks:get-log', async (_e: IpcMainInvokeEvent, limit: number = 200) => {
    try {
      const r = await bridgeFetch(ctx, `/api/hooks/log?limit=${limit}`)
      if (!r.ok) return { entries: [] }
      return await r.json()
    } catch {
      return { entries: [] }
    }
  })

  ipcMain.handle('hooks:clear-log', async () => {
    try {
      await bridgeFetch(ctx, '/api/hooks/log/clear', { method: 'POST' })
      return { ok: true }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // ── Currently active in a session (registry). ──
  ipcMain.handle('hooks:active', async (_e, sessionId: string) => {
    try {
      const r = await bridgeFetch(ctx, `/api/hooks/active/${sessionId}`)
      if (!r.ok) return { hooks: [] }
      return await r.json()
    } catch {
      return { hooks: [] }
    }
  })

  // ── Install a Library template into user settings.json ──
  ipcMain.handle('hooks:install-template', async (_e, templateId: string) => {
    try {
      let templates: Array<{ id: string; event: string; matcher?: string; handler: MinimalHookCmd }> = []
      try {
        const lib = await bridgeFetch(ctx, '/api/hooks/library')
        if (lib.ok) {
          const j = await lib.json() as { templates?: typeof templates }
          templates = j?.templates ?? []
        }
      } catch { /* fall through — bridge may be temporarily down */ }
      if (templates.length === 0) {
        return { ok: false, error: 'Library is unreachable. Check bridge connection.' }
      }
      const tmpl = templates.find(t => t.id === templateId)
      if (!tmpl) return { ok: false, error: `Template "${templateId}" not found` }

      const settings = readSettings()
      settings.hooks = settings.hooks ?? {}
      settings.hooks[tmpl.event] = settings.hooks[tmpl.event] ?? []
      settings.hooks[tmpl.event]!.push({
        matcher: tmpl.matcher,
        hooks: [tmpl.handler],
      })
      writeSettings(settings)
      return { ok: true }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // ── Create a new hook in user settings (HookEditor "Save"). ──
  ipcMain.handle('hooks:save', async (_e, draft: { event: string; matcher?: string; handler: MinimalHookCmd; index?: { matcherIdx: number; handlerIdx: number } }) => {
    try {
      const settings = readSettings()
      settings.hooks = settings.hooks ?? {}
      settings.hooks[draft.event] = settings.hooks[draft.event] ?? []
      const matchers = settings.hooks[draft.event]!
      if (draft.index && matchers[draft.index.matcherIdx]?.hooks[draft.index.handlerIdx]) {
        // Update existing
        const m = matchers[draft.index.matcherIdx]!
        m.matcher = draft.matcher
        m.hooks[draft.index.handlerIdx] = draft.handler
      } else {
        // Append
        matchers.push({ matcher: draft.matcher, hooks: [draft.handler] })
      }
      writeSettings(settings)
      return { ok: true }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // ── Test a hook with a mock payload (HookEditor "Test"). ──
  // For type=command + host=local we execute directly via Electron's own
  // spawn (the server-side route requires a live PTY-session WS to dispatch
  // to, which doesn't exist when the user is just inspecting the Hooks
  // tab — would always fail with "No active Electron WS"). For other types
  // (server-only commands, http, prompt/agent) fall through to the server
  // which already knows how to handle them.
  ipcMain.handle('hooks:test', async (_e, draft: { event: string; matcher?: string; handler: MinimalHookCmd; mockPayload?: Record<string, unknown> }) => {
    try {
      const handler = draft?.handler
      const isLocalCommand = handler?.type === 'command' && (!handler.host || handler.host === 'auto' || handler.host === 'local')
      if (isLocalCommand && typeof handler.command === 'string' && handler.command.length > 0) {
        const { executeHook } = await import('../hooks/executor')
        const start = Date.now()
        const payload = { hook_event_name: draft.event, session_id: '__test__local__', ...(draft.mockPayload ?? {}) }
        const result = await executeHook({
          type: 'hook:execute',
          requestId: '__test__',
          sessionId: '__test__local__',
          hookId: '__test__',
          event: draft.event,
          command: handler.command,
          shell: (handler as any).shell,
          payload,
          timeoutMs: (handler.timeout ?? 60) * 1000,
        })
        return {
          ok: true,
          effectiveHost: 'local',
          result: { ...result, durationMs: result.durationMs ?? (Date.now() - start) },
        }
      }
      // Fall through to server for non-local-command types.
      const r = await bridgeFetch(ctx, '/api/hooks/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(draft),
      })
      if (!r.ok) {
        return { ok: false, error: `Bridge returned ${r.status}` }
      }
      return await r.json()
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // ── Plugin hook approval store. ──
  ipcMain.handle('hooks:get-plugin-approval', async (_e, pluginId: string) => {
    try {
      const approvalsFile = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'plugin-hook-approvals.json')
      if (!fs.existsSync(approvalsFile)) return { approved: false, hashes: [] }
      const parsed = JSON.parse(fs.readFileSync(approvalsFile, 'utf-8')) as Record<string, string[]>
      return { approved: !!parsed[pluginId], hashes: parsed[pluginId] ?? [] }
    } catch {
      return { approved: false, hashes: [] }
    }
  })

  ipcMain.handle('hooks:set-plugin-approval', async (_e, pluginId: string, hashes: string[]) => {
    try {
      const approvalsFile = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'plugin-hook-approvals.json')
      fs.mkdirSync(path.dirname(approvalsFile), { recursive: true })
      let parsed: Record<string, string[]> = {}
      try { if (fs.existsSync(approvalsFile)) parsed = JSON.parse(fs.readFileSync(approvalsFile, 'utf-8')) } catch { /* ignore */ }
      parsed[pluginId] = hashes
      fs.writeFileSync(approvalsFile, JSON.stringify(parsed, null, 2))
      return { ok: true }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })

  // ── Delete a hook by index (in user settings only). ──
  ipcMain.handle('hooks:delete', async (_e, event: string, matcherIdx: number, handlerIdx: number) => {
    try {
      const settings = readSettings()
      const matchers = settings.hooks?.[event]
      if (!matchers || !matchers[matcherIdx]) return { ok: false, error: 'Not found' }
      matchers[matcherIdx]!.hooks.splice(handlerIdx, 1)
      if (matchers[matcherIdx]!.hooks.length === 0) matchers.splice(matcherIdx, 1)
      if (matchers.length === 0) delete settings.hooks![event]
      writeSettings(settings)
      return { ok: true }
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : String(err) }
    }
  })
}
