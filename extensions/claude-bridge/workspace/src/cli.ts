import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createBrowserEnvironment, createAppAssetHandler, startWorkspaceServer, WorkspaceAuth, serverTokenResolver } from './index.js'
import { createSharedBackend } from './runtime.js'
declare const BRIDGE_VERSION: string

async function main(): Promise<void> {
  const root = fs.realpathSync(process.env.WORKSPACE_ROOT || '/workspace')
  const storage = path.resolve(process.env.WORKSPACE_STORAGE || path.join(os.homedir(), '.local/state/bridge'))
  if (process.env.BRIDGE_TOKEN && process.env.BRIDGE_TOKEN_FILE) throw new Error('Ambiguous token source')
  const token = process.env.BRIDGE_TOKEN || (process.env.BRIDGE_TOKEN_FILE
    ? fs.readFileSync(process.env.BRIDGE_TOKEN_FILE, 'utf8').trim() : '')
  const serverUrl = process.env.BRIDGE_SERVER_URL || ''
  if (!token || !serverUrl) throw new Error('Bridge server and token are required')
  delete process.env.BRIDGE_TOKEN
  delete process.env.BRIDGE_TOKEN_FILE
  const bind = process.env.WORKSPACE_BIND || '127.0.0.1'
  const port = Number(process.env.WORKSPACE_PORT || 3460)
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid workspace port')
  const serveApp = await createAppAssetHandler(path.resolve(__dirname, '../../webview/dist/browser'))
  const browser = createBrowserEnvironment({ root })
  const backend = createSharedBackend({ root, storage, serverUrl, token, version: BRIDGE_VERSION, environment: browser.environment })
  let server: Awaited<ReturnType<typeof startWorkspaceServer>>
  try {
    server = await startWorkspaceServer({ bind, port, origin: process.env.WORKSPACE_ORIGIN,
      auth: new WorkspaceAuth(token, serverTokenResolver(serverUrl)), backend: browser.wrap(backend), serveApp })
  } catch (error) { backend.dispose(); throw error }
  let closing = false
  const close = () => {
    if (closing) return
    closing = true
    backend.dispose()
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
