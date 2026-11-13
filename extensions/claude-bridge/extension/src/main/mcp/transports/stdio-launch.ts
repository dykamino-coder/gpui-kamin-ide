import nodeFs from 'node:fs'
import nodePath from 'node:path'
import { assertCmdLineFits } from '../../utils/script-file'

function envValue(env: NodeJS.ProcessEnv, name: string): string | undefined {
  const key = Object.keys(env).find((key) => key.toLowerCase() === name.toLowerCase())
  return key === undefined ? undefined : env[key]
}

/** Resolve batch launchers as well as executables, without changing the host cwd. */
function windowsCommand(command: string, cwd: string, env: NodeJS.ProcessEnv): string {
  const directories = /[\\/]/.test(command) ? [''] : [cwd, ...(envValue(env, 'PATH') ?? '').split(';')]
  const extensions = nodePath.win32.extname(command) ? [''] : (envValue(env, 'PATHEXT') ?? '.COM;.EXE;.BAT;.CMD').split(';')
  for (const directory of directories) {
    const base = nodePath.win32.resolve(cwd, directory.replace(/^"(.*)"$/, '$1'), command)
    for (const extension of extensions) {
      const file = base + extension
      try { if (nodeFs.statSync(file).isFile()) return file } catch { /* try the next PATH entry */ }
    }
  }
  return command
}

// Quote one argv element for the Windows C runtime. Batch launchers forward
// these elements through %*, so cmd.exe must also preserve the quotes and
// metacharacters through both the initial /c parse and the forwarding line.
function batchArgument(value: string): string {
  let quoted = '"'
  let slashes = 0
  for (const char of value) {
    if (char === '\\') { slashes++; continue }
    quoted += '\\'.repeat(char === '"' ? slashes * 2 + 1 : slashes) + char
    slashes = 0
  }
  quoted += '\\'.repeat(slashes * 2) + '"'
  return escapeCmd(escapeCmd(quoted))
}

function escapeCmd(value: string): string {
  return value.replace(/[()[\]%!^"`<>&|;, *?]/g, (char) => `^${char}`)
}

export function stdioLaunch(command: string, args: string[], cwd: string, env: NodeJS.ProcessEnv): {
  command: string; args: string[]; windowsVerbatimArguments?: boolean
} {
  if (process.platform !== 'win32') return { command, args }
  const file = windowsCommand(command, cwd, env)
  if (!/\.(cmd|bat)$/i.test(file)) return { command: file, args }
  const shell = envValue(env, 'ComSpec') ?? 'cmd.exe'
  const escapedCommand = escapeCmd(file)
  const escapedArgs = args.map(batchArgument)
  assertCmdLineFits(escapedCommand, escapedArgs, 'win32', shell)
  return {
    command: shell,
    args: ['/d', '/s', '/c', `"${[escapedCommand, ...escapedArgs].join(' ')}"`],
    windowsVerbatimArguments: true,
  }
}
