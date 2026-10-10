import { describe, expect, it } from 'vitest'
import { InvokeDiagnostics, type SafeInvokeBoundary } from './invoke-diagnostics'
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve, dirname, basename } from 'node:path'
import { vi } from 'vitest'

function fixture() {
  const records: SafeInvokeBoundary[] = []
  const observer = new InvokeDiagnostics<object>(event => records.push(event))
  const view = {}
  observer.attach(view)
  return { records, observer, view }
}
describe('BR-24 host invoke boundaries', () => {
  it('correlates completion and false/rejected send without retrying a completed mutation', async () => {
    for (const failure of ['false', 'reject', 'throw'] as const) {
      const { records, observer, view } = fixture()
      const trace = observer.begin(view, 1, 'hooks:set-plugin-approval', 'document-private')
      let mutations = 0
      mutations++
      trace.handler('resolved')
      let sends = 0
      await trace.reply(() => {
        sends++
        if (failure === 'throw') throw new Error('synthetic-private-error')
        return failure === 'reject' ? Promise.reject(new Error('synthetic-private-error')) : Promise.resolve(false)
      })
      expect(mutations).toBe(1)
      expect(sends).toBe(1)
      expect(records.map(event => event.stage)).toEqual(['host-received', 'handler-resolved', failure === 'false' ? 'reply-false' : 'reply-rejected'])
      expect(new Set(records.map(event => event.documentRef)).size).toBe(1)
      expect(new Set(records.map(event => event.channelRef)).size).toBe(1)
      expect(JSON.stringify(records)).not.toContain('private')
    }
  })
  it('distinguishes handler rejection/unregistered from accepted send, without error/result payloads', async () => {
    const { records, observer, view } = fixture()
    const one = observer.begin(view, 1, 'synthetic-private-channel', 'doc')
    one.handler('rejected'); await one.reply(() => Promise.resolve(true))
    const two = observer.begin(view, 2, 'unregistered', 'doc')
    two.handler('unregistered'); await two.reply(() => Promise.resolve(true))
    expect(records.map(event => event.stage)).toEqual(['host-received', 'handler-rejected', 'reply-accepted', 'host-received', 'handler-unregistered', 'reply-accepted'])
    expect(JSON.stringify(records)).not.toContain('synthetic-private-channel')
  })
  it('marks replaced/disposed source and hidden state while preserving the original single send attempt', async () => {
    const { records, observer, view } = fixture()
    const old = observer.begin(view, 1, 'config:get', 'old-doc')
    observer.setHidden(view, true)
    observer.begin(view, 1, 'config:get', 'new-doc')
    observer.detach(view)
    old.handler('resolved')
    await old.reply(() => Promise.resolve(false))
    expect(records.slice(-4).map(event => event.stage)).toEqual(['handler-resolved', 'reply-source-disposed', 'reply-document-replaced', 'reply-false'])
    expect(records.at(-1)?.hidden).toBe(true)
    expect(records[0]!.documentRef).not.toBe(records[1]!.documentRef)
  })
  it('keeps two views/documents with reused numeric IDs distinguishable', async () => {
    const { records, observer, view } = fixture()
    const other = {}; observer.attach(other)
    observer.begin(view, 1, 'config:get', 'doc-a')
    observer.begin(other, 1, 'config:get', 'doc-b')
    expect(records[0]!.view).not.toBe(records[1]!.view)
    expect(records[0]!.documentRef).not.toBe(records[1]!.documentRef)
  })
  it('normalizes untrusted renderer data and bounds all correlation strings', () => {
    const { records, observer, view } = fixture()
    observer.renderer(view, { stage: 'arbitrary-private-stage', args: 'private' })
    expect(records).toHaveLength(0)
    observer.renderer(view, { stage: 'renderer-unknown', id: Infinity, channel: 'private'.repeat(10000), generation: { secret: 'private' }, args: 'private', result: 'private', error: 'private' })
    expect(records[0]).toMatchObject({ id: 0, stage: 'renderer-unknown' })
    expect(records[0]!.documentRef).toMatch(/^[a-f0-9]{24}$/)
    expect(Buffer.byteLength(JSON.stringify(records))).toBeLessThan(600)
    expect(JSON.stringify(records)).not.toContain('private')
  })
  it('allows a pending handler to remain identifiable and ignores diagnostic sink failures', async () => {
    const { records, observer, view } = fixture()
    observer.begin(view, 1, 'config:get', 'doc')
    expect(records.map(event => event.stage)).toEqual(['host-received'])
    const failing = new InvokeDiagnostics<object>(() => { throw new Error('disk unavailable') })
    const trace = failing.begin(view, 2, 'config:get', 'doc')
    trace.handler('resolved')
    let sends = 0
    await trace.reply(() => { sends++; return Promise.resolve(true) })
    expect(sends).toBe(1)
  })
  it('persists whole privacy-safe records within actual incident-log rotation bounds', async () => {
    const root = mkdtempSync(join(tmpdir(), 'br24-diag-'))
    try {
      vi.resetModules()
      const log = await import('./incident-diagnostics')
      log.installIncidentDiagnostics(root, () => undefined)
      const observer = new InvokeDiagnostics<object>(log.recordInvokeBoundary)
      const view = {}; observer.attach(view)
      for (let i = 0; i < 14000; i++) observer.renderer(view, { stage: 'renderer-unknown',
        id: i, channel: 'synthetic-private-channel', generation: 'synthetic-private-document',
        args: 'synthetic-private-payload', result: 'synthetic-private-result' })
      const files = readdirSync(root).filter(name => name.startsWith('incident.log'))
      expect(files.length).toBeGreaterThan(1)
      expect(files.length).toBeLessThanOrEqual(4)
      for (const name of files) {
        const file = join(root, name)
        expect(statSync(file).size).toBeLessThanOrEqual(1024 * 1024)
        const text = readFileSync(file, 'utf8')
        expect(text).not.toContain('synthetic-private')
        for (const line of text.trim().split('\n')) {
          const record = JSON.parse(line.slice('[incident] '.length))
          expect(record.event).toBe('invoke-boundary')
          expect(record.channel).toBe('other')
          expect(record.count).toBeGreaterThan(0)
        }
      }
    } finally {
      if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith('br24-diag-')) throw new Error('Unsafe fixture cleanup')
      rmSync(root, { recursive: true, force: true })
    }
  })
})
