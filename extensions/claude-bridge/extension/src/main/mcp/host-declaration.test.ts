import { describe, expect, it } from 'vitest'
import { probeHostDeclaration, USER_MACHINE_TRAITS, type HostTraits } from './host-declaration'
import type { RuntimeLookupOptions } from '../utils/command-runtime'

// Registry order; the registry itself needs the extension host to load.
const REGISTERED = ['Read', 'Write', 'Edit', 'Glob', 'Grep', 'Bash', 'PowerShell', 'BashOutput', 'KillShell', 'NotebookEdit', 'WebFetch',
  'EnterWorktree', 'ExitWorktree', 'AskUserQuestion', 'PushNotification', 'ShowWidget', 'preview_mount', 'preview_update',
  'preview_unmount', 'TodoWrite', 'LspDiagnostics', 'LspHover']
const WORKSPACE: HostTraits = { place: 'the workspace host', preview: true }
const PREVIEW = ['preview_mount', 'preview_update', 'preview_unmount']

function posix(platform: NodeJS.Platform, installed: string[]): RuntimeLookupOptions {
  return { platform, env: {}, exists: () => false, which: async (command) => (installed.includes(command) ? `/usr/bin/${command}` : null) }
}
function windows(existing: string[], installed: string[] = []): RuntimeLookupOptions {
  const paths = new Set(existing.map((value) => value.toLowerCase()))
  return {
    platform: 'win32',
    env: { ProgramFiles: 'C:\\Program Files', 'ProgramFiles(x86)': 'C:\\Program Files (x86)', LOCALAPPDATA: 'C:\\Users\\dev\\AppData\\Local', SystemRoot: 'C:\\Windows' },
    exists: (candidate) => paths.has(candidate.toLowerCase()),
    which: async (command) => (installed.includes(command) ? `C:\\tools\\${command}.exe` : null),
  }
}
const absent = (tools: string[], names: string[]) => names.filter((name) => tools.includes(name))

describe('host declaration probe', () => {
  it('does not declare PowerShell on a Linux host without pwsh', async () => {
    const host = await probeHostDeclaration(REGISTERED, WORKSPACE, posix('linux', ['bash', 'git']))
    expect(host).toMatchObject({ platform: 'linux', place: 'the workspace host' })
    expect(host.tools).toEqual(REGISTERED.filter((name) => name !== 'PowerShell'))
  })

  it('declares PowerShell on Linux only when pwsh is installed', async () => {
    const host = await probeHostDeclaration(REGISTERED, WORKSPACE, posix('linux', ['bash', 'git', 'pwsh']))
    expect(host.tools).toEqual(REGISTERED)
  })

  it('declares both shells on Windows with Git Bash and no preview tools for the IDE', async () => {
    const bash = 'C:\\Program Files\\Git\\bin\\bash.exe'
    const host = await probeHostDeclaration(REGISTERED, USER_MACHINE_TRAITS, windows([bash], ['pwsh', 'git']))
    expect(host).toMatchObject({ platform: 'win32', place: "the user's machine" })
    expect(host.tools).toEqual(REGISTERED.filter((name) => !PREVIEW.includes(name)))
  })

  it('does not declare Bash on Windows without Git Bash', async () => {
    const host = await probeHostDeclaration(REGISTERED, USER_MACHINE_TRAITS, windows([], ['pwsh', 'git']))
    expect(absent(host.tools, ['Bash'])).toEqual([])
    expect(host.tools).toEqual(expect.arrayContaining(['PowerShell', 'BashOutput', 'KillShell']))
  })

  it('declares no shell, background-shell or worktree tools on a host without shell and git', async () => {
    const host = await probeHostDeclaration(REGISTERED, { place: 'the review host', preview: false }, posix('linux', []))
    expect(absent(host.tools, ['Bash', 'PowerShell', 'BashOutput', 'KillShell', 'EnterWorktree', 'ExitWorktree', ...PREVIEW])).toEqual([])
    expect(host.tools).toEqual(expect.arrayContaining(['Read', 'Write', 'Edit', 'Glob', 'Grep', 'WebFetch', 'LspHover']))
  })
})
