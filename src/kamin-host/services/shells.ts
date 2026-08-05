// Shell-profile discovery — same recipe Windows Terminal uses to build
// its "+" dropdown:
//   1. A handful of static built-ins per OS.
//   2. PATH lookups (`where` / `which`).
//   3. Well-known install locations (Program Files / etc).
//   4. WSL distro list on Windows (`wsl -l -q`).
//
// Cached for the lifetime of the host process — the discovery walk
// touches the FS and spawns several child processes (slow on corp
// machines), no point re-running on every "+ new terminal" click.
//
// Ported 1:1 from the legacy Bridge desktop client's shell discovery.

import { execFile } from "node:child_process"
import { existsSync, readFileSync } from "node:fs"
import os from "node:os"
import path from "node:path"
import { promisify } from "node:util"

// ASYNC child-process spawn. discoverShells() runs `where`/`which`/`wsl -l` to
// enumerate shells; these are slow on corp machines (cold WSL alone can take
// seconds). The former `execFileSync` blocked the ENTIRE host event loop for
// that whole time, stalling every other IPC the renderer awaited (listDir,
// doc-sync, streaming forwarding) → the UI froze ("Not Responding"). execFile is
// non-blocking: the discovery still takes a moment but the event loop keeps
// servicing other IPC meanwhile.
const execFileAsync = promisify(execFile)

export interface ShellProfile {
  id: string
  label: string
  command: string
  args: string[]
  icon?: string
}

const WHICH_TIMEOUT_MS = 1500
const WSL_LIST_TIMEOUT_MS = 2000

function safeExists(p: string): boolean {
  try { return existsSync(p) } catch { return false }
}

async function whichWindows(exe: string): Promise<string | null> {
  try {
    const { stdout } = await execFileAsync("where", [exe], {
      encoding: "utf8",
      timeout: WHICH_TIMEOUT_MS,
      windowsHide: true,
    })
    const first = stdout.trim().split(/\r?\n/)[0]
    return first && safeExists(first) ? first : null
  } catch { return null }
}

async function whichUnix(exe: string): Promise<string | null> {
  try {
    const { stdout } = await execFileAsync("which", [exe], {
      encoding: "utf8",
      timeout: WHICH_TIMEOUT_MS,
    })
    const out = stdout.trim()
    return out && safeExists(out) ? out : null
  } catch { return null }
}

function which(exe: string): Promise<string | null> {
  return process.platform === "win32" ? whichWindows(exe) : whichUnix(exe)
}

async function discoverWindows(): Promise<ShellProfile[]> {
  const out: ShellProfile[] = []
  const sysRoot = process.env.SystemRoot ?? "C:\\Windows"
  const programFiles = process.env.ProgramFiles ?? "C:\\Program Files"
  const programFilesX86 = process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)"
  const localAppData = process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local")

  const ps5 = path.join(sysRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe")
  if (safeExists(ps5)) {
    out.push({ id: "powershell", label: "Windows PowerShell", command: ps5, args: ["-NoLogo"], icon: "terminal-powershell" })
  }

  const pwshCandidates: string[] = [
    (await which("pwsh")) ?? "",
    path.join(programFiles, "PowerShell", "7", "pwsh.exe"),
    path.join(programFilesX86, "PowerShell", "7", "pwsh.exe"),
  ].filter((p) => p.length > 0 && safeExists(p))
  if (pwshCandidates[0]) {
    out.push({ id: "pwsh", label: "PowerShell 7", command: pwshCandidates[0], args: ["-NoLogo"], icon: "terminal-powershell" })
  }

  const cmd = path.join(sysRoot, "System32", "cmd.exe")
  if (safeExists(cmd)) out.push({ id: "cmd", label: "Command Prompt", command: cmd, args: [], icon: "terminal-cmd" })

  const gitBashCandidates = [
    path.join(programFiles, "Git", "bin", "bash.exe"),
    path.join(programFilesX86, "Git", "bin", "bash.exe"),
    path.join(localAppData, "Programs", "Git", "bin", "bash.exe"),
  ].filter(safeExists)
  if (gitBashCandidates[0]) {
    out.push({ id: "git-bash", label: "Git Bash", command: gitBashCandidates[0], args: ["--login", "-i"], icon: "terminal-bash" })
  }

  const wslExe = path.join(sysRoot, "System32", "wsl.exe")
  if (safeExists(wslExe)) {
    try {
      const { stdout: raw } = await execFileAsync(wslExe, ["-l", "-q"], {
        encoding: "utf16le",
        timeout: WSL_LIST_TIMEOUT_MS,
        windowsHide: true,
      })
      const distros = raw.split(/\r?\n/).map((s) => s.replace(/ /g, "").trim()).filter(Boolean)
      for (const d of distros) {
        out.push({ id: `wsl-${d}`, label: d, command: wslExe, args: ["-d", d], icon: "terminal-linux" })
      }
    } catch { /* WSL not installed or no distros */ }
  }

  return out
}

async function discoverPosix(): Promise<ShellProfile[]> {
  const seen = new Set<string>()
  try {
    const raw = readFileSync("/etc/shells", "utf8")
    for (const line of raw.split("\n")) {
      const t = line.trim()
      if (!t || t.startsWith("#")) continue
      if (!safeExists(t)) continue
      seen.add(t)
    }
  } catch { /* missing on some macOS configs */ }
  if (process.env.SHELL && safeExists(process.env.SHELL)) seen.add(process.env.SHELL)
  // Resolve the candidate shells concurrently — non-blocking + faster than the
  // former sequential sync `which` calls.
  const resolved = await Promise.all(["bash", "zsh", "fish", "sh", "dash", "ksh"].map((n) => which(n)))
  for (const w of resolved) { if (w) seen.add(w) }
  const out: ShellProfile[] = []
  for (const cmd of seen) {
    const name = path.basename(cmd)
    const label = name.charAt(0).toUpperCase() + name.slice(1)
    out.push({ id: name, label, command: cmd, args: [], icon: "fas fa-terminal" })
  }
  return out
}

// Cache the resolved list (or the in-flight promise) so concurrent + repeat
// `kamin:shells:list` calls share one discovery walk instead of each spawning
// their own child processes.
let cached: Promise<ShellProfile[]> | null = null

export function discoverShells(): Promise<ShellProfile[]> {
  cached ??= (process.platform === "win32" ? discoverWindows() : discoverPosix())
  return cached
}
