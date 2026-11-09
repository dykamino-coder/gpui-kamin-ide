import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createBrowserEnvironment, createAppAssetHandler, startWorkspaceServer, WorkspaceAuth, serverTokenResolver, WorkspaceFiles, TerminalWorkspaceBackend, withBrowserServer, WorkspacePreview } from './index.js'
import { createSharedBackend } from './runtime.js'
import { loadCredentials, resetCredentials, SESSION_FILE } from './credentials.js'
declare const BRIDGE_VERSION: string

async function main(): Promise<void> {
  const storage = path.resolve(process.env.WORKSPACE_STORAGE || path.join(os.homedir(), '.local/state/bridge'))
  if (process.argv.includes('--reset-credentials')) { resetCredentials(storage); return }
  const root = fs.realpathSync(process.env.WORKSPACE_ROOT || '/workspace')
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
  const credentials = await loadCredentials(storage, serverUrl)
  const { token } = credentials
  delete process.env.BRIDGE_TOKEN
  delete process.env.BRIDGE_TOKEN_FILE
  const serveApp = await createAppAssetHandler(path.resolve(__dirname, '../../webview/dist/browser'))
  const preview = new WorkspacePreview()
  const browser = createBrowserEnvironment({ root, preview })
  const backend = createSharedBackend({ root, storage, serverUrl, token, version: BRIDGE_VERSION, environment: browser.environment })
  const files = new WorkspaceFiles(browser.wrap(withBrowserServer(backend, { serverUrl, token })), { root,
    onWatchError: () => { process.stderr.write('Bridge workspace file watch failed.\n') },
  })
  const terminals = new TerminalWorkspaceBackend(files, root)
  const dispose = () => {
    try { terminals.dispose() } finally { try { files.dispose() } finally { backend.dispose() } }
  }
  let server: Awaited<ReturnType<typeof startWorkspaceServer>>
  try {
    server = await startWorkspaceServer({ bind, port, origin, autoLogin: loopback,
      auth: new WorkspaceAuth(token, serverTokenResolver(serverUrl), 30 * 24 * 60 * 60 * 1000, Date.now,
        { secret: credentials.browserSecret, file: path.join(storage, SESSION_FILE) }), backend: terminals, serveApp, preview })
  } catch (error) { dispose(); throw error }
  let closing = false
  const close = () => {
    if (closing) return
    closing = true
    dispose()
    void server.close().catch(() => { process.exitCode = 1 })
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
