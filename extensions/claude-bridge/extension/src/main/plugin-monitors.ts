// Plugin monitors — long-running background processes declared by plugins.
//
// A plugin can ship `monitors/monitors.json`, or point to declarations from
// `plugin.json:experimental.monitors` (legacy top-level `monitors` is accepted):
//
//   "monitors": [
//     { "name": "deploy", "command": "${CLAUDE_PLUGIN_ROOT}/poll.sh",
//       "description": "Polls deploy status", "when": "always" },
//     { "name": "log",    "command": "tail -F ./errors.log",
//       "description": "Tails app error log", "when": "on-skill-invoke:debug" }
//   ]
//
// At tab-open time we spawn each `when: "always"` monitor (or missing `when`,
// which defaults to always) under that tab's session. Output lines are
// captured into a ring buffer and broadcast to the renderer via
// `monitor:output` IPC events. On tab close we kill every monitor we started.
//
// `on-skill-invoke:*` is bridged through a synthetic PreToolUse(Skill) hook;
// the command never reaches a shell and only starts the matching monitor.

import fs from 'fs'
import os from 'os'
import path from 'path'
import { spawn, type ChildProcess } from 'child_process'
import type { BrowserWindow } from '@kaminide/host-compat'
import {
  listEnabledInstalledPlugins,
  readEffectivePluginManifest,
} from './plugin-helpers'
import { CircularBuffer } from './utils/circular-buffer'

const RING_CAP = 200
const MAX_PENDING_LINE_CHARS = 64 * 1024
const MAX_NOTIFICATION_LINES = 50
const MAX_NOTIFICATION_CHARS = 8 * 1024
const NOTIFICATION_DEBOUNCE_MS = 5_000
const NOTIFICATION_MIN_INTERVAL_MS = 30_000

export interface MonitorLine { ts: number; stream: 'stdout' | 'stderr' | 'meta'; text: string }

interface MonitorInstance {
  id: string                // "<tabId>/<pluginId>/<monitor.name>"
  tabId: string
  pluginId: string          // "name@marketplace"
  pluginName: string
  monitorName: string
  description: string
  command: string           // resolved
  cwd: string
  env: Record<string, string>
  notifyClaude?: (line: string) => void
  process?: ChildProcess
  ring: CircularBuffer<MonitorLine>
  startedAt: number
  status: 'running' | 'exited' | 'error'
  exitCode?: number | null
  notifyTimer?: ReturnType<typeof setTimeout>
  pendingNotifications: string[]
  pendingNotificationChars: number
  droppedNotifications: number
  lastNotificationAt: number
}

/** Live set of running monitors — keyed by composite id so two tabs can
 *  each have their own copy of the same plugin's monitor. */
const monitors = new Map<string, MonitorInstance>()
/** Reverse lookup: every monitor id a given tab owns. */
const tabOwners = new Map<string, Set<string>>()
const tabContexts = new Map<string, {
  projectCwd?: string
  notifyClaude?: (pluginId: string, monitorName: string, line: string) => void
}>()
// Discovery reads manifests asynchronously. A tab can close or a plugin reload
// can replace its monitor set while those reads are in flight; the token keeps
// a stale continuation from spawning an orphan process afterward.
const tabLoadTokens = new Map<string, symbol>()

let mainWindow: BrowserWindow | null = null
export function setMonitorsWindow(w: BrowserWindow | null): void {
  mainWindow = w
}

function broadcast(channel: string, payload: unknown): void {
  if (!mainWindow || mainWindow.isDestroyed()) return
  mainWindow.webContents.send(channel, payload)
}

function flushClaudeNotifications(inst: MonitorInstance): void {
  if (inst.notifyTimer) clearTimeout(inst.notifyTimer)
  inst.notifyTimer = undefined
  if (!inst.notifyClaude || inst.pendingNotifications.length === 0) {
    inst.pendingNotifications = []
    inst.pendingNotificationChars = 0
    inst.droppedNotifications = 0
    return
  }
  const suffix = inst.droppedNotifications > 0
    ? `\n[monitor] ${inst.droppedNotifications} additional output line(s) were coalesced`
    : ''
  const message = `${inst.pendingNotifications.join('\n')}${suffix}`
  inst.pendingNotifications = []
  inst.pendingNotificationChars = 0
  inst.droppedNotifications = 0
  inst.lastNotificationAt = Date.now()
  try { inst.notifyClaude(message) } catch { /* tab/session may have closed */ }
}

function queueClaudeNotification(inst: MonitorInstance, text: string): void {
  if (!inst.notifyClaude) return
  const truncationMarker = '\n[monitor] line truncated'
  const clipped = text.length > MAX_NOTIFICATION_CHARS
    ? `${text.slice(0, MAX_NOTIFICATION_CHARS - truncationMarker.length)}${truncationMarker}`
    : text
  if (
    inst.pendingNotifications.length >= MAX_NOTIFICATION_LINES
    || inst.pendingNotificationChars + clipped.length > MAX_NOTIFICATION_CHARS
  ) {
    inst.droppedNotifications += 1
  } else {
    inst.pendingNotifications.push(clipped)
    inst.pendingNotificationChars += clipped.length
  }
  // Monitor stdout is model input, not a passive log stream. Use a trailing
  // debounce (continuous heartbeat/log streams never create turns) plus a
  // minimum interval for sparse but still chatty output.
  if (inst.notifyTimer) clearTimeout(inst.notifyTimer)
  const delay = Math.max(
    NOTIFICATION_DEBOUNCE_MS,
    inst.lastNotificationAt + NOTIFICATION_MIN_INTERVAL_MS - Date.now(),
  )
  inst.notifyTimer = setTimeout(() => flushClaudeNotifications(inst), delay)
}

function appendLine(inst: MonitorInstance, stream: MonitorLine['stream'], text: string): void {
  const entry: MonitorLine = { ts: Date.now(), stream, text }
  inst.ring.add(entry)
  broadcast('monitor:output', { id: inst.id, entry })
  if (stream === 'stdout' && text) queueClaudeNotification(inst, text)
}

/** Incrementally split process output while keeping the no-newline tail
 * bounded. Exported for the lifecycle regression tests. */
export function consumeMonitorOutput(
  previous: string,
  chunk: string,
  emit: (line: string) => void,
): string {
  let buffer = previous + chunk
  let idx: number
  while ((idx = buffer.indexOf('\n')) !== -1) {
    const rawLine = buffer.slice(0, idx).replace(/\r$/, '')
    buffer = buffer.slice(idx + 1)
    if (rawLine) {
      emit(rawLine.length > MAX_PENDING_LINE_CHARS
        ? `${rawLine.slice(0, MAX_PENDING_LINE_CHARS)}\n[monitor] line truncated`
        : rawLine)
    }
  }
  if (buffer.length > MAX_PENDING_LINE_CHARS) {
    emit(`${buffer.slice(0, MAX_PENDING_LINE_CHARS)}\n[monitor] unterminated line truncated`)
    return ''
  }
  return buffer
}

function substituteAll(
  str: string,
  pluginRoot: string,
  pluginId?: string,
  projectCwd?: string,
): string {
  // Current Claude Code deliberately rejects user_config interpolation in
  // monitor shell commands. Unlike exec-form hooks, a monitor has no argv
  // boundary, so interpolating arbitrary option text would be shell injection.
  if (/\$\{user_config\.[^}]+\}/.test(str)) {
    throw new Error('Monitor commands cannot reference ${user_config.*}; read options from a plugin-owned config file')
  }
  let out = str
  out = out.replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
  out = out.replace(/\$\{CLAUDE_PROJECT_DIR\}/g, projectCwd || os.homedir())
  if (pluginId && out.includes('${CLAUDE_PLUGIN_DATA}')) {
    const dataDir = path.join(os.homedir(), '.claude', 'plugins', 'data', pluginId.replace(/[^a-zA-Z0-9\-_]/g, '-'))
    try { fs.mkdirSync(dataDir, { recursive: true }) } catch { /* best effort */ }
    out = out.replace(/\$\{CLAUDE_PLUGIN_DATA\}/g, dataDir)
  }
  out = out.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}/g, (_m, name: string) => {
    const v = process.env[name]
    return v === undefined ? '' : v
  })
  return out
}

function spawnMonitor(inst: MonitorInstance): void {
  try {
    // `shell: true` mirrors how CLI runs monitors — the `command` field is a
    // shell string (pipes, redirects, &&). On Windows this launches cmd.exe,
    // on POSIX /bin/sh. `windowsHide` prevents flash-of-console windows.
    const proc = spawn(inst.command, {
      shell: true,
      cwd: inst.cwd,
      env: { ...process.env, ...inst.env },
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
      detached: process.platform !== 'win32',
    })
    inst.process = proc
    inst.status = 'running'
    appendLine(inst, 'meta', `[monitor] spawned pid=${proc.pid ?? 'n/a'}: ${inst.command}`)

    let stdoutBuf = ''
    proc.stdout?.on('data', (chunk: Buffer) => {
      stdoutBuf = consumeMonitorOutput(stdoutBuf, chunk.toString(), line => appendLine(inst, 'stdout', line))
    })

    let stderrBuf = ''
    proc.stderr?.on('data', (chunk: Buffer) => {
      stderrBuf = consumeMonitorOutput(stderrBuf, chunk.toString(), line => appendLine(inst, 'stderr', line))
    })

    proc.on('error', (err) => {
      appendLine(inst, 'meta', `[monitor] spawn error: ${err.message}`)
      inst.status = 'error'
      broadcast('monitor:status', { id: inst.id, status: inst.status })
    })

    proc.on('exit', (code, signal) => {
      if (stdoutBuf) appendLine(inst, 'stdout', stdoutBuf)
      if (stderrBuf) appendLine(inst, 'stderr', stderrBuf)
      flushClaudeNotifications(inst)
      appendLine(inst, 'meta', `[monitor] exited code=${code} signal=${signal ?? 'none'}`)
      inst.status = 'exited'
      inst.exitCode = code
      inst.process = undefined
      broadcast('monitor:status', { id: inst.id, status: inst.status, exitCode: code })
    })
  } catch (err) {
    const m = err instanceof Error ? err.message : String(err)
    appendLine(inst, 'meta', `[monitor] failed to spawn: ${m}`)
    inst.status = 'error'
    broadcast('monitor:status', { id: inst.id, status: inst.status })
  }
}

/** Enumerate enabled plugins, read their manifest.monitors, spawn every
 *  `when: "always"` (or missing `when`). Intended to be called once at tab
 *  open. Returns the spawned monitor ids so the caller can later kill them. */
export async function loadMonitorDeclarations(pluginRoot: string, manifest: Record<string, any>): Promise<any[]> {
  const readFile = async (ref: string): Promise<any[]> => {
    const resolved = path.resolve(pluginRoot, ref)
    const relative = path.relative(pluginRoot, resolved)
    if (relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) return []
    try {
      const parsed = JSON.parse(await fs.promises.readFile(resolved, 'utf-8'))
      return Array.isArray(parsed) ? parsed : []
    } catch { return [] }
  }

  const experimental = manifest.experimental && typeof manifest.experimental === 'object'
    ? manifest.experimental as Record<string, unknown>
    : {}
  const declared = experimental.monitors ?? manifest.monitors
  if (declared === undefined) return readFile('monitors/monitors.json')
  if (typeof declared === 'string') return readFile(declared)
  if (!Array.isArray(declared)) return []
  const out: any[] = []
  for (const item of declared) {
    if (typeof item === 'string') out.push(...await readFile(item))
    else if (item && typeof item === 'object') out.push(item)
  }
  return out
}

export async function startMonitorsForTab(
  tabId: string,
  projectCwd?: string,
  notifyClaude?: (pluginId: string, monitorName: string, line: string) => void,
): Promise<string[]> {
  const loadToken = Symbol(tabId)
  tabLoadTokens.set(tabId, loadToken)
  tabContexts.set(tabId, { projectCwd, notifyClaude })
  const plugins = await listEnabledInstalledPlugins()
  const started: string[] = []

  for (const plugin of plugins) {
    if (tabLoadTokens.get(tabId) !== loadToken) break
    const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
    const rawMonitors = await loadMonitorDeclarations(plugin.installPath, manifest)
    if (tabLoadTokens.get(tabId) !== loadToken) break
    if (rawMonitors.length === 0) continue

    const pluginRoot = plugin.installPath

    for (const m of rawMonitors) {
      if (tabLoadTokens.get(tabId) !== loadToken) break
      if (!m || typeof m !== 'object') continue
      const monitorName = typeof m.name === 'string' ? m.name : ''
      const commandRaw = typeof m.command === 'string' ? m.command : ''
      const description = typeof m.description === 'string' ? m.description : ''
      const when = typeof m.when === 'string' ? m.when : 'always'
      if (!monitorName || !commandRaw) continue
      if (when !== 'always' && when !== '') {
        continue
      }

      const id = `${tabId}/${plugin.key}/${monitorName}`
      if (monitors.has(id)) continue  // already running

      let command: string
      try {
        command = substituteAll(commandRaw, pluginRoot, plugin.key, projectCwd)
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        console.warn(`[plugin-monitor] Skipping ${plugin.key}/${monitorName}: ${message}`)
        broadcast('monitor:error', { tabId, pluginId: plugin.key, monitorName, error: message })
        continue
      }

      const pluginDataDir = path.join(os.homedir(), '.claude', 'plugins', 'data', plugin.key.replace(/[^a-zA-Z0-9\-_]/g, '-'))
      try { fs.mkdirSync(pluginDataDir, { recursive: true }) } catch { /* best effort */ }

      const inst: MonitorInstance = {
        id,
        tabId,
        pluginId: plugin.key,
        pluginName: plugin.name,
        monitorName,
        description,
        command,
        cwd: projectCwd || os.homedir(),
        env: Object.fromEntries([
          ['CLAUDE_PLUGIN_ROOT', pluginRoot],
          ['CLAUDE_PLUGIN_DATA', pluginDataDir],
          ['CLAUDE_PROJECT_DIR', projectCwd || os.homedir()],
        ]),
        notifyClaude: notifyClaude ? (line) => notifyClaude(plugin.key, monitorName, line) : undefined,
        pendingNotifications: [],
        pendingNotificationChars: 0,
        droppedNotifications: 0,
        lastNotificationAt: 0,
        ring: new CircularBuffer<MonitorLine>(RING_CAP),
        startedAt: Date.now(),
        status: 'running',
      }
      const pluginBin = path.join(pluginRoot, 'bin')
      if (fs.existsSync(pluginBin)) inst.env.PATH = `${pluginBin}${path.delimiter}${process.env.PATH ?? ''}`
      monitors.set(id, inst)
      if (!tabOwners.has(tabId)) tabOwners.set(tabId, new Set())
      tabOwners.get(tabId)!.add(id)
      started.push(id)
      spawnMonitor(inst)
      broadcast('monitor:started', {
        id, tabId, pluginId: plugin.key, pluginName: plugin.name,
        monitorName, description, command: inst.command,
      })
    }
  }
  return started
}

/** Kill every monitor owned by a tab. Safe to call on tab close. */
export function stopMonitorsForTab(tabId: string): void {
  const owned = tabOwners.get(tabId)
  tabLoadTokens.delete(tabId)
  tabContexts.delete(tabId)
  if (!owned) return
  for (const id of owned) {
    const inst = monitors.get(id)
    if (!inst) continue
    if (inst.notifyTimer) clearTimeout(inst.notifyTimer)
    inst.pendingNotifications = []
    inst.pendingNotificationChars = 0
    inst.droppedNotifications = 0
    if (inst.process && !inst.process.killed) {
      try {
        if (process.platform === 'win32' && inst.process.pid) {
          spawn('taskkill', ['/pid', String(inst.process.pid), '/T', '/F'], { windowsHide: true })
        } else if (inst.process.pid) {
          try { process.kill(-inst.process.pid, 'SIGTERM') } catch { inst.process.kill('SIGTERM') }
        } else {
          inst.process.kill('SIGTERM')
        }
      } catch {}
    }
    monitors.delete(id)
    broadcast('monitor:stopped', { id })
  }
  tabOwners.delete(tabId)
}

const TRIGGER_COMMAND = '__bridge_monitor_start__:'

export async function buildMonitorTriggerHooks(
  pluginRoot: string,
  manifest: Record<string, any>,
  pluginId: string,
): Promise<Record<string, unknown[]>> {
  const handlers: unknown[] = []
  for (const monitor of await loadMonitorDeclarations(pluginRoot, manifest)) {
    if (!monitor || typeof monitor !== 'object' || typeof monitor.name !== 'string' || typeof monitor.when !== 'string') continue
    if (!monitor.when.startsWith('on-skill-invoke:')) continue
    const skill = monitor.when.slice('on-skill-invoke:'.length).trim()
    if (!skill) continue
    const token = [pluginId, monitor.name, skill].map(value => Buffer.from(value).toString('base64url')).join(':')
    handlers.push({ type: 'command', command: `${TRIGGER_COMMAND}${token}`, host: 'local', timeout: 5 })
  }
  return handlers.length > 0 ? { PreToolUse: [{ matcher: 'Skill', hooks: handlers }] } : {}
}

export async function handleMonitorTriggerCommand(tabId: string, command: string, payload: Record<string, unknown>, sourcePluginId?: string): Promise<boolean> {
  if (!command.startsWith(TRIGGER_COMMAND)) return false
  const parts = command.slice(TRIGGER_COMMAND.length).split(':')
  if (parts.length !== 3) return true
  const [pluginId, monitorName, expectedSkill] = parts.map(value => Buffer.from(value, 'base64url').toString('utf-8'))
  if (sourcePluginId !== pluginId) return true
  const toolInput = payload.tool_input && typeof payload.tool_input === 'object'
    ? payload.tool_input as Record<string, unknown>
    : {}
  const invoked = [toolInput.skill, toolInput.name, toolInput.command]
    .find(value => typeof value === 'string')
  if (typeof invoked !== 'string') return true
  const bareInvoked = invoked.includes(':') ? invoked.slice(invoked.lastIndexOf(':') + 1) : invoked
  if (invoked !== expectedSkill && bareInvoked !== expectedSkill) return true

  const plugin = (await listEnabledInstalledPlugins()).find(candidate => candidate.key === pluginId)
  const tabContext = tabContexts.get(tabId)
  if (!plugin || !tabContext) return true
  const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
  const declaration = (await loadMonitorDeclarations(plugin.installPath, manifest)).find((monitor: any) =>
    monitor && monitor.name === monitorName && monitor.when === `on-skill-invoke:${expectedSkill}`)
  if (tabContexts.get(tabId) !== tabContext) return true
  if (!declaration || typeof declaration.command !== 'string') return true

  const id = `${tabId}/${plugin.key}/${monitorName}`
  if (monitors.has(id)) return true
  let resolvedCommand: string
  try {
    resolvedCommand = substituteAll(declaration.command, plugin.installPath, plugin.key, tabContext.projectCwd)
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    console.warn(`[plugin-monitor] Cannot start ${plugin.key}/${monitorName}: ${message}`)
    broadcast('monitor:error', { tabId, pluginId: plugin.key, monitorName, error: message })
    return true
  }
  const pluginDataDir = path.join(os.homedir(), '.claude', 'plugins', 'data', plugin.key.replace(/[^a-zA-Z0-9\-_]/g, '-'))
  try { fs.mkdirSync(pluginDataDir, { recursive: true }) } catch {}
  const inst: MonitorInstance = {
    id,
    tabId,
    pluginId: plugin.key,
    pluginName: plugin.name,
    monitorName,
    description: typeof declaration.description === 'string' ? declaration.description : '',
    command: resolvedCommand,
    cwd: tabContext.projectCwd || os.homedir(),
    env: Object.fromEntries([
      ['CLAUDE_PLUGIN_ROOT', plugin.installPath],
      ['CLAUDE_PLUGIN_DATA', pluginDataDir],
      ['CLAUDE_PROJECT_DIR', tabContext.projectCwd || os.homedir()],
    ]),
    notifyClaude: tabContext.notifyClaude ? (line) => tabContext.notifyClaude!(plugin.key, monitorName, line) : undefined,
    pendingNotifications: [],
    pendingNotificationChars: 0,
    droppedNotifications: 0,
    lastNotificationAt: 0,
    ring: new CircularBuffer<MonitorLine>(RING_CAP),
    startedAt: Date.now(),
    status: 'running',
  }
  const pluginBin = path.join(plugin.installPath, 'bin')
  if (fs.existsSync(pluginBin)) inst.env.PATH = `${pluginBin}${path.delimiter}${process.env.PATH ?? ''}`
  monitors.set(id, inst)
  if (!tabOwners.has(tabId)) tabOwners.set(tabId, new Set())
  tabOwners.get(tabId)!.add(id)
  spawnMonitor(inst)
  broadcast('monitor:started', {
    id, tabId, pluginId: plugin.key, pluginName: plugin.name,
    monitorName, description: inst.description, command: inst.command,
  })
  return true
}

export interface MonitorSummary {
  id: string
  tabId: string
  pluginId: string
  pluginName: string
  monitorName: string
  description: string
  command: string
  status: MonitorInstance['status']
  startedAt: number
  exitCode?: number | null
}

export function listMonitors(): MonitorSummary[] {
  return [...monitors.values()].map(m => ({
    id: m.id, tabId: m.tabId, pluginId: m.pluginId, pluginName: m.pluginName,
    monitorName: m.monitorName, description: m.description, command: m.command,
    status: m.status, startedAt: m.startedAt, exitCode: m.exitCode,
  }))
}

/** Return the ring buffer for a monitor (fresh copy, safe to mutate). */
export function getMonitorLog(id: string): MonitorLine[] {
  const inst = monitors.get(id)
  return inst ? inst.ring.toArray() : []
}
