import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { readFile, symlink, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { test } from 'node:test'
import { promisify } from 'node:util'
import { WorkspaceFiles } from '../src/files.js'
import { fileFixture } from './file-fixture.js'

test('symlink paths preserve the container filesystem semantics', async t => {
  const f = await fileFixture(t)
  const client = f.client()
  const target = join(f.root, 'target.txt')
  const link = join(f.root, 'link.txt')
  await writeFile(target, 'target')
  try { await symlink(target, link, 'file') }
  catch (error) {
    if (process.platform === 'win32' && (error as NodeJS.ErrnoException).code === 'EPERM') {
      t.skip('Windows identity lacks symlink privilege; Linux CI exercises this path'); return
    }
    throw error
  }
  assert.equal((await client.invoke('file-viewer:read', link)).content, 'target')
  await client.invoke('file-viewer:write', link, 'through link')
  assert.equal(await readFile(target, 'utf8'), 'through link')
})

test('FIFO selections fail promptly without truncation or a blocked read', { skip: process.platform === 'win32' }, async t => {
  const f = await fileFixture(t)
  const client = f.client()
  const fifo = join(f.root, 'owned-fifo')
  await promisify(execFile)('mkfifo', [fifo], { timeout: 3000 })
  assert.match(String((await client.reply('file-viewer:read', fifo)).error), /regular file/)
  assert.equal((await client.reply('file-viewer:write', fifo, 'text')).ok, false)
})

test('a backend disposal failure still releases every other view', async t => {
  const f = await fileFixture(t)
  let disposed = 0
  const files = new WorkspaceFiles({ attach() {
    return { async receive() { throw new Error('Unused') }, dispose() { disposed++; throw new Error('Fixture dispose failure') } }
  } }, { root: f.root, onWatchError(error) { assert.fail(error.message) } })
  for (let i = 0; i < 2; i++) {
    const view = files.attach({ async postMessage() { assert.fail('Disposed view emitted'); return false } })
    await view.receive({ kind: 'send', channel: 'file-viewer:watch', args: [join(f.root, `pending-${i}`)] })
  }
  assert.throws(() => files.dispose(), error => error instanceof AggregateError && error.errors.length === 2)
  assert.equal(disposed, 2)
  files.dispose()
})
