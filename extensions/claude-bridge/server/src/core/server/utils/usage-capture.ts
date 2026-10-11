import path from 'node:path'
import fs from 'node:fs'
import os from 'node:os'
import headless from '@xterm/headless'
import { devCli } from '../../pty/dev-cli'
import { injectProxyEnv } from '../../config/settings'
import { emptyPlanUsage, combinePlanUsage, parseUsageScreen } from './plan-usage'
import { createUsageProbe, quoteCaptureArgument } from './usage-statusline'
import type { PlanUsageData } from '../../../shared/plan-usage'

export type UsageData = PlanUsageData
const CACHE_TTL_MS = 30_000
const MAX_CAPTURE_BYTES = 1024 * 1024
let cachedUsage: UsageData | null = null
let cacheTime = 0
let inFlight: Promise<UsageData> | null = null

function findClaude(): string {
  if (process.platform !== 'win32') return 'claude'
  const command = path.join(process.env.APPDATA || '', 'npm', 'claude.cmd')
  return fs.existsSync(command) ? command : 'claude.cmd'
}
async function captureSnapshot(): Promise<UsageData> {
  const attemptedAt = new Date().toISOString()
  const probe = await createUsageProbe()
  const terminal = new headless.Terminal({ cols: 120, rows: 80, scrollback: 0, allowProposedApi: true })
  let stop = () => {}
  try {
    const pty = await import('node-pty')
    // The guarded checkout-only path shares the capture pipeline. An invalid
    // fake configuration must fail closed, never fall back to an installed CLI.
    const developmentCli = devCli()
    const env: Record<string, string> = {}
    for (const [name, value] of Object.entries(process.env))
      if (value !== undefined && name !== 'CLAUDECODE') env[name] = value
    injectProxyEnv(env)
    const command = `${quoteCaptureArgument(findClaude())} --dangerously-skip-permissions --settings ${quoteCaptureArgument(probe.settings)} /usage`
    const shell = process.platform === 'win32' ? 'cmd.exe' : 'bash'
    const args = process.platform === 'win32' ? ['/d', '/v:off', '/s', '/c', command] : ['-lc', `exec ${command}`]
    const proc = pty.spawn(
      developmentCli?.command ?? shell,
      developmentCli
        ? [...developmentCli.prefix, '--dangerously-skip-permissions', '--settings', probe.settings, '/usage']
        : args,
      { name: 'xterm-256color', cols: 120, rows: 80, cwd: os.homedir(), env },
    )
    let stopped = false
    stop = () => {
      if (stopped) return
      stopped = true
      try {
        proc.kill()
      } catch {
        /* already exited */
      }
    }
    return await new Promise<UsageData>((resolve) => {
      let done = false
      let bytes = 0
      let limited = false
      let observedAt = attemptedAt
      let renderTimer: ReturnType<typeof setTimeout> | undefined
      let hardTimer: ReturnType<typeof setTimeout> | undefined
      let dataSub: { dispose(): void } | undefined
      let exitSub: { dispose(): void } | undefined
      const finish = (reason?: string) => {
        if (done) return
        done = true
        if (renderTimer) clearTimeout(renderTimer)
        if (hardTimer) clearTimeout(hardTimer)
        try {
          dataSub?.dispose()
        } catch {
          /* best effort */
        }
        try {
          exitSub?.dispose()
        } catch {
          /* best effort */
        }
        stop()
        terminal.write('', () => {
          const lines: string[] = []
          for (let row = 0; row < terminal.rows; row++)
            lines.push(terminal.buffer.active.getLine(row)?.translateToString(true) ?? '')
          const result = parseUsageScreen(lines.join('\n'), observedAt)
          result.attemptedAt = attemptedAt
          result.diagnostics.outputLimited = limited
          if (reason) result.reason = reason
          void probe
            .read()
            .then((structured) => resolve(combinePlanUsage(result, structured, cachedUsage)))
            .catch(() => resolve(combinePlanUsage(emptyPlanUsage('capture-error', attemptedAt), null, cachedUsage)))
        })
      }
      dataSub = proc.onData((chunk: string) => {
        if (done) return
        bytes += Buffer.byteLength(chunk)
        if (bytes > MAX_CAPTURE_BYTES) {
          limited = true
          finish('output-limit')
          return
        }
        observedAt = new Date().toISOString()
        terminal.write(chunk)
      })
      exitSub = proc.onExit(() => finish())
      renderTimer = setTimeout(() => finish(), 8000)
      hardTimer = setTimeout(() => finish('capture-timeout'), 12_000)
    })
  } catch {
    return combinePlanUsage(emptyPlanUsage('capture-error', attemptedAt), null, cachedUsage)
  } finally {
    stop()
    terminal.dispose()
    await probe.cleanup()
  }
}

export async function captureUsage(forceRefresh = false): Promise<UsageData> {
  if (!forceRefresh && cachedUsage && Date.now() - cacheTime < CACHE_TTL_MS) return structuredClone(cachedUsage)
  if (!inFlight) {
    inFlight = captureSnapshot()
      .catch(() => combinePlanUsage(emptyPlanUsage('capture-error'), null, cachedUsage))
      .then((data) => {
        cachedUsage = data
        cacheTime = Date.now()
        return data
      })
      .finally(() => {
        inFlight = null
      })
  }
  return structuredClone(await inFlight)
}
