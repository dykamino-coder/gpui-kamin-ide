import 'reflect-metadata'
// Isolated container smoke: no Claude login and no CLI invocation.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import { readdirSync, readFileSync } from 'node:fs'
import { WebSocket } from 'ws'
import { initStatsDb } from '../src/core/stats/database/lifecycle'
import { createToken } from '../src/core/auth/tokens'
import { installUpgradeHandler } from '../src/core/server/ws'
import { attachSessionWebSocket } from '../src/core/pty/session-ws'
import { sessions } from '../src/core/pty/session-core'

await initStatsDb()
const token = await createToken('synthetic-notemp-smoke')
const server = createServer()
installUpgradeHandler(server)
attachSessionWebSocket(server)
server.listen(0, '127.0.0.1')
await once(server, 'listening')
const port = (server.address() as { port: number }).port
const socket = new WebSocket(`ws://127.0.0.1:${port}/ws/session`)
await once(socket, 'open')
const frames: unknown[] = []
socket.on('message', (raw) => frames.push(JSON.parse(String(raw))))
for (let attempt = 0; attempt < 3; attempt++) {
  const response = once(socket, 'message', { signal: AbortSignal.timeout(15000) })
  socket.send(
    JSON.stringify({
      type: 'session:resume',
      token: token.token,
      conversationId: '00000000-0000-4000-8000-000000000074',
      noSpawnOnNotFound: true,
    }),
  )
  const [raw] = await response
  assert.deepEqual(JSON.parse(String(raw)), { type: 'session:resume-not-found' })
  assert.equal(sessions.size, 0)
}
const cliProcesses = readdirSync('/proc')
  .filter((pid) => /^\d+$/.test(pid))
  .filter((pid) => {
    try {
      return /(?:^|\x00)(?:[^\x00]*\/)?claude(?:\x00|$)/.test(readFileSync(`/proc/${pid}/cmdline`, 'utf8'))
    } catch {
      return false
    }
  })
assert.equal(cliProcesses.length, 0)
assert.equal(frames.length, 3)
console.log('PASS: 3 authenticated missing resumes, no session IDs, no registered sessions, no Claude processes')
socket.close()
await once(socket, 'close')
server.close()
process.exit(0)
