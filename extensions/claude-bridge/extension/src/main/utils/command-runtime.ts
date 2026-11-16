import { spawn } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

export interface RuntimeLookupOptions {
  platform?: NodeJS.Platform
  env?: NodeJS.ProcessEnv
  exists?: (candidate: string) => boolean
  which?: (command: string) => Promise<string | null>
}

export interface ResolvedShell {
  bin: string
  args: string[]
  kind: 'bash' | 'powershell' | 'sh' | 'zsh'
}

/** What `where <command>` prints first, found without starting it: the current
 *  directory, then each `PATH` entry in order, trying the `PATHEXT` extensions
 *  in order (a command that already has one of them is also tried as is).
 *
 *  Starting `where.exe` took 130–250 ms per lookup with a 70-entry `PATH`, and
 *  a slow start hit its 5 s cap, which the first session waited for. One stat
 *  per candidate is no better on such a machine (~2.5 ms each under file
 *  system filters), so each directory is listed once, all in parallel
 *  (~5 ms for 70 entries), and the names are matched in memory. */
export async function whereOnPath(
  command: string,
  env: NodeJS.ProcessEnv = process.env,
  list: (dir: string) => Promise<string[]> = listFiles,
  cwd: string = process.cwd(),
): Promise<string | null> {
  if (!command || /[\\/:*?"<>|]/.test(command)) return null
  const pathext = (envValue(env, 'PATHEXT') ?? '.COM;.EXE;.BAT;.CMD')
    .split(';')
    .map((ext) => ext.trim().toLowerCase())
    .filter(Boolean)
  const lower = command.toLowerCase()
  const names = [...(pathext.some((ext) => lower.endsWith(ext)) ? [lower] : []), ...pathext.map((ext) => lower + ext)]
  const dirs = [cwd, ...(envValue(env, 'PATH') ?? '').split(';')]
    .map((dir) => dir.trim().replace(/^"(.*)"$/, '$1'))
    .filter(Boolean)
  const listings = await Promise.all(dirs.map((dir) => list(dir).catch(() => [] as string[])))
  for (let i = 0; i < dirs.length; i++) {
    const byName = new Map(listings[i]!.map((name) => [name.toLowerCase(), name]))
    for (const name of names) {
      const actual = byName.get(name)
      if (actual !== undefined) return path.win32.join(dirs[i]!, actual)
    }
  }
  return null
}

/** Windows environment names are case-insensitive (`Path` vs `PATH`). */
function envValue(env: NodeJS.ProcessEnv, name: string): string | undefined {
  if (env[name] !== undefined) return env[name]
  const key = Object.keys(env).find((k) => k.toUpperCase() === name)
  return key === undefined ? undefined : env[key]
}

/** Names of the files (and links) in `dir`; `where` never prints a directory. */
async function listFiles(dir: string): Promise<string[]> {
  const entries = await fs.promises.readdir(dir, { withFileTypes: true })
  return entries.filter((e) => !e.isDirectory()).map((e) => e.name)
}
export function whichExecutable(command: string, platform: NodeJS.Platform = process.platform, timeoutMs = 5000): Promise<string | null> {
  if (platform === 'win32' && process.platform === 'win32') return whereOnPath(command)
  return new Promise((resolve) => {
    const finder = platform === 'win32' ? 'where' : 'which'
    const child = spawn(finder, [command], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
    })
    const chunks: Buffer[] = []
    let settled = false
    const finish = (value: string | null): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve(value)
    }
    const timer = setTimeout(() => {
      try {
        child.kill()
      } catch {
        /* best effort */
      }
      finish(null)
    }, timeoutMs)
    child.stdout.on('data', (chunk: Buffer) => chunks.push(chunk))
    child.on('error', () => finish(null))
    child.on('close', (code) => {
      if (code !== 0) return finish(null)
      const first = Buffer.concat(chunks).toString('utf8').trim().split(/\r?\n/)[0]
      finish(first || null)
    })
  })
}

function lookupOptions(options: RuntimeLookupOptions): Required<Pick<RuntimeLookupOptions, 'platform' | 'env' | 'exists' | 'which'>> {
  const platform = options.platform ?? process.platform
  return {
    platform,
    env: options.env ?? process.env,
    exists: options.exists ?? fs.existsSync,
    which: options.which ?? ((command: string) => whichExecutable(command, platform)),
  }
}

/** Resolve Git for Windows Bash without ever accepting System32\\bash.exe,
 * which is the WSL launcher and fails on machines without a WSL distro. */
/** The host's own lookups are cached: the install does not change under a running host. */
const defaultLookups = new Map<string, Promise<string>>()
function cachedLookup(key: string, options: RuntimeLookupOptions, find: () => Promise<string>): Promise<string> {
  if (Object.keys(options).length > 0) return find()
  let found = defaultLookups.get(key)
  if (!found) {
    found = find()
    defaultLookups.set(key, found)
    found.catch(() => defaultLookups.delete(key))
  }
  return found
}

export function findBashExecutable(options: RuntimeLookupOptions = {}): Promise<string> {
  return cachedLookup('bash', options, () => lookupBash(options))
}

async function lookupBash(options: RuntimeLookupOptions): Promise<string> {
  const { platform, env, exists, which } = lookupOptions(options)
  if (platform !== 'win32') {
    const bash = await which('bash')
    if (bash) return bash
    if (exists('/bin/bash')) return '/bin/bash'
    throw new Error('Bash is not installed or is not available in PATH')
  }

  const configured = env.CLAUDE_CODE_GIT_BASH_PATH
  if (configured && exists(configured)) return configured

  const gitPath = await which('git')
  if (gitPath) {
    const derived = path.win32.resolve(path.win32.dirname(gitPath), '..', 'bin', 'bash.exe')
    if (exists(derived)) return derived
  }

  const candidates = [
    env.ProgramFiles && path.win32.join(env.ProgramFiles, 'Git', 'bin', 'bash.exe'),
    env['ProgramFiles(x86)'] && path.win32.join(env['ProgramFiles(x86)']!, 'Git', 'bin', 'bash.exe'),
    env.LOCALAPPDATA && path.win32.join(env.LOCALAPPDATA, 'Programs', 'Git', 'bin', 'bash.exe'),
  ].filter((candidate): candidate is string => Boolean(candidate))
  for (const candidate of candidates) {
    if (exists(candidate)) return candidate
  }

  throw new Error('Git Bash was not found. Install Git for Windows or set CLAUDE_CODE_GIT_BASH_PATH; ' + 'the Windows System32 bash.exe WSL launcher is not supported.')
}

export function findPowerShellExecutable(options: RuntimeLookupOptions = {}): Promise<string> {
  return cachedLookup('pwsh', options, () => lookupPowerShell(options))
}

async function lookupPowerShell(options: RuntimeLookupOptions): Promise<string> {
  const { platform, env, exists, which } = lookupOptions(options)
  const pwsh = await which('pwsh')
  if (pwsh) return pwsh

  if (platform === 'win32') {
    const candidates = [
      env.ProgramFiles && path.win32.join(env.ProgramFiles, 'PowerShell', '7', 'pwsh.exe'),
      env.SystemRoot && path.win32.join(env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe'),
    ].filter((candidate): candidate is string => Boolean(candidate))
    for (const candidate of candidates) {
      if (exists(candidate)) return candidate
    }
    const legacy = await which('powershell')
    if (legacy) return legacy
    throw new Error('PowerShell was not found (tried pwsh.exe and Windows PowerShell)')
  }

  throw new Error('PowerShell (pwsh) is not installed or is not available in PATH')
}

/** Match Claude Code's documented Windows policy: Git Bash by default, with
 * PowerShell fallback only when the hook did not explicitly require Bash. */
export async function resolveHookShell(requested?: 'bash' | 'powershell' | 'sh' | 'zsh', options: RuntimeLookupOptions = {}): Promise<ResolvedShell> {
  const platform = options.platform ?? process.platform
  if (platform === 'win32') {
    if (requested === 'sh' || requested === 'zsh') {
      throw new Error(`${requested} hooks are not supported natively on Windows; use bash (Git Bash) or powershell`)
    }
    if (requested === 'powershell') {
      return {
        bin: await findPowerShellExecutable(options),
        args: ['-NoProfile', '-NonInteractive', '-Command'],
        kind: 'powershell',
      }
    }
    if (requested === 'bash') {
      return {
        bin: await findBashExecutable(options),
        args: ['-c'],
        kind: 'bash',
      }
    }
    try {
      return {
        bin: await findBashExecutable(options),
        args: ['-c'],
        kind: 'bash',
      }
    } catch {
      return {
        bin: await findPowerShellExecutable(options),
        args: ['-NoProfile', '-NonInteractive', '-Command'],
        kind: 'powershell',
      }
    }
  }

  if (requested === 'powershell') {
    return {
      bin: await findPowerShellExecutable(options),
      args: ['-NoProfile', '-NonInteractive', '-Command'],
      kind: 'powershell',
    }
  }
  const kind = requested ?? 'bash'
  const bin = kind === 'bash' ? await findBashExecutable(options) : ((await (options.which ?? ((command: string) => whichExecutable(command, platform)))(kind)) ?? kind)
  return { bin, args: ['-c'], kind }
}

export function toPosixShellPath(value: string, platform: NodeJS.Platform = process.platform): string {
  if (platform !== 'win32') return value
  if (value.startsWith('\\\\')) return `//${value.slice(2).replace(/\\/g, '/')}`
  return value.replace(/\\/g, '/')
}

export function shellSingleQuote(value: string): string {
  return `'${value.replace(/'/g, `'"'"'`)}'`
}

export function powerShellSingleQuote(value: string): string {
  return `'${value.replace(/'/g, "''")}'`
}
