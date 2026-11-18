// Protocol-level load client for a fake-CLI Bridge (see load-test.md).
// Each simulated workspace container owns one Bridge token and N chat sockets,
// answers MCP tool calls like a client host, and measures what its user would
// see. Synthetic only: no model, no account, no prompt content.
import { WebSocket } from 'ws'
import fs from 'node:fs'
import { setTimeout as sleep } from 'node:timers/promises'
import { parseArgs } from 'node:util'

const { values: opt } = parseArgs({
  options: {
    port: { type: 'string', default: '3456' },
    clients: { type: 'string', default: '20' },
    sessions: { type: 'string', default: '5' },
    blocks: { type: 'string', default: '20' },
    interval: { type: 'string', default: '50' },
    ramp: { type: 'string', default: '10' },
    jitter: { type: 'string', default: '2000' },
    idle: { type: 'string', default: '60' },
    phases: { type: 'string', default: 'open,turn,idle,reconnect,resume,close' },
    out: { type: 'string' },
  },
})
const num = (key) => Number(opt[key])
const origin = `http://127.0.0.1:${opt.port}`
const wsUrl = `ws://127.0.0.1:${opt.port}/ws/session`
const MARK = /\[load (\S+) seq=(\d+) t=(\d+)\]/
const started = Date.now()
const log = (...a) => console.log(`[${((Date.now() - started) / 1000).toFixed(1)}s]`, ...a)

const samples = { ttft: [], gap: [], delivery: [], open: [], resume: [], reattach: [], turn: [] }
// Turn tags start with a phase letter: a = first turn, b = reconnect, c = after cold resume.
const push = (name, value, tag) => {
  samples[name].push(value)
  ;(samples[`${name}_${tag[0]}`] ??= []).push(value)
}
const counts = {
  lost: 0,
  dup: 0,
  reconnectErrors: 0,
  turnErrors: 0,
  openErrors: 0,
  errorFrames: 0,
  toolCalls: 0,
  dupBy: {},
}
const idleTraffic = { bytesIn: 0, bytesOut: 0, seconds: 0, frames: {} }

function pct(list, p) {
  if (!list.length) return null
  const s = [...list].sort((a, b) => a - b)
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))]
}
const summary = (list) => ({
  n: list.length,
  p50: pct(list, 50),
  p95: pct(list, 95),
  p99: pct(list, 99),
  max: list.length ? Math.max(...list) : null,
})

class Chat {
  constructor(client, index) {
    this.client = client
    this.index = index
    this.conversationId = null
    this.mirror = { posEnd: undefined, pos: undefined, uuid: undefined, head: undefined }
    this.turn = null
    this.waiters = []
    this.idleFrames = null
  }
  connect() {
    const ws = new WebSocket(wsUrl)
    this.ws = ws
    ws.on('message', (raw) => this.onFrame(JSON.parse(raw.toString()), raw.length))
    ws.on('error', () => {})
    return new Promise((resolve, reject) => {
      ws.once('open', resolve)
      ws.once('error', reject)
    })
  }
  bytes() {
    const s = this.ws?._socket
    return s ? { in: s.bytesRead, out: s.bytesWritten } : { in: 0, out: 0 }
  }
  send(msg) {
    if (this.ws.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg))
  }
  waitFor(predicate, ms, what) {
    return new Promise((resolve, reject) => {
      const w = { predicate, resolve }
      w.timer = setTimeout(() => {
        this.waiters = this.waiters.filter((x) => x !== w)
        reject(new Error(`timeout: ${what}`))
      }, ms)
      this.waiters.push(w)
    })
  }
  onFrame(f, size) {
    const now = Date.now()
    if (this.idleFrames) this.idleFrames[f.type] = (this.idleFrames[f.type] ?? 0) + size
    if (f.type === 'error') counts.errorFrames++
    if (f.type === 'session:activity' && typeof f.promptReady === 'boolean') this.ready = f.promptReady
    if (f.type === 'session:conversation-id' && f.conversationId) this.conversationId = f.conversationId
    // A host reports the calls it still holds; the server redelivers a call
    // that went out on a dropped socket and is absent from this report.
    if (f.type === 'commands:request') this.send({ type: 'commands:report', report: { calls: [], shells: [] } })
    if (f.type === 'mcp:call') {
      counts.toolCalls++
      this.send({ type: 'mcp:response', requestId: f.requestId, result: 'Synthetic host result' })
    }
    if (f.type === 'jsonl:sync-reset' || f.type === 'jsonl:sync-begin' || f.type === 'jsonl:sync-end')
      this.mirror.head = f.head ?? this.mirror.head
    if (f.type === 'jsonl:entries') {
      for (const e of f.entries ?? []) {
        if (typeof e._posEnd === 'number' && (this.mirror.posEnd ?? -1) < e._posEnd) {
          this.mirror = { ...this.mirror, posEnd: e._posEnd, pos: e._pos, uuid: e.uuid }
        }
        const text = e.message?.content?.find?.((b) => b.type === 'text')?.text
        const m = text && MARK.exec(text)
        const t = this.turn
        if (!m || !t || m[1] !== t.tag) continue
        const seq = Number(m[2])
        if (t.seen.has(seq)) {
          counts.dup++
          const key = `${t.tag[0]}${f.replay ? '-replay' : ''}${t.reconnected ? '-reattached' : ''}`
          counts.dupBy[key] = (counts.dupBy[key] ?? 0) + 1
          continue
        }
        t.seen.add(seq)
        push('delivery', now - Number(m[3]), t.tag)
        if (t.first === undefined) {
          t.first = now
          push('ttft', now - t.sent, t.tag)
        } else if (!t.reconnected) push('gap', now - t.last, t.tag)
        t.last = now
      }
    }
    for (const w of [...this.waiters]) {
      if (w.predicate(f)) {
        clearTimeout(w.timer)
        this.waiters = this.waiters.filter((x) => x !== w)
        w.resolve(f)
      }
    }
  }
  promptReady = (f) => f.type === 'session:activity' && f.promptReady
  async open() {
    const t0 = Date.now()
    await this.connect()
    const ready = this.waitFor(this.promptReady, 60000, 'prompt ready')
    this.send({ type: 'session:create', token: this.client.token, cols: 120, rows: 40, protocolVersion: 1 })
    await this.waitFor((f) => f.type === 'session:created', 60000, 'created')
    await ready
    samples.open.push(Date.now() - t0)
  }
  async streamTurn(tag, { dropAfter } = {}) {
    const blocks = num('blocks')
    const t = (this.turn = { tag, seen: new Set(), sent: Date.now() })
    const deadline = t.sent + 60000 + blocks * num('interval') * 2
    this.send({ type: 'session:submitText', data: `/fake-stream ${blocks} ${opt.interval} ${tag}` })
    this.ready = false
    if (dropAfter !== undefined) {
      while (t.seen.size < dropAfter && Date.now() < deadline) await sleep(5)
      await this.reattach()
    }
    // Done when every block arrived (live or by catch-up); the deadline only
    // bounds a broken run, whose missing blocks are counted as lost.
    while (t.seen.size < blocks && Date.now() < deadline) await sleep(10)
    const missing = blocks - t.seen.size
    counts.lost += missing
    if (missing) throw new Error(`turn ${tag}: ${missing} blocks missing`)
    push('turn', t.last - t.sent, tag)
    this.turn = null
    // The CLI answers the Stop hook after its last block; let it settle.
    if (!this.ready)
      await this.waitFor((f) => f.type === 'session:activity' && f.promptReady, 10000, 'idle after turn').catch(
        () => {},
      )
  }
  async reattach() {
    const t0 = Date.now()
    this.ws.terminate()
    if (this.turn) this.turn.reconnected = true
    await sleep(300)
    try {
      await this.connect()
      const created = this.waitFor((f) => f.type === 'session:created', 15000, 'reattach created')
      this.send({
        type: 'session:resume',
        token: this.client.token,
        conversationId: this.conversationId,
        cols: 120,
        rows: 40,
        protocolVersion: 1,
      })
      const reply = await created
      if (!reply.reattached) {
        counts.reconnectErrors++
        log('not reattached', JSON.stringify(reply).slice(0, 200))
      }
      const end = this.waitFor((f) => f.type === 'jsonl:sync-end' || f.type === 'jsonl:sync-reset', 15000, 'sync')
      this.send({
        type: 'jsonl:sync-request',
        sinceHead: this.mirror.head,
        sincePos: this.mirror.posEnd,
        lastPos: this.mirror.pos,
        lastUuid: this.mirror.uuid,
      })
      const sync = await end
      if (sync.type === 'jsonl:sync-reset') {
        counts.reconnectErrors++
        log('sync reset', JSON.stringify({ sync, mirror: this.mirror }).slice(0, 300))
      }
      samples.reattach.push(Date.now() - t0)
    } catch (err) {
      counts.reconnectErrors++
      log(`client ${this.client.index} chat ${this.index}: ${err.message}`)
    }
  }
  async coldResume() {
    this.send({ type: 'session:end' })
    this.ws.close()
    await sleep(500)
    const t0 = Date.now()
    await this.connect()
    const ready = this.waitFor(this.promptReady, 60000, 'resume prompt ready')
    this.send({
      type: 'session:resume',
      token: this.client.token,
      conversationId: this.conversationId,
      cols: 120,
      rows: 40,
      protocolVersion: 1,
    })
    const reply = await this.waitFor(
      (f) => f.type === 'session:created' || f.type === 'session:resume-not-found' || f.type === 'error',
      60000,
      'resume created',
    )
    if (reply.type !== 'session:created') throw new Error(`resume failed: ${reply.type} ${reply.message ?? ''}`)
    await ready
    this.mirror = { head: undefined }
    samples.resume.push(Date.now() - t0)
  }
  async learnHead() {
    const reply = this.waitFor((f) => f.type === 'jsonl:sync-reset' || f.type === 'jsonl:sync-end', 15000, 'head')
    this.send({ type: 'jsonl:sync-request' })
    await reply.catch(() => {})
  }
}

async function each(chats, fn, spreadMs) {
  return Promise.all(
    chats.map(async (chat) => {
      if (spreadMs) await sleep(Math.random() * spreadMs)
      try {
        await fn(chat)
      } catch (err) {
        log(`client ${chat.client.index} chat ${chat.index}: ${err.message}`)
        return err
      }
    }),
  ).then((r) => r.filter(Boolean).length)
}

const phases = new Set(opt.phases.split(','))
const clients = []
for (let i = 0; i < num('clients'); i++) {
  const token = await fetch(`${origin}/api/dashboard/tokens`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: `synthetic-load-${i}` }),
  }).then((r) => r.json())
  if (!token.token) throw new Error('Local Bridge token creation failed')
  const client = { index: i, token: token.token }
  client.chats = Array.from({ length: num('sessions') }, (_, j) => new Chat(client, j))
  clients.push(client)
}
const chats = clients.flatMap((c) => c.chats)
const phaseLog = []
const mark = (name) => phaseLog.push({ name, at: Date.now() - started })
log(`clients=${clients.length} chats=${chats.length}`)

mark('open')
counts.openErrors += await each(chats, (c) => c.open(), num('ramp') * 1000)
const live = () => chats.filter((c) => c.ws?.readyState === WebSocket.OPEN)
log(`opened ${live().length}`)
if (phases.has('turn')) {
  mark('turn')
  counts.turnErrors += await each(live(), (c) => c.streamTurn(`a${c.client.index}x${c.index}`), num('jitter'))
  await each(live(), (c) => c.learnHead())
  log('turn A done')
}
if (phases.has('idle')) {
  mark('idle')
  const before = live().map((c) => ({ c, b: c.bytes() }))
  for (const { c } of before) c.idleFrames = {}
  const t0 = Date.now()
  await sleep(num('idle') * 1000)
  idleTraffic.seconds = (Date.now() - t0) / 1000
  for (const { c, b } of before) {
    const a = c.bytes()
    idleTraffic.bytesIn += a.in - b.in
    idleTraffic.bytesOut += a.out - b.out
    for (const [k, v] of Object.entries(c.idleFrames)) idleTraffic.frames[k] = (idleTraffic.frames[k] ?? 0) + v
    c.idleFrames = null
  }
  log('idle done')
}
if (phases.has('wake')) {
  // After a long idle the server may have hibernated the CLIs: this turn wakes them.
  mark('wake')
  counts.turnErrors += await each(live(), (c) => c.streamTurn(`w${c.client.index}x${c.index}`), num('jitter'))
  log('wake turn done')
}
if (phases.has('reconnect')) {
  mark('reconnect')
  counts.turnErrors += await each(
    live(),
    (c) => c.streamTurn(`b${c.client.index}x${c.index}`, { dropAfter: Math.floor(num('blocks') / 3) }),
    num('jitter'),
  )
  log('reconnect turn done')
}
if (phases.has('resume')) {
  mark('resume')
  counts.reconnectErrors += await each(live(), (c) => c.coldResume(), num('jitter'))
  counts.turnErrors += await each(live(), (c) => c.streamTurn(`c${c.client.index}x${c.index}`), num('jitter'))
  log('resume turn done')
}
if (phases.has('close')) {
  mark('close')
  for (const c of chats) {
    c.send({ type: 'session:end' })
    c.ws?.close()
  }
  await sleep(2000)
}
const result = {
  config: opt,
  startedAt: started,
  finishedAt: Date.now(),
  chats: chats.length,
  phases: phaseLog,
  counts,
  idleTraffic: {
    ...idleTraffic,
    bytesInPerChatPerMin: idleTraffic.seconds ? (idleTraffic.bytesIn / chats.length / idleTraffic.seconds) * 60 : null,
  },
  ms: Object.fromEntries(Object.entries(samples).map(([k, v]) => [k, summary(v)])),
}
const text = JSON.stringify(result, null, 2)
if (opt.out) fs.writeFileSync(opt.out, text)
console.log(text)
process.exit(0)
