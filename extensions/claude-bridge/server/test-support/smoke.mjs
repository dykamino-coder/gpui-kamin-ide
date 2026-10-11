// Automated real-server smoke. This is not Windows GPUI/CEF acceptance.
import { WebSocket } from 'ws'
import { once } from 'node:events'
import { setTimeout } from 'node:timers/promises'
const port = Number(process.argv[2] ?? 3456)
const deadlineSignal = AbortSignal.timeout(120000)
const origin = `http://127.0.0.1:${port}`
let health
while (!health) {
  deadlineSignal.throwIfAborted()
  try {
    health = await fetch(`${origin}/health`, { signal: deadlineSignal }).then((r) => r.json())
  } catch {
    deadlineSignal.throwIfAborted()
    await setTimeout(100)
  }
}
if (health.status !== 'ok') throw new Error('Bridge health failed')
const token = await fetch(`${origin}/api/dashboard/tokens`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ name: 'synthetic-harness-smoke' }),
  signal: deadlineSignal,
}).then((r) => r.json())
if (!token.token) throw new Error('Local Bridge token creation failed')
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws/session`)
const frames = []
ws.on('message', (data) => {
  const msg = JSON.parse(data.toString())
  frames.push(msg)
  if (msg.type === 'mcp:call')
    ws.send(JSON.stringify({ type: 'mcp:response', requestId: msg.requestId, result: 'Synthetic host result' }))
})
const wait = async (predicate, boundary = 'session') => {
  const deadline = Date.now() + 60000
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error(`Smoke boundary timed out: ${boundary}`)
    await setTimeout(20)
  }
}
try {
  await once(ws, 'open', { signal: deadlineSignal })
  ws.send(JSON.stringify({ type: 'session:create', token: token.token, cols: 120, rows: 40 }))
  await wait(() => frames.some((f) => f.type === 'session:created'))
  await wait(() => frames.some((f) => f.type === 'session:activity' && f.promptReady))
  for (const command of [
    '/fake-pressure',
    '/compact',
    '/fake-agents 800',
    '/fake-idle',
    '/fake-follow-up',
    '/fake-report',
    '/fake-mcp',
  ]) {
    const before = frames.length
    ws.send(JSON.stringify({ type: 'session:submitText', data: command }))
    await wait(() =>
      frames
        .slice(before)
        .some(
          (f) =>
            f.type === 'session:activity' &&
            f.hookDriven &&
            f.promptReady &&
            f.lastMessage === 'Synthetic turn complete',
        ),
    )
  }
  await wait(() =>
    frames.some((f) => f.type === 'jsonl:entries' && f.entries.some((e) => e.subtype === 'compact_boundary')),
  )
  await wait(() => frames.some((f) => f.type === 'jsonl:subagent-entries'))
  const tools = frames.filter((f) => f.type === 'mcp:call')
  if (tools.length !== 1) throw new Error('Unexpected MCP execution count')
  const account = await fetch(`${origin}/api/dashboard/auth-status?force=true`, { signal: deadlineSignal }).then((r) =>
    r.json(),
  )
  if (account.authMethod !== 'fake-cli') throw new Error('Auth probe escaped fake CLI')
  console.log(
    JSON.stringify({
      passed: true,
      realPty: true,
      realHooks: true,
      compact: true,
      agents: true,
      mcpCalls: tools.length,
      fakeAuth: true,
    }),
  )
} finally {
  if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'session:end' }))
  ws.close()
}
