// PTY service — node-pty running directly inside kamin-host. Merges the
// former pty-host.ts (main-process bridge) + pty-utility.ts (fork) pair:
// kamin-host already IS a window-less Node process, so the original
// reason for the extra fork (`proc.kill()`/taskkill flashing a phantom
// window when spawned from the windowed main process) holds here by
// construction. One process fewer, one hop fewer for terminal data.
//
// Forking from inside a utilityProcess is NOT an option anyway:
// `process.execPath` is the Electron binary and the packaged build
// flips the RunAsNode fuse off.

import { execFile } from "node:child_process"
import { existsSync } from "node:fs"
import { homedir } from "node:os"
import * as pty from "@homebridge/node-pty-prebuilt-multiarch"
import { getAppPrefs } from "./app-prefs.js"
import { applyEnvCollections } from "./env-collection.js"
import { discoverShells, type ShellProfile } from "./shells.js"

export interface PtyCreateOpts {
  /** Workspace folder or any existing directory; falls back to $HOME. */
  cwd?: string | null
  /** Profile id from `discoverShells()`; sane per-platform default. */
  shellId?: string | null
  cols?: number
  rows?: number
}

export interface PtyEvents {
  onData: (ptyId: string, data: string) => void
  onExit: (ptyId: string, code: number, signal?: number) => void
}

const DEFAULT_COLS = 80
const DEFAULT_ROWS = 24
const GRACEFUL_EXIT_DELAY_MS = 250
const TASKKILL_TIMEOUT_MS = 3000

const handles = new Map<string, pty.IPty>()
let nextId = 1
let events: PtyEvents | null = null

/** Called once at host boot — data/exit fan out to the shell transport. */
export function initPtyEvents(ev: PtyEvents): void {
  events = ev
}

function pickProfile(profiles: ShellProfile[], id: string | null | undefined): ShellProfile | null {
  if (id) {
    const found = profiles.find((p) => p.id === id)
    if (found) return found
  }
  const order = process.platform === "win32"
    ? ["pwsh", "powershell", "git-bash", "cmd"]
    : ["bash", "zsh", "fish", "sh"]
  for (const wanted of order) {
    const f = profiles.find((p) => p.id === wanted)
    if (f) return f
  }
  return profiles[0] ?? null
}

export async function createSession(opts: PtyCreateOpts): Promise<string> {
  const profile = pickProfile(await discoverShells(), opts.shellId)
  const command = profile?.command ?? (process.platform === "win32" ? "cmd.exe" : process.env.SHELL ?? "/bin/bash")
  const args = profile?.args ?? []
  const cwd = opts.cwd && existsSync(opts.cwd) ? opts.cwd : homedir()
  const ptyId = `pty-${(nextId++).toString()}`
  const ptyOpts: Record<string, unknown> = {
    name: "xterm-color",
    cols: opts.cols ?? DEFAULT_COLS,
    rows: opts.rows ?? DEFAULT_ROWS,
    cwd,
    // Extensions' environmentVariableCollection mutations (e.g. a venv PATH).
    env: applyEnvCollections(process.env),
  }
  // Bundled ConPTY by default (instant on Win11); the app-pref flips to the
  // Windows-signed system ConPTY for corp AppLocker setups that block the
  // unsigned OpenConsole.exe. Re-open the terminal to apply.
  if (process.platform === "win32") ptyOpts.useConptyDll = getAppPrefs().useConptyDll
  const proc = pty.spawn(command, args, ptyOpts)
  handles.set(ptyId, proc)
  proc.onData((data) => { events?.onData(ptyId, data) })
  proc.onExit((info) => {
    events?.onExit(ptyId, info.exitCode, info.signal)
    handles.delete(ptyId)
  })
  return ptyId
}

export function writeToSession(ptyId: string, data: string): void {
  handles.get(ptyId)?.write(data)
}

export function resizeSession(ptyId: string, cols: number, rows: number): void {
  if (cols < 1 || rows < 1) return
  handles.get(ptyId)?.resize(cols, rows)
}

export function disposeSession(ptyId: string): void {
  const h = handles.get(ptyId)
  if (!h) return
  handles.delete(ptyId)
  // Graceful first — well-behaved shells close themselves.
  try { h.write("\x03") } catch { /* dead */ }
  try { h.write("exit\r") } catch { /* dead */ }
  setTimeout(() => {
    if (process.platform === "win32") {
      // Tree-kill anything the shell spawned (npm dev / watchers).
      execFile(
        "taskkill",
        ["/F", "/T", "/PID", String(h.pid)],
        { windowsHide: true, timeout: TASKKILL_TIMEOUT_MS },
        () => { /* fire-and-forget */ },
      )
      // Tear down node-pty's ConPTY pseudo-console. The kill helper it
      // spawns lives inside this window-less process — no flash.
      try { h.kill() } catch { /* gone */ }
    } else {
      try { process.kill(-h.pid, "SIGTERM") } catch { /* group gone */ }
    }
  }, GRACEFUL_EXIT_DELAY_MS)
}

export function disposeAllSessions(): void {
  for (const id of [...handles.keys()]) disposeSession(id)
}
