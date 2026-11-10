// Диск, изоляция и CAS черновиков проверяются без браузерного кэша.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test, type TestContext } from 'node:test'
import { WorkspaceDrafts } from '../src/draft-state.js'

function fixture(t: TestContext) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-drafts-'))
  t.after(() => {
    assert.equal(path.dirname(root), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-drafts-'))
    fs.rmSync(root, { recursive: true })
  })
  const store = (workspace = '/workspace', server = 'ws://fixture', token = 'synthetic-draft-secret') => new WorkspaceDrafts(root, workspace, server, token)
  return { root, store, file: path.join(root, 'workspace-drafts.json') }
}
const empty = { text: '', attachments: [] }
const draft = { text: 'несохранённый текст', attachments: [{ path: '/workspace/proof.txt', name: 'proof.txt', ext: 'txt' }] }

test('acknowledged drafts survive restart, stale clears cannot overwrite, and empty revisions prevent resurrection', t => {
  const { store, file, root } = fixture(t)
  let host = store()
  assert.deepEqual(host.get('first'), { revision: 0, value: empty })
  assert.equal(host.update('first', 0, draft).accepted, true)
  host = store()
  assert.deepEqual(host.get('first'), { revision: 1, value: draft })
  assert.equal(host.update('first', 0, empty).accepted, false)
  assert.equal(host.update('first', 1, empty).accepted, true)
  assert.equal(store().update('first', 1, draft).accepted, false)
  assert.deepEqual(store().get('first'), { revision: 2, value: empty })
  assert.deepEqual(host.get('second'), { revision: 0, value: empty })
  assert.equal(fs.readFileSync(file, 'utf8').includes('synthetic-draft-secret'), false)
  assert.deepEqual(fs.readdirSync(root), ['workspace-drafts.json'])
  for (const other of [store('/other'), store(undefined, 'ws://other'), store(undefined, undefined, 'rotated')]) {
    assert.deepEqual(other.get('first'), { revision: 0, value: empty })
  }
  host.prune(['second'])
  assert.deepEqual(store().get('first'), { revision: 0, value: empty })
})

test('invalid and oversized drafts reject before changing accepted memory or disk; failed replacement leaves no temp file', t => {
  const { store, file, root } = fixture(t)
  const host = store()
  host.update('first', 0, draft)
  const before = fs.readFileSync(file, 'utf8')
  for (const input of [null, { text: 42, attachments: [] }, { ...empty, extra: true },
    { text: 'x'.repeat(256 * 1024 + 1), attachments: [] }, { ...empty, attachments: Array(33).fill(draft.attachments[0]) },
    { ...empty, attachments: [{ path: '/proof', name: 'proof', ext: '', dataUri: 'http://remote' }] },
    { ...empty, attachments: [{ path: '/proof', name: 'proof', ext: '', dataUri: 'data:image/png;base64,' + 'a'.repeat(8 * 1024 * 1024) }] }]) {
    assert.throws(() => host.update('first', 1, input), /Invalid|exceeds/)
    assert.equal(fs.readFileSync(file, 'utf8'), before)
  }
  for (const revision of [-1, 1.1, '1', NaN]) assert.throws(() => host.update('first', revision, empty), /revision/)
  assert.throws(() => host.update('__proto__', 0, empty), /session/)
  // A directory at the target makes rename fail on Windows and Linux alike.
  fs.unlinkSync(file); fs.mkdirSync(file)
  assert.throws(() => host.update('first', 1, empty))
  assert.deepEqual(host.get('first'), { revision: 1, value: draft })
  assert.deepEqual(fs.readdirSync(root), ['workspace-drafts.json'])
})

test('corrupt matching-scope disk never silently becomes an empty writable store', t => {
  const { store, file } = fixture(t)
  store().update('first', 0, draft)
  const saved = JSON.parse(fs.readFileSync(file, 'utf8'))
  for (const bad of ['{', 'null', JSON.stringify({ ...saved, states: { first: { revision: 0, value: draft } } }),
    JSON.stringify({ ...saved, states: { first: { revision: 1, value: { text: 2 } } } })]) {
    fs.writeFileSync(file, bad)
    assert.throws(() => store())
    assert.equal(fs.readFileSync(file, 'utf8'), bad)
  }
})

test('session and aggregate limits reject without changing accepted drafts', t => {
  const { store, file } = fixture(t)
  const host = store()
  for (let i = 0; i < 128; i++) host.update(`tab-${i}`, 0, empty)
  assert.throws(() => host.update('overflow', 0, empty), /Too many/)
  assert.deepEqual(store().get('overflow'), { revision: 0, value: empty })
  host.prune(['tab-0'])
  const large = { text: '', attachments: [{ path: '/proof', name: 'proof', ext: '',
    dataUri: 'data:image/png;base64,' + 'a'.repeat(7 * 1024 * 1024) }] }
  for (let i = 1; i <= 4; i++) host.update(`large-${i}`, 0, large)
  const before = fs.readFileSync(file, 'utf8')
  assert.throws(() => host.update('large-5', 0, large), /32 MiB/)
  assert.deepEqual(host.get('large-5'), { revision: 0, value: empty })
  assert.equal(fs.readFileSync(file, 'utf8'), before)
})
