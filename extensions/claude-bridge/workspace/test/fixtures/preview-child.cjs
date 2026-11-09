// Isolated real shared MCP handler graph; no operator HOME or credentials.
const path = require('node:path')
const { createSharedBackend } = require('../../dist/runtime.cjs')

async function main() {
  const { createBrowserEnvironment, createAppAssetHandler, startWorkspaceServer, WorkspaceAuth, WorkspacePreview } = await import('../../dist/index.js')
  const [root, upstream] = process.argv.slice(2)
  const preview = new WorkspacePreview()
  const browser = createBrowserEnvironment({ root, preview })
  const backend = createSharedBackend({ root, storage: path.join(root, 'storage'), serverUrl: upstream,
    token: 'synthetic-preview-token', version: 'fixture', environment: browser.environment })
  const server = await startWorkspaceServer({ backend: browser.wrap(backend), preview, port: 0,
    auth: new WorkspaceAuth('synthetic-preview-token', async () => ({ id: 'fixture' })),
    serveApp: await createAppAssetHandler(path.resolve(__dirname, '../../../webview/dist/browser')) })
  process.on('message', async message => {
    if (message !== 'shutdown') return
    await server.close()
    backend.dispose()
    process.disconnect()
  })
  process.send({ origin: server.origin })
}
main().catch(() => { process.exitCode = 1; process.disconnect() })
