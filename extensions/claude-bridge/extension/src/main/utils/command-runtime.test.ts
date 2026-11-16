import { spawnSync } from 'node:child_process'
import { describe, expect, it } from 'vitest'
import { findBashExecutable, resolveHookShell, toPosixShellPath, whereOnPath, type RuntimeLookupOptions } from './command-runtime'

function windowsLookup(existing: string[]): RuntimeLookupOptions {
  const paths = new Set(existing.map((value) => value.toLowerCase()))
  return {
    platform: 'win32',
    env: {
      ProgramFiles: 'C:\\Program Files',
      'ProgramFiles(x86)': 'C:\\Program Files (x86)',
      LOCALAPPDATA: 'C:\\Users\\dev\\AppData\\Local',
      SystemRoot: 'C:\\Windows',
    },
    exists: (candidate) => paths.has(candidate.toLowerCase()),
    which: async () => null,
  }
}

describe('cross-platform command runtime', () => {
  it('finds a per-user Git Bash installation', async () => {
    const bash = 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\bin\\bash.exe'
    await expect(findBashExecutable(windowsLookup([bash]))).resolves.toBe(bash)
  })

  it('never treats the WSL launcher as Git Bash', async () => {
    const lookup = windowsLookup(['C:\\Windows\\System32\\bash.exe'])
    await expect(findBashExecutable(lookup)).rejects.toThrow('System32 bash.exe WSL launcher is not supported')
  })

  it('falls back to Windows PowerShell only for an unspecified shell', async () => {
    const powershell = 'C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe'
    const lookup = windowsLookup([powershell])
    await expect(resolveHookShell(undefined, lookup)).resolves.toMatchObject({
      bin: powershell,
      kind: 'powershell',
    })
    await expect(resolveHookShell('bash', lookup)).rejects.toThrow('Git Bash was not found')
  })

  it('normalizes Windows paths for Git Bash without invoking WSL', () => {
    expect(toPosixShellPath('C:\\Users\\dev\\Plugin Root', 'win32')).toBe('C:/Users/dev/Plugin Root')
    expect(toPosixShellPath('\\\\server\\share\\hooks', 'win32')).toBe('//server/share/hooks')
  })
})

describe('whereOnPath', () => {
  const tree = (files: Record<string, string[]>) => async (dir: string) => {
    const hit = Object.entries(files).find(([d]) => d.toLowerCase() === dir.toLowerCase())
    if (!hit) throw new Error('ENOENT')
    return hit[1]
  }
  const env = { Path: 'C:\\a;"C:\\b c";;C:\\d\\;C:\\missing', PATHEXT: '.COM;.EXE;.CMD' }

  it('searches the current directory, then PATH in order, with PATHEXT in order', async () => {
    expect(await whereOnPath('git', env, tree({ 'C:\\d\\': ['git.exe'], 'C:\\b c': ['git.cmd'] }), 'C:\\w')).toBe('C:\\b c\\git.cmd')
    expect(await whereOnPath('git', env, tree({ 'C:\\b c': ['git.cmd', 'git.exe'] }), 'C:\\w')).toBe('C:\\b c\\git.exe')
    expect(await whereOnPath('git', env, tree({ 'C:\\w': ['git.com'], 'C:\\a': ['git.exe'] }), 'C:\\w')).toBe('C:\\w\\git.com')
    expect(await whereOnPath('git', env, tree({ 'C:\\d\\': ['git.exe'] }), 'C:\\w')).toBe('C:\\d\\git.exe')
  })

  it('keeps the case of the file on disk and matches names case-insensitively', async () => {
    expect(await whereOnPath('Git', env, tree({ 'C:\\a': ['GIT.EXE'] }), 'C:\\w')).toBe('C:\\a\\GIT.EXE')
  })

  it('tries a name that already has an extension as is, and never a bare name', async () => {
    expect(await whereOnPath('git.exe', env, tree({ 'C:\\a': ['git.exe'] }), 'C:\\w')).toBe('C:\\a\\git.exe')
    expect(await whereOnPath('git', env, tree({ 'C:\\a': ['git'] }), 'C:\\w')).toBeNull()
  })

  it('refuses paths and patterns', async () => {
    const all = async () => ['git.exe', 'g*t.exe']
    expect(await whereOnPath('C:\\a\\git', env, all, 'C:\\w')).toBeNull()
    expect(await whereOnPath('g*t', env, all, 'C:\\w')).toBeNull()
    expect(await whereOnPath('', env, all, 'C:\\w')).toBeNull()
  })

  it.runIf(process.platform === 'win32')('prints what where.exe prints first on this machine', async () => {
    for (const command of ['git', 'node', 'cmd', 'powershell', 'pwsh', 'where', 'no-such-command-kamin']) {
      const r = spawnSync('where', [command], { encoding: 'utf8', windowsHide: true })
      const expected = r.status === 0 ? r.stdout.trim().split(/\r?\n/)[0]! : null
      expect((await whereOnPath(command))?.toLowerCase() ?? null, command).toBe(expected?.toLowerCase() ?? null)
    }
  })
})