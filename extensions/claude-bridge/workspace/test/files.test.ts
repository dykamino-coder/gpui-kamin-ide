import assert from 'node:assert/strict'
import { mkdir, readFile, rename, truncate, unlink, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { test } from 'node:test'
import { setTimeout as delay } from 'node:timers/promises'
import { FILE_INVOKES, MAX_FILE_BYTES } from '../src/file-operations.js'
import { FILE_SENDS } from '../src/file-watches.js'
import { fileFixture } from './file-fixture.js'

test('all file invoke channels act on real container paths and preserve reply identity', async t => {
  const f = await fileFixture(t)
  const client = f.client()
  await mkdir(join(f.root, 'nested'))
  const text = 'Привет\r\nUTF-8: 🙂\n'
  const info = await client.invoke('file-viewer:write', 'draft.txt', text)
  assert.equal(info.size, Buffer.byteLength(text))
  assert.equal(typeof info.mtimeMs, 'number')
  assert.equal(await readFile(join(f.root, 'draft.txt'), 'utf8'), text)
  assert.deepEqual(await client.invoke('file-viewer:read', 'draft.txt'), { content: text, ...info })
  await client.invoke('file-viewer:write', 'draft.txt', 'x')
  assert.equal(await readFile(join(f.root, 'draft.txt'), 'utf8'), 'x')
  const binary = Buffer.from([0, 1, 127, 128, 255])
  await writeFile(join(f.root, 'image.bin'), binary)
  const read = await client.invoke('file-viewer:read-binary', join(f.root, 'image.bin'))
  assert.deepEqual(Buffer.from(read.base64, 'base64'), binary)
  assert.equal(read.size, binary.length)
  const listing = await client.invoke('file-tree:list-dir', '.')
  assert.deepEqual(listing.map((entry: any) => [entry.name, entry.isDir]), [['nested', true], ['draft.txt', false], ['image.bin', false]])
  assert.ok(listing.every((entry: any) => entry.path === join(f.root, entry.name) && typeof entry.mtimeMs === 'number'))
  assert.equal((await client.invoke('file-tree:rename', 'draft.txt', 'nested/final.txt')).size, 1)
  assert.equal(await readFile(join(f.root, 'nested/final.txt'), 'utf8'), 'x')
  await client.invoke('file-tree:rename', 'nested', 'renamed')
  assert.equal(await readFile(join(f.root, 'renamed/final.txt'), 'utf8'), 'x')
  assert.deepEqual(new Set(client.frames.filter(frame => frame.kind === 'invoke-reply').map(frame => frame.generation)), new Set(['file-document']))
  assert.equal(f.forwarded.length, 0)
  assert.deepEqual(new Set(client.calls), new Set(FILE_INVOKES))
})

test('file errors are explicit replies; invalid writes do not truncate existing bytes', async t => {
  const f = await fileFixture(t)
  const client = f.client()
  await writeFile(join(f.root, 'keep.txt'), 'keep')
  const bad: [string, ...unknown[]][] = [
    ['file-viewer:read', 'missing'], ['file-viewer:read', '.'],
    ['file-viewer:read', ''], ['file-viewer:read', 'bad\0path'],
    ['file-viewer:read', 123], ['file-viewer:read', 'keep.txt', 'extra'],
    ['file-viewer:write', 'keep.txt', 123],
    ['file-viewer:write', 'keep.txt', '🙂'.repeat(MAX_FILE_BYTES / 4 + 1)],
    ['file-viewer:write', 'no-parent/file', 'data'],
    ['file-tree:rename', 'missing', 'next'], ['file-tree:list-dir', 'keep.txt'],
  ]
  for (const [channel, ...args] of bad) {
    const reply = await client.reply(channel, ...args)
    assert.equal(reply.ok, false, `${channel} must fail`)
    assert.equal(typeof reply.error, 'string')
    assert.equal(reply.generation, 'file-document')
  }
  assert.equal(await readFile(join(f.root, 'keep.txt'), 'utf8'), 'keep')
  const large = join(f.root, 'large')
  await writeFile(large, '')
  await truncate(large, MAX_FILE_BYTES + 1)
  for (const channel of ['file-viewer:read', 'file-viewer:read-binary']) {
    assert.match(String((await client.reply(channel, large)).error), /8 MiB/)
  }
  await writeFile(large, Buffer.alloc(2 * 1024 * 1024))
  assert.match(String((await client.reply('file-viewer:read', large)).error), /encoded frame limit/)
  assert.equal((await client.invoke('file-viewer:read-binary', large)).size, 2 * 1024 * 1024)
})

test('absolute paths outside root name the same container machine, not a root sandbox', async t => {
  const f = await fileFixture(t)
  const other = await fileFixture(t)
  const path = join(other.root, 'outside.txt')
  await f.client().invoke('file-viewer:write', path, 'same machine')
  assert.equal(await readFile(path, 'utf8'), 'same machine')
})

test('file and recursive tree watches are view-owned and survive atomic replacement', async t => {
  const f = await fileFixture(t)
  const a = f.client()
  const b = f.client()
  const path = join(f.root, 'watched.txt')
  const nested = join(f.root, 'nested')
  await mkdir(nested)
  await writeFile(path, 'before')
  const seen = new Set<string>()
  async function send(client: typeof a, channel: string, ...args: unknown[]) {
    seen.add(channel); await client.send(channel, ...args)
  }
  await send(a, 'file-viewer:watch', path)
  await send(a, 'file-viewer:watch', path)
  await send(b, 'file-viewer:watch', path)
  await send(a, 'file-tree:watch', f.root)
  const changed = a.next('file-tree:change', value => value.changedPath === join(nested, 'new.txt'))
  await writeFile(join(nested, 'new.txt'), 'nested')
  assert.deepEqual(await changed, { root: f.root, changedDir: nested, changedPath: join(nested, 'new.txt') })
  const updates = [a.next('file-viewer:external-change', value => value.content === 'atomic'), b.next('file-viewer:external-change', value => value.content === 'atomic')]
  await writeFile(join(f.root, 'replacement'), 'atomic')
  await rename(join(f.root, 'replacement'), path)
  for (const update of await Promise.all(updates)) { assert.equal(update.path, path); assert.equal(typeof update.mtimeMs, 'number') }
  await send(a, 'file-viewer:unwatch', path)
  await send(a, 'file-tree:unwatch', f.root)
  const aCount = a.frames.length
  const bUpdate = b.next('file-viewer:external-change', value => value.content === 'only-b')
  await writeFile(path, 'only-b')
  await bUpdate
  await delay(100)
  assert.equal(a.frames.length, aCount)
  assert.deepEqual(seen, new Set(FILE_SENDS))
  assert.deepEqual(f.errors, [])
})

test('deleted files never generate fabricated empty content and recreation is observed', async t => {
  const f = await fileFixture(t)
  const client = f.client()
  const path = join(f.root, 'return.txt')
  await writeFile(path, 'before')
  await client.send('file-viewer:watch', path)
  await unlink(path)
  await delay(100)
  assert.equal(client.frames.length, 0)
  const update = client.next('file-viewer:external-change', value => value.content === 'returned')
  await writeFile(path, 'returned')
  await update
  assert.deepEqual(f.errors, [])
})

test('oversized watched files report an asynchronous error and recover without empty updates', async t => {
  const f = await fileFixture(t)
  const client = f.client()
  const path = join(f.root, 'growing.txt')
  await writeFile(path, 'before')
  await client.send('file-viewer:watch', path)
  const error = f.nextError()
  await truncate(path, MAX_FILE_BYTES + 1)
  assert.match((await error).message, /8 MiB/)
  assert.equal(client.frames.length, 0)
  const recovered = client.next('file-viewer:external-change', value => value.content === 'small')
  await writeFile(path, 'small')
  await recovered
})

test('watch limits/errors are explicit and disposal releases only that view', async t => {
  const f = await fileFixture(t)
  const a = f.client()
  const b = f.client()
  await assert.rejects(a.send('file-viewer:watch', 'path', 'extra'), /one file watch path/)
  await assert.rejects(a.send('file-viewer:watch', null), /container file path/)
  await assert.rejects(a.send('file-viewer:watch', 'no-parent/file'), /ENOENT/)
  await writeFile(join(f.root, 'not-dir'), '')
  await assert.rejects(a.send('file-tree:watch', 'not-dir'), /requires a directory/)
  for (let i = 0; i < 64; i++) await a.send('file-viewer:watch', `pending-${i}`)
  await assert.rejects(a.send('file-viewer:watch', 'one-too-many'), /subscription limit/)
  await a.send('file-viewer:unwatch', 'pending-0')
  await a.send('file-viewer:watch', 'one-too-many')
  await b.send('file-viewer:watch', 'one-too-many')
  a.attachment.dispose(); a.attachment.dispose()
  assert.equal(f.disposals(), 1)
  const update = b.next('file-viewer:external-change', value => value.content === 'alive')
  await writeFile(join(f.root, 'one-too-many'), 'alive')
  await update
  assert.equal(a.frames.length, 0)
  f.files.dispose(); f.files.dispose()
  assert.equal(f.disposals(), 2)
  await assert.rejects(b.invoke('file-viewer:read', 'one-too-many'), /disposed/)
  assert.throws(() => f.client(), /disposed/)
})

test('unknown channels and diagnostics go to the shared backend unchanged', async t => {
  const f = await fileFixture(t)
  const client = f.client()
  const reply = await client.reply('not-a-file-handler', 123)
  assert.equal(reply.ok, false)
  await client.send('shared:event', 'data')
  const diagnostic = { kind: 'invoke-diagnostic', id: 1, generation: 'file-document', channel: 'file-viewer:read', stage: 'renderer-received' } as const
  await client.attachment.receive(diagnostic)
  assert.deepEqual(f.forwarded.map(frame => frame.kind), ['invoke', 'send', 'invoke-diagnostic'])
  assert.equal(f.forwarded[2], diagnostic)
})
