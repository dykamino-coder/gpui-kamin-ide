import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, afterEach, describe, expect, it } from 'vitest'
import { WarmSearch, findWarmSearch } from './warm-search'

// A stand-in for kamin-search speaking its protocol; the mode is its argument.
const FAKE = String.raw`
const mode = process.argv[2]
let buf = ''
process.stdin.on('data', (d) => {
  buf += d
  let i
  while ((i = buf.indexOf('\n')) !== -1) {
    const req = JSON.parse(buf.slice(0, i))
    buf = buf.slice(i + 1)
    const send = (h, body = '') => process.stdout.write(JSON.stringify({ ...h, stdoutLen: Buffer.byteLength(body) }) + '\n' + body)
    if (mode === 'die') process.exit(3)
    if (mode === 'unsupported') send({ id: req.id, unsupported: 'flag' })
    else if (mode === 'hang') { if (req.cancel) send({ id: req.id, code: 1, cancelled: true }) }
    else if (mode === 'split') {
      const body = 'a.rs\0'.repeat(20000)
      const frame = JSON.stringify({ id: req.id, code: 0, stdoutLen: body.length }) + '\n' + body
      for (let k = 0; k < frame.length; k += 777) process.stdout.write(frame.slice(k, k + 777))
    } else send({ id: req.id, code: 0, stderr: '', truncated: false }, req.args.join(' ') + '\n')
  }
})
`

const dir = mkdtempSync(join(tmpdir(), 'kamin-warm-search-'))
const script = join(dir, 'fake.cjs')
writeFileSync(script, FAKE)
const clients: WarmSearch[] = []
const client = (mode: string): WarmSearch => {
  const c = new WarmSearch(process.execPath, [script, mode])
  clients.push(c)
  return c
}
const opts = { cwd: dir, timeoutMs: 10_000 }

afterEach(() => {
  for (const c of clients.splice(0)) c.dispose()
})
afterAll(() => rmSync(dir, { recursive: true, force: true }))

describe('WarmSearch', () => {
  it('returns the answer with its raw stdout', async () => {
    const c = client('echo')
    const r = await c.run(['--files', '--'], opts)
    expect(r).toEqual({ stdout: Buffer.from('--files --\n'), stderr: '', code: 0, truncated: false, timedOut: false })
    // The process is reused for the next call.
    const pid = c.process?.pid
    expect((await c.run(['x'], opts))?.stdout.toString()).toBe('x\n')
    expect(c.process?.pid).toBe(pid)
  })

  it('joins a body that arrives in many chunks', async () => {
    const r = await client('split').run(['--files'], opts)
    expect(r?.stdout.length).toBe(5 * 20000)
  })

  it('falls back to ripgrep for a command it does not replicate', async () => {
    expect(await client('unsupported').run(['--pcre2'], opts)).toBeNull()
  })

  it('falls back when the process dies, and stays off after three deaths', async () => {
    const c = client('die')
    for (let i = 0; i < 3; i++) {
      expect(c.available).toBe(true)
      expect(await c.run(['x'], opts)).toBeNull()
    }
    expect(c.available).toBe(false)
    expect(await c.run(['x'], opts)).toBeNull()
    expect(c.process).toBeNull()
  })

  it('cancels on abort and removes its listener', async () => {
    const c = client('hang')
    const ac = new AbortController()
    let listeners = 0
    const add = ac.signal.addEventListener.bind(ac.signal)
    const remove = ac.signal.removeEventListener.bind(ac.signal)
    ac.signal.addEventListener = ((...a: Parameters<typeof add>) => (listeners++, add(...a))) as typeof add
    ac.signal.removeEventListener = ((...a: Parameters<typeof remove>) => (listeners--, remove(...a))) as typeof remove
    const pending = c.run(['x'], { ...opts, signal: ac.signal })
    ac.abort()
    expect((await pending)?.code).toBe(1)
    expect(listeners).toBe(0)
    expect(await c.run(['x'], { ...opts, signal: ac.signal })).toBeNull()
  })
})

describe('findWarmSearch', () => {
  it('is off by KAMIN_SEARCH=0 and off the bundled path outside Windows', () => {
    expect(findWarmSearch({ KAMIN_SEARCH: '0', KAMIN_SEARCH_PATH: script }, process.execPath, 'win32')).toBeNull()
    expect(findWarmSearch({ KAMIN_SEARCH_PATH: script }, process.execPath, 'linux')).toBe(script)
    expect(findWarmSearch({}, join(dir, 'runtime', 'node.exe'), 'linux')).toBeNull()
    writeFileSync(join(dir, 'kamin-search.exe'), '')
    expect(findWarmSearch({}, join(dir, 'runtime', 'node.exe'), 'win32')).toBe(join(dir, 'runtime', '..', 'kamin-search.exe'))
  })
})
