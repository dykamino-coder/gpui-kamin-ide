// A separate HOME and process are essential: real shared handlers use singleton
// registries and ~/.claude. This fixture never loads the operator's profile.
const path = require('node:path')
const { createSharedBackend } = require('../../dist/runtime.cjs')

async function main() {
  const { startWorkspaceServer, WorkspaceAuth } = await import('../../dist/index.js')
  const [root, upstream] = process.argv.slice(2)
  let clipboard = ''
  const backend = createSharedBackend({
    root, storage: path.join(root, 'storage'), serverUrl: upstream, token: 'synthetic-workspace-token', version: 'fixture',
    environment: {
      clipboard: { readText: async () => clipboard, writeText: async text => { clipboard = text } },
      message: async () => undefined,
      pickPaths: async () => { throw new Error('No browser path chooser in this fixture') },
      savePath: async () => { throw new Error('No browser save dialog in this fixture') },
      openExternal: async () => { throw new Error('No browser navigation in this fixture') },
      openDocument: async () => { throw new Error('No browser editor in this fixture') },
      reveal: async () => { throw new Error('No browser file panel in this fixture') },
    },
  })
  const auth = new WorkspaceAuth('synthetic-workspace-token', async () => ({ id: 'fixture' }))
  const server = await startWorkspaceServer({ backend, auth, port: 0 })
  process.on('message', async message => {
    if (message === 'shutdown') {
      await server.close()
      backend.dispose()
      backend.dispose() // idempotence
      process.send({ closed: true })
      process.disconnect()
      // No process.exit: the parent asserts natural exit, exposing leaked timers.
    }
  })
  process.send({ origin: server.origin })
}
main().catch(() => { process.exitCode = 1; process.disconnect() })
