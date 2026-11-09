import { cp, lstat, mkdir, readdir, writeFile } from 'node:fs/promises'
import { spawn } from 'node:child_process'
import path from 'node:path'

if (process.platform !== 'linux' || process.env.BRIDGEWEB_ACC_CONTAINER !== '1') throw new Error('Linux acceptance container required')
// Copy only source roots into the disposable Linux filesystem. Never use host
// node_modules, build outputs, Git metadata, dotenv or credential files.
const roots = ['src', 'builtin-extensions/claude-bridge/package.json', 'builtin-extensions/claude-bridge/chat.html', 'extensions/claude-bridge/extension', 'extensions/claude-bridge/workspace',
  'extensions/claude-bridge/webview', 'extensions/claude-bridge/server/src/ui/src/lib/terminal-console']
const excluded = /^(node_modules|dist|\.git|test-results|playwright-report)$|^\.env|\.(log.*|pem|key)$/
for (const relative of roots) {
  await cp(path.join('/source', relative), path.join('/opt/bridge', relative), {
    recursive: true,
    filter: async source => {
      if (excluded.test(path.basename(source))) return false
      // cp's verbatimSymlinks must not give the source mount any write route.
      return !(await lstat(source)).isSymbolicLink()
    },
  })
}
async function run(args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn('npm', args, { cwd, stdio: 'inherit' })
    child.once('error', reject)
    child.once('close', code => code === 0 ? resolve() : reject(new Error('Build failed')))
  })
}
const workspace = '/opt/bridge/extensions/claude-bridge/workspace'
await run(['run', 'build'], workspace)
await run(['run', 'build:browser'], '/opt/bridge/extensions/claude-bridge/webview')
await mkdir('/reports', { recursive: true })
const mode = process.argv[2]
if (!['fixture', 'live', 'credentials'].includes(mode)) throw new Error('Expected fixture, live or credentials')
const args = mode !== 'fixture'
  ? [`acceptance/${mode === 'credentials' ? 'credentials' : 'run'}.mjs`, '--report-dir', '/reports', '--server-origin', 'http://host.containers.internal:3462', ...process.argv.slice(3)]
  : ['--test', ...((await readdir(path.join(workspace, 'test'))).filter(n => n.endsWith('.smoke.mjs')).map(n => `test/${n}`))]
const child = spawn('node', args, { cwd: workspace, stdio: ['ignore', 'pipe', 'pipe'] })
let fixtureOutput = ''
for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {
  if (mode === 'fixture') fixtureOutput += bytes.toString()
  process.stdout.write(bytes)
})
child.once('error', () => { process.exitCode = 1 })
const code = await new Promise(resolve => child.once('close', resolve))
if (mode === 'fixture') await writeFile('/reports/fixture.tap', fixtureOutput, { mode: 0o600 })
process.exitCode = code ?? 1
