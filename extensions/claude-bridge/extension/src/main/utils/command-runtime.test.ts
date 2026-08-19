import { describe, expect, it } from 'vitest'
import { findBashExecutable, resolveHookShell, toPosixShellPath, type RuntimeLookupOptions } from './command-runtime'

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
