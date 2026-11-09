// Shared PTY ownership for the native host and the browser workspace host.
import { execFile } from "node:child_process"
import { existsSync } from "node:fs"
import { homedir } from "node:os"
import * as pty from "@homebridge/node-pty-prebuilt-multiarch"
import { discoverShells, type ShellProfile } from "./shells.js"

const DEFAULT_COLS = 80
const DEFAULT_ROWS = 24
const TASKKILL_TIMEOUT_MS = 3000

export interface PtyCreateOpts {
  cwd?: string | null
  shellId?: string | null
  cols?: number
  rows?: number
}

export interface PtyEvents {
  onData: (ptyId: string, data: string) => void
  onExit: (ptyId: string, code: number, signal?: number) => void
}

export function pickProfile(profiles: ShellProfile[], id?: string | null): ShellProfile | null {
  if (id) {
    const found = profiles.find(p => p.id === id)
    if (found) return found
  }
  const order = process.platform === "win32"
    ? ["pwsh", "powershell", "git-bash", "cmd"] : ["bash", "zsh", "fish", "sh"]
  return order.map(wanted => profiles.find(p => p.id === wanted)).find(Boolean) ?? profiles[0] ?? null
}

export class PtySessions {
  private readonly handles = new Map<string, pty.IPty>()
  private nextId = 1

  constructor(
    private readonly events: PtyEvents,
    private readonly environment: () => NodeJS.ProcessEnv = () => process.env,
    private readonly useConptyDll: () => boolean = () => false,
  ) {}

  async createSession(opts: PtyCreateOpts): Promise<string> {
    const profile = pickProfile(await discoverShells(), opts.shellId)
    const command = profile?.command ?? (process.platform === "win32" ? "cmd.exe" : process.env.SHELL ?? "/bin/bash")
    const ptyId = `pty-${(this.nextId++).toString()}`
    const ptyOpts: Record<string, unknown> = {
      name: "xterm-color", cols: opts.cols ?? DEFAULT_COLS, rows: opts.rows ?? DEFAULT_ROWS,
      cwd: opts.cwd && existsSync(opts.cwd) ? opts.cwd : homedir(), env: this.environment(),
    }
    if (process.platform === "win32") ptyOpts.useConptyDll = this.useConptyDll()
    const proc = pty.spawn(command, profile?.args ?? [], ptyOpts)
    this.handles.set(ptyId, proc)
    proc.onData(data => { this.events.onData(ptyId, data) })
    proc.onExit(info => {
      // node-pty's Windows output worker also needs disposal on natural exit.
      // A requested kill has already taken ownership of that cleanup below.
      if (process.platform === "win32" && this.handles.has(ptyId)) {
        try { proc.kill() } catch { /* already closed */ }
      }
      this.handles.delete(ptyId)
      this.events.onExit(ptyId, info.exitCode, info.signal)
    })
    return ptyId
  }

  writeToSession(id: string, data: string): void { this.handles.get(id)?.write(data) }

  resizeSession(id: string, cols: number, rows: number): void {
    if (cols >= 1 && rows >= 1) this.handles.get(id)?.resize(cols, rows)
  }

  disposeSession(id: string): void {
    const proc = this.handles.get(id)
    if (!proc) return
    this.handles.delete(id)
    // Terminate the owned process tree before releasing its ConPTY parent.
    // Otherwise taskkill may enumerate after the shell PID has disappeared.
    if (process.platform === "win32") {
      execFile("taskkill", ["/F", "/T", "/PID", String(proc.pid)],
        { windowsHide: true, timeout: TASKKILL_TIMEOUT_MS }, () => {
          try { proc.kill() } catch { /* already exited */ }
        })
    } else {
      try { process.kill(-proc.pid, "SIGTERM") } catch { /* group already exited */ }
      try { proc.kill() } catch { /* already exited */ }
    }
  }

  disposeAllSessions(): void {
    for (const id of [...this.handles.keys()]) this.disposeSession(id)
  }
}
