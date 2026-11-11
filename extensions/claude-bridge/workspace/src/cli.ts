// Container executable: environment configuration, the bundled browser UI and
// Bridge-token sign-in around the one composition in headless.ts. The build
// keeps `./headless.js` external, so this file and @kaminide/bridge-workspace-host
// run the same dist/headless.cjs.
import os from 'node:os'
import path from 'node:path'
import { createAppAssetHandler } from './assets.js'
import { bridgeTokenAuth, resetCredentials, startHeadlessWorkspace } from './headless.js'

async function main(): Promise<void> {
  const storage = path.resolve(process.env.WORKSPACE_STORAGE || path.join(os.homedir(), '.local/state/bridge'))
  if (process.argv.includes('--reset-credentials')) { resetCredentials(storage); return }
  const root = process.env.WORKSPACE_ROOT || '/workspace'
  const serverUrl = process.env.BRIDGE_SERVER_URL || ''
  if (!serverUrl) throw new Error('Bridge server is required')
  const bind = process.env.WORKSPACE_BIND || '127.0.0.1'
  const port = Number(process.env.WORKSPACE_PORT || 3460)
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid workspace port')
  const origin = process.env.WORKSPACE_ORIGIN || `http://${bind === '::1' ? '[::1]' : bind}:${port}`
  const publicUrl = new URL(origin)
  if (publicUrl.origin !== origin || !['http:', 'https:'].includes(publicUrl.protocol)) throw new Error('Invalid browser origin')
  const loopback = ['127.0.0.1', '[::1]', 'localhost'].includes(publicUrl.hostname)
  if (!loopback && process.env.WORKSPACE_ALLOW_REMOTE !== '1') throw new Error('Remote publication requires WORKSPACE_ALLOW_REMOTE=1')
  const serveApp = await createAppAssetHandler(process.env.WORKSPACE_UI_DIR || path.resolve(__dirname, '../../webview/dist/browser'))
  const workspace = await startHeadlessWorkspace({ root, storage, serverUrl, origin, bind, port, serveApp,
    auth: bridgeTokenAuth, autoLogin: loopback })
  let closing = false
  const close = () => {
    if (closing) return
    closing = true
    void workspace.close().catch(() => { process.exitCode = 1 })
  }
  process.once('SIGINT', close)
  process.once('SIGTERM', close)
  process.stdout.write('Bridge workspace listening.\n')
}

void main().catch(() => {
  // Shared/runtime errors may contain upstream credentials or model payloads.
  process.stderr.write('Bridge workspace startup failed; check configured paths, server, token and origin.\n')
  process.exitCode = 1
})
