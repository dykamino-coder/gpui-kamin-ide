// ============================================================================
// Usage Capture — Spawns `claude /usage` via PTY, parses TUI output
// ============================================================================

import * as path from "path"
import * as fs from "fs"
import * as os from "os"
import { debugLog, warnLog } from "../../logging"
import { injectProxyEnv } from "../../config/settings"

export interface UsageData {
  session: { percent: number; resets: string } | null
  weekAll: { percent: number; resets: string } | null
  weekSonnet: { percent: number; resets: string } | null
  extra: string | null
  timestamp: string
  /** Raw PTY output (only included when all fields are null — for diagnostics) */
  _raw?: string
  _error?: string
}

// Cache to avoid spawning PTY on every request
let cachedUsage: UsageData | null = null
let cacheTime = 0
const CACHE_TTL_MS = 30_000 // 30 seconds

let captureInProgress = false

function stripAnsi(str: string): string {
  return str
    .replace(/\x1b\[[0-9;?]*[a-zA-Z]/g, "")
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b[()][AB012]/g, "")
    .replace(/\x1b[^[\]()][^\x1b]?/g, "")
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, "")
}

function parseUsageText(text: string): UsageData {
  const data: UsageData = {
    session: null,
    weekAll: null,
    weekSonnet: null,
    extra: null,
    timestamp: new Date().toISOString(),
  }

  const sessionMatch = text.match(
    /Current\s*session[^%]*?(\d+)%\s*used[^R]*Rese[ts]*\s*([\d\w\s:(),\/]+?)(?=Current|Extra|Esc)/i
  )
  if (sessionMatch) {
    data.session = { percent: parseInt(sessionMatch[1]!), resets: sessionMatch[2]!.trim() }
  }

  const weekAllMatch = text.match(
    /Current\s*week\s*\(?\s*all\s*models?\s*\)?[^%]*?(\d+)%\s*used[^R]*Resets?\s*([\d\w\s:(),\/]+?)(?=Current|Extra|Esc)/i
  )
  if (weekAllMatch) {
    data.weekAll = { percent: parseInt(weekAllMatch[1]!), resets: weekAllMatch[2]!.trim() }
  }

  const weekSonnetMatch = text.match(
    /Current\s*week\s*\(?\s*Sonnet\s*only\s*\)?[^%]*?(\d+)%\s*used[^R]*Resets?\s*([\d\w\s:(),\/]+?)(?=Current|Extra|Esc)/i
  )
  if (weekSonnetMatch) {
    data.weekSonnet = { percent: parseInt(weekSonnetMatch[1]!), resets: weekSonnetMatch[2]!.trim() }
  }

  const extraMatch = text.match(/Extra\s*usage\s*(.*?)(?=Esc|$)/i)
  if (extraMatch) {
    const extraText = extraMatch[1]!.trim()
    data.extra = /not\s*enabled/i.test(extraText) ? "not enabled" : extraText
  }

  // Detect CLI-level errors (e.g. "Failed to load usage data")
  const errorMatch = text.match(/Error:\s*(.+?)(?:\r|$)/i)
  if (errorMatch && !data.session && !data.weekAll && !data.weekSonnet) {
    data._error = errorMatch[1]!.trim()
  }

  // Include raw output when parsing yields nothing — helps diagnose regex mismatches
  if (!data.session && !data.weekAll && !data.weekSonnet && text.trim().length > 0) {
    data._raw = text.slice(0, 2000)
  }

  return data
}

function findClaude(): string {
  if (process.platform === "win32") {
    const npmCmd = path.join(process.env.APPDATA || "", "npm", "claude.cmd")
    if (fs.existsSync(npmCmd)) return npmCmd
    return "claude.cmd"
  }
  return "claude"
}

export async function captureUsage(forceRefresh = false): Promise<UsageData> {
  // Return cache if fresh
  if (!forceRefresh && cachedUsage && Date.now() - cacheTime < CACHE_TTL_MS) {
    return cachedUsage
  }

  // Prevent concurrent captures
  if (captureInProgress) {
    if (cachedUsage) return cachedUsage
    // Wait for in-progress capture
    return new Promise((resolve) => {
      const interval = setInterval(() => {
        if (!captureInProgress && cachedUsage) {
          clearInterval(interval)
          resolve(cachedUsage)
        }
      }, 500)
      // Timeout after 15s
      setTimeout(() => { clearInterval(interval); resolve(cachedUsage || emptyUsage()) }, 15_000)
    })
  }

  captureInProgress = true

  try {
    const pty = await import("node-pty")
    const claudePath = findClaude()
    debugLog("Usage capture: spawning", { claudePath })

    return await new Promise<UsageData>((resolve) => {
      let buffer = ""
      let resolved = false

      // Spawn through bash login shell — matches how the terminal runs commands.
      // Direct spawn misses .bashrc/.profile setup that claude may need.
      const cleanEnv: Record<string, string> = {}
      for (const [k, v] of Object.entries(process.env)) {
        if (v !== undefined && k !== 'CLAUDECODE') cleanEnv[k] = v
      }
      injectProxyEnv(cleanEnv)

      const shell = process.platform === 'win32' ? 'cmd.exe' : 'bash'
      const shellArgs = process.platform === 'win32'
        ? ['/c', `${claudePath} --dangerously-skip-permissions /usage`]
        : ['-lc', `${claudePath} --dangerously-skip-permissions /usage`]

      const proc = pty.spawn(shell, shellArgs, {
        name: "xterm-256color",
        cols: 120,
        rows: 50,
        cwd: os.homedir(),
        env: cleanEnv as any,
      })

      proc.onData((data: string) => {
        buffer += data
      })

      const finish = () => {
        if (resolved) return
        resolved = true
        const clean = stripAnsi(buffer)
        const data = parseUsageText(clean)
        debugLog("Usage capture: parsed", data)
        cachedUsage = data
        cacheTime = Date.now()
        captureInProgress = false

        // Kill process
        try {
          proc.write("\x1b")
          setTimeout(() => {
            try { proc.write("/exit\r") } catch {}
            setTimeout(() => {
              try { proc.kill() } catch {}
            }, 2000)
          }, 300)
        } catch {}

        resolve(data)
      }

      proc.onExit(() => finish())
      // Wait for TUI to render
      setTimeout(finish, 8000)
      // Hard timeout
      setTimeout(() => {
        if (!resolved) {
          warnLog("Usage capture: hard timeout")
          resolved = true
          captureInProgress = false
          try { proc.kill() } catch {}
          resolve(cachedUsage || emptyUsage())
        }
      }, 12_000)
    })
  } catch (err) {
    captureInProgress = false
    const msg = err instanceof Error ? err.message : "Unknown"
    warnLog("Usage capture failed", { error: msg })
    const result = cachedUsage || emptyUsage()
    result._error = msg
    return result
  }
}

function emptyUsage(): UsageData {
  return { session: null, weekAll: null, weekSonnet: null, extra: null, timestamp: new Date().toISOString() }
}
