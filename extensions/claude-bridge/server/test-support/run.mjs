// Launch from a checkout, with all runtime state/logs under an explicit external root.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'
const server = fileURLToPath(new URL('../', import.meta.url))
const root = process.argv[2] && path.resolve(process.argv[2])
if (!root || root === path.parse(root).root) throw new Error('Pass an external diagnostics directory')
const repository = path.resolve(server, '../../..')
const relative = path.relative(repository, root)
if (!relative.startsWith('..') && !path.isAbsolute(relative))
  throw new Error('Diagnostics must be outside the checkout')
fs.mkdirSync(root, { recursive: true })
const home = path.join(root, 'home')
fs.mkdirSync(home, { recursive: true })
const env = {
  ...process.env,
  NODE_ENV: 'development',
  BRIDGE_DEV_FAKE_CLI: '1',
  BRIDGE_FAKE_CLI_HOME: home,
  USERPROFILE: home,
  HOME: home,
  CLAUDE_PROXY_HOST: '127.0.0.1',
  CLAUDE_PROXY_PORT: process.env.CLAUDE_PROXY_PORT ?? '3456',
  BRIDGE_LIFECYCLE_LOG_DIR: path.join(root, 'logs', 'lifecycle'),
}
// Neither fake CLI nor server needs inherited provider, proxy or CLI credentials.
for (const key of Object.keys(env)) {
  if (
    /ANTHROPIC|OAUTH|TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL|ACCESS_KEY|CLAUDECODE|CLAUDE_CODE_SESSION|^https?_proxy$|^all_proxy$/i.test(
      key,
    )
  )
    delete env[key]
}
const dashboard = path.join(server, 'dist/dashboard')
if (fs.existsSync(dashboard)) fs.cpSync(dashboard, path.join(root, 'dist/dashboard'), { recursive: true })
const out = fs.openSync(path.join(root, 'server-output.log'), 'a')
const child = spawn(
  process.execPath,
  [path.join(server, 'node_modules/tsx/dist/cli.mjs'), path.join(server, 'bin/claude-bridge.ts')],
  { cwd: root, env, stdio: ['inherit', out, out], windowsHide: true },
)
console.log(`Fake Bridge pid=${child.pid}; loopback port=${env.CLAUDE_PROXY_PORT}; state/logs=${root}`)
child.on('exit', (code) => {
  fs.closeSync(out)
  process.exitCode = code ?? 1
})
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal))
