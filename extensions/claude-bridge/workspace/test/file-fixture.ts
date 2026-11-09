import assert from 'node:assert/strict'
import { EventEmitter } from 'node:events'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import type { TestContext } from 'node:test'
import { WorkspaceFiles } from '../src/files.js'
import type { ClientFrame, HostFrame } from '../src/protocol.js'

export async function fileFixture(t: TestContext) {
  const root = await mkdtemp(join(tmpdir(), 'bridgeweb-files-'))
  const lifecycle = new EventEmitter()
  const forwarded: ClientFrame[] = []
  const errors: Error[] = []
  let disposed = 0
  const files = new WorkspaceFiles({ attach(view) {
    return {
      async receive(frame) {
        forwarded.push(frame)
        if (frame.kind === 'invoke') await view.postMessage({ kind: 'invoke-reply', id: frame.id,
          generation: frame.generation, ok: false, error: 'Unsupported fixture backend channel' })
      },
      dispose() { disposed++; lifecycle.emit('dispose') },
    }
  } }, { root, onWatchError(error) { errors.push(error); lifecycle.emit('watch-error', error) } })
  t.after(async () => {
    files.dispose()
    const target = resolve(root)
    assert.equal(dirname(target), resolve(tmpdir()))
    assert.ok(basename(target).startsWith('bridgeweb-files-'))
    await rm(target, { recursive: true, force: true })
  })
  function client() {
    const frames: HostFrame[] = []
    const calls: string[] = []
    const events = new EventEmitter()
    const attachment = files.attach({ async postMessage(frame) { frames.push(frame); events.emit('frame', frame); return true } })
    let id = 0
    return {
      attachment, frames, calls,
      async reply(channel: string, ...args: unknown[]) {
        calls.push(channel)
        const requestId = ++id
        await attachment.receive({ kind: 'invoke', id: requestId, generation: 'file-document', channel, args })
        return frames.find(frame => frame.kind === 'invoke-reply' && frame.id === requestId)!
      },
      async invoke(channel: string, ...args: unknown[]) {
        const reply = await this.reply(channel, ...args)
        assert.equal(reply.ok, true, String(reply.error))
        return reply.result as any
      },
      async send(channel: string, ...args: unknown[]) { calls.push(channel); await attachment.receive({ kind: 'send', channel, args }) },
      next(channel: string, predicate: (value: any) => boolean = () => true): Promise<any> {
        return new Promise((resolve, reject) => {
          const timer = setTimeout(() => { events.off('frame', listener); reject(new Error(`Missing event ${channel}`)) }, 3000)
          const listener = (frame: HostFrame) => {
            if (frame.kind !== 'event' || frame.channel !== channel || !predicate((frame.args as any[])[0])) return
            clearTimeout(timer); events.off('frame', listener); resolve((frame.args as any[])[0])
          }
          events.on('frame', listener)
        })
      },
    }
  }
  function nextError(): Promise<Error> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { lifecycle.off('watch-error', listener); reject(new Error('Missing watch error')) }, 3000)
      const listener = (error: Error) => { clearTimeout(timer); lifecycle.off('watch-error', listener); resolve(error) }
      lifecycle.on('watch-error', listener)
    })
  }
  function waitDisposed(count: number): Promise<void> {
    if (disposed >= count) return Promise.resolve()
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { lifecycle.off('dispose', listener); reject(new Error('Missing view disposal')) }, 3000)
      const listener = () => {
        if (disposed < count) return
        clearTimeout(timer); lifecycle.off('dispose', listener); resolve()
      }
      lifecycle.on('dispose', listener)
    })
  }
  return { root, files, client, errors, forwarded, nextError, waitDisposed, disposals: () => disposed }
}
