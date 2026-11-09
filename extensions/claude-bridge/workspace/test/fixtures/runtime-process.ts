import assert from 'node:assert/strict'
import { fork, spawnSync } from 'node:child_process'
import { once } from 'node:events'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export async function startRuntimeProcess(root: string, tmp: string, home: string, upstreamUrl: string,
  cleanup: Array<() => void | Promise<void>>) {
  const child = fork(fileURLToPath(new URL('./runtime-child.cjs', import.meta.url)), [root, upstreamUrl], {
    cwd: tmp, execArgv: [], stdio: ['ignore', 'pipe', 'pipe', 'ipc'],
    env: { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot, ComSpec: process.env.ComSpec,
      ProgramFiles: process.env.ProgramFiles, CLAUDE_CODE_GIT_BASH_PATH: process.env.CLAUDE_CODE_GIT_BASH_PATH,
      HOME: home, USERPROFILE: home, TEMP: tmp, TMP: tmp, TMPDIR: tmp,
      KAMIN_CACHE_DIR: path.join(root, 'cache'), KAMIN_DISABLE_EXTERNAL_MCP: '1' },
  })
  // Keep output private; assertion messages never dump runtime logs/credentials.
  let output = ''
  child.stdout!.on('data', chunk => { output += chunk.toString() })
  child.stderr!.on('data', chunk => { output += chunk.toString() })
  cleanup.push(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
      else child.kill()
      await once(child, 'exit', { signal: AbortSignal.timeout(5000) })
    }
  })
  const exited = once(child, 'exit')
  const [ready] = await once(child, 'message', { signal: AbortSignal.timeout(10000) }) as [{ origin: string }]
  assert.ok(ready.origin)
  return { child, exited, ready, runtimeOutput: () => output }
}
