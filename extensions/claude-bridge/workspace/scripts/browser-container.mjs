// One Linux browser run; repository input is read-only, outputs are copied out.
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { fileURLToPath } from 'node:url'

const repo = fileURLToPath(new URL('../../../../', import.meta.url))
const lock = JSON.parse(await fs.readFile(new URL('../package-lock.json', import.meta.url), 'utf8'))
const version = lock.packages['node_modules/playwright'].version
if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Expected a released Playwright version')
const name = `bridgeweb-acc-browser-${randomUUID()}`
const output = process.env.BRIDGE_BROWSER_OUTPUT ?? await fs.mkdtemp(path.join(os.tmpdir(), 'bridgeweb-acc-browser-'))
await fs.mkdir(output, { recursive: true })
const run = args => spawnSync('podman', args, { stdio: 'inherit', windowsHide: true })
const script = `set -eu
mkdir -p /tmp/task/extensions/claude-bridge /tmp/browser-output /workspace
mkdir -p /tmp/task/builtin-extensions/claude-bridge
cp /source/builtin-extensions/claude-bridge/package.json /tmp/task/builtin-extensions/claude-bridge/package.json
mkdir -p /tmp/task/src/kamin-host
cp /source/src/kamin-host/rolling-log.ts /tmp/task/src/kamin-host/rolling-log.ts
mkdir -p /tmp/task/src/kamin-host/services
cp /source/src/kamin-host/services/pty-sessions.ts /source/src/kamin-host/services/shells.ts /tmp/task/src/kamin-host/services/
cd /source/extensions/claude-bridge
tar --exclude=node_modules --exclude=dist -cf - extension webview workspace server/src/ui/src/lib/terminal-console | tar -xf - -C /tmp/task/extensions/claude-bridge
cp -a /ms-playwright /tmp/bridgeweb-playwright
cd /tmp/task/extensions/claude-bridge/extension
npm ci --no-audit --no-fund
cd ../webview
npm ci --no-audit --no-fund
npm run build:browser
cd ../workspace
npm ci --no-audit --no-fund
npm run build
result=0
npm run test:browser > /tmp/browser-output/tests.log 2>&1 || result=$?
cat /tmp/browser-output/tests.log
exit $result
`
const created = run(['create', '--name', name, '--mount', `type=bind,source=${repo},target=/source,readonly`,
  '--env', 'PLAYWRIGHT_SKIP_BROWSER_GC=1', '--env', 'PLAYWRIGHT_BROWSERS_PATH=/tmp/bridgeweb-playwright',
  '--env', 'BRIDGE_BROWSER_SCREENSHOT=/tmp/browser-output/bridge-browser-fixture.png',
  '--env', 'BRIDGE_PREVIEW_ROOT=/workspace',
  `mcr.microsoft.com/playwright:v${version}-noble`, 'sh', '-lc', script])
if (created.error || created.status !== 0) throw new Error('Could not create the browser test container')
try {
  const result = run(['start', '--attach', name])
  const copied = run(['cp', `${name}:/tmp/browser-output/.`, path.resolve(output)])
  console.log(`Browser evidence: ${path.resolve(output)}`)
  process.exitCode = result.error || result.status !== 0 || copied.error || copied.status !== 0 ? 1 : 0
} finally {
  if (run(['rm', name]).status !== 0) process.exitCode = 1
}
