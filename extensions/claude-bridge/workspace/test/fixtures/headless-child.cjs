// Embedding application fixture: the packaged entry dist/headless.cjs with its
// own sign-in policy and no UI. A separate process and HOME, as for runtime-child.
const { createServer } = require('node:http')
const path = require('node:path')
const host = require('../../dist/headless.cjs')

async function main() {
  const [root, upstream, port] = process.argv.slice(2)
  // Studio-style policy: its site verified the user and posts an assertion.
  const sessions = new Set()
  const auth = {
    async login(candidate) {
      if (candidate !== 'synthetic-studio-assertion') return null
      const cookie = host.createSessionCookie()
      sessions.add(cookie)
      return cookie
    },
    async validate(cookie) { return !!cookie && sessions.has(cookie) },
    logout(cookie) { sessions.delete(cookie) },
    cookieHeader(cookie, secure) { return host.sessionCookieHeader(cookie, { secure, maxAgeSeconds: 600 }) },
  }
  const env = { BRIDGE_TOKEN: 'synthetic-bootstrap-token' }
  const origin = `http://127.0.0.1:${port}`
  const workspace = await host.startHeadlessWorkspace({
    root, storage: path.join(root, 'storage'), serverUrl: upstream, origin, port: Number(port), auth, env,
  })
  // A container-local dev server for the preview mount.
  let build = 'first build'
  const dev = createServer((request, response) => { response.end(`/* ${build} */`) })
  await new Promise(resolve => dev.listen(0, '127.0.0.1', resolve))
  process.on('message', async message => {
    if (message === 'mount') {
      workspace.preview.mount({ name: 'demo', entry: `http://127.0.0.1:${dev.address().port}/remoteEntry.js`, exposedModule: './Panel' })
      process.send({ mounted: true })
    } else if (message === 'rebuild') {
      build = 'second build'
      process.send({ rebuilt: true })
    } else if (message === 'shutdown') {
      await workspace.close()
      await workspace.close() // idempotence
      await new Promise(resolve => dev.close(resolve))
      process.send({ closed: true })
      process.disconnect()
    }
  })
  process.send({ origin: workspace.origin, version: host.version, bootstrapConsumed: !('BRIDGE_TOKEN' in env) })
}
main().catch(error => { process.send?.({ error: String(error?.message ?? error) }); process.exitCode = 1; process.disconnect?.() })
