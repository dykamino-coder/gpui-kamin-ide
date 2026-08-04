// Plugin monitors — long-running background processes declared by plugins.
//
// A plugin can ship `monitors` in its manifest:
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
// `on-skill-invoke:*` is NOT yet implemented — we don't have a signal from
// Claude CLI about which skill it's invoking, so we can't start monitors on
// demand. Those declarations are ignored for now (logged as skipped).

import fs from 'fs'
import os from 'os'
import path from 'path'
import { spawn, type ChildProcess } from 'child_process'
import type { BrowserWindow } from 'electron'
import {
  listEnabledInstalledPlugins,
  loadPluginOptions,
  substituteUserConfig,
  readPluginManifest,
} from './plugin-helpers'
import { CircularBuffer } from './utils/circular-buffer'

const RING_CAP = 200

export interface MonitorLine { ts: number; stream: 'stdout' | 'stderr' | 'meta'; text: string }

interface MonitorInstance {
  id: string                // "<tabId>/<pluginId>/<monitor.name>"
  tabId: string
  pluginId: string          // "name@marketplace"
  pluginName: string
  monitorName: string
  description: string
  command: string           // resolved
  process?: ChildProcess
  ring: CircularBuffer<MonitorLine>
  startedAt: number
  status: 'running' | 'exited' | 'error'
  exitCode?: number | null
}

/** Live set of running monitors — keyed by composite id so two tabs can
 *  each have their own copy of the same plugin's monitor. */
const monitors = new Map<string, MonitorInstance>()
/** Reverse lookup: every monitor id a given tab owns. */
const tabOwners = new Map<string, Set<string>>()

let mainWindow: BrowserWindow | null = null
export function setMonitorsWindow(w: BrowserWindow | null): void {
  mainWindow = w
}

function broadcast(channel: string, payload: unknown): void {
  if (!mainWindow || mainWindow.isDestroyed()) return
  mainWindow.webContents.send(channel, payload)
}

function appendLine(inst: MonitorInstance, stream: MonitorLine['stream'], text: string): void {
  const entry: MonitorLine = { ts: Date.now(), stream, text }
  inst.ring.add(entry)
  broadcast('monitor:output', { id: inst.id, entry })
}

function substituteAll(str: string, pluginRoot: string, userCfg: Record<string, unknown>, pluginId?: string): string {
  // Resolve in order: user_config → CLAUDE_PLUGIN_ROOT → CLAUDE_PLUGIN_DATA →
  // generic ${VAR}. Matches what parseMcpJson does for MCP commands — same
  // precedence so plugin authors can read one set of rules across all
  // spawn-based features (mcp/lsp/monitors/hooks).
  let out = substituteUserConfig(str, userCfg)
  out = out.replace(/\$\{CLAUDE_PLUGIN_ROOT\}/g, pluginRoot)
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
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    })
    inst.process = proc
    inst.status = 'running'
    appendLine(inst, 'meta', `[monitor] spawned pid=${proc.pid ?? 'n/a'}: ${inst.command}`)

    let stdoutBuf = ''
    proc.stdout?.on('data', (chunk: Buffer) => {
      stdoutBuf += chunk.toString()
      let idx: number
      while ((idx = stdoutBuf.indexOf('\n')) !== -1) {
        const line = stdoutBuf.slice(0, idx).replace(/\r$/, '')
        stdoutBuf = stdoutBuf.slice(idx + 1)
        if (line) appendLine(inst, 'stdout', line)
      }
    })

    let stderrBuf = ''
    proc.stderr?.on('data', (chunk: Buffer) => {
      stderrBuf += chunk.toString()
      let idx: number
      while ((idx = stderrBuf.indexOf('\n')) !== -1) {
        const line = stderrBuf.slice(0, idx).replace(/\r$/, '')
        stderrBuf = stderrBuf.slice(idx + 1)
        if (line) appendLine(inst, 'stderr', line)
      }
    })

    proc.on('error', (err) => {
      appendLine(inst, 'meta', `[monitor] spawn error: ${err.message}`)
      inst.status = 'error'
      broadcast('monitor:status', { id: inst.id, status: inst.status })
    })

    proc.on('exit', (code, signal) => {
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
export async function startMonitorsForTab(tabId: string): Promise<string[]> {
  const plugins = await listEnabledInstalledPlugins()
  const started: string[] = []

  for (const plugin of plugins) {
    const manifest = await readPluginManifest(plugin.installPath)
    if (!manifest) continue
    const rawMonitors = manifest.monitors
    if (!Array.isArray(rawMonitors) || rawMonitors.length === 0) continue

    const userCfg = loadPluginOptions(plugin.key)
    const pluginRoot = plugin.installPath

    for (const m of rawMonitors) {
      if (!m || typeof m !== 'object') continue
      const monitorName = typeof m.name === 'string' ? m.name : ''
      const commandRaw = typeof m.command === 'string' ? m.command : ''
      const description = typeof m.description === 'string' ? m.description : ''
      const when = typeof m.when === 'string' ? m.when : 'always'
      if (!monitorName || !commandRaw) continue
      // Only `always` supported today. `on-skill-invoke:*` would need a
      // signal from CLI about which skill was dispatched — we don't have
      // that yet, so those monitors are declared-but-idle.
      if (when !== 'always' && when !== '') {
        console.log(`[monitors] skipping ${plugin.key}/${monitorName} (when=${when}, not supported yet)`)
        continue
      }

      const id = `${tabId}/${plugin.key}/${monitorName}`
      if (monitors.has(id)) continue  // already running

      const inst: MonitorInstance = {
        id,
        tabId,
        pluginId: plugin.key,
        pluginName: plugin.name,
        monitorName,
        description,
        command: substituteAll(commandRaw, pluginRoot, userCfg, plugin.key),
        ring: new CircularBuffer<MonitorLine>(RING_CAP),
        startedAt: Date.now(),
        status: 'running',
      }
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
  if (!owned) return
  for (const id of owned) {
    const inst = monitors.get(id)
    if (!inst) continue
    if (inst.process && !inst.process.killed) {
      try { inst.process.kill() } catch {}
    }
    monitors.delete(id)
    broadcast('monitor:stopped', { id })
  }
  tabOwners.delete(tabId)
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
