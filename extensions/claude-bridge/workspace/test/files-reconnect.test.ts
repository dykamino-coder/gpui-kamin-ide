import assert from 'node:assert/strict'
import { mkdir, readFile, rename, truncate, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { test } from 'node:test'
import { setTimeout as sleep } from 'node:timers/promises'
import { MAX_FILE_BYTES } from '../src/file-operations.js'
import { fileFixture } from './file-fixture.js'

async function externalAtomicReplace(source: string, destination: string) {
  // Windows can temporarily reject replacing a watched/read target. Retry only
  // the external editor fixture action; persistent/other failures still fail.
  const deadline = Date.now() + 500
  for (;;) {
    try { await rename(source, destination); return }
    catch (error) {
      if (process.platform !== 'win32' || (error as NodeJS.ErrnoException).code !== 'EPERM'
        || Date.now() >= deadline) throw error
      await sleep(25)
    }
  }
}

test('host file watches survive zero sockets; re-attach refreshes snapshots without replaying mutations', async t => {
  const f = await fileFixture(t)
  const first = f.client()
  const path = join(f.root, 'draft.txt')
  await first.invoke('file-viewer:write', path, 'user write once')
  await first.invoke('file-tree:rename', path, join(f.root, 'renamed.txt'))
  const renamed = join(f.root, 'renamed.txt')
  await first.send('file-viewer:watch', renamed)
  await first.send('file-tree:watch', f.root)
  await first.send('workspace:sync')
  const state = (first.frames.find(frame => frame.channel === 'workspace:files-state')!.args as any[])[0]
  assert.deepEqual(state.files, [renamed])
  assert.deepEqual(state.trees, [f.root])
  first.attachment.dispose()
  const detachedCount = first.frames.length
  // A read error with zero sockets proves the host still executes its watcher,
  // rather than reinstalling it only when another page arrives.
  const offlineError = f.nextError()
  await truncate(renamed, MAX_FILE_BYTES + 1)
  assert.match((await offlineError).message, /8 MiB/)
  await writeFile(renamed, 'agent changed while offline')
  await mkdir(join(f.root, 'created-offline'))
  const reopened = f.client()
  // Child FilePanel effects can install a watch before the parent's sync
  // effect. The opaque identity must bind that first frame to retained state.
  await reopened.attachment.receive({ kind: 'send', channel: 'file-viewer:watch', args: [renamed], fileViewId: state.id })
  await reopened.send('workspace:sync', { fileViewId: state.id })
  const frames = reopened.frames
  assert.equal((frames.find(frame => frame.channel === 'workspace:files-state')!.args as any[])[0].id, state.id)
  assert.ok(frames.some(frame => frame.channel === 'file-viewer:external-change'
    && (frame.args as any[])[0].content === 'agent changed while offline'))
  assert.ok(frames.some(frame => frame.channel === 'file-tree:change' && (frame.args as any[])[0].resync))
  assert.ok((await reopened.invoke('file-tree:list-dir', f.root)).some((entry: any) => entry.name === 'created-offline'))
  assert.equal(await readFile(renamed, 'utf8'), 'agent changed while offline')
  assert.equal((await reopened.reply('file-viewer:read', path)).ok, false)
  // Only explicit initial invokes mutated disk; sync delegates just a read
  // snapshot request to the shared host, never a recorded write or rename.
  assert.deepEqual(f.forwarded.map(frame => frame.channel), ['workspace:sync', 'workspace:sync'])
  const nextFile = reopened.next('file-viewer:external-change', value => value.content === 'after reopen')
  const nextTree = reopened.next('file-tree:change', value => value.changedPath === join(f.root, 'later.txt'))
  await writeFile(join(f.root, 'atomic'), 'after reopen')
  await externalAtomicReplace(join(f.root, 'atomic'), renamed)
  await writeFile(join(f.root, 'later.txt'), 'later')
  await Promise.all([nextFile, nextTree])
  assert.equal(first.frames.length, detachedCount)
  reopened.attachment.dispose()
  // Closing and opening a new tab, without sessionStorage, adopts host state.
  const fresh = f.client()
  await fresh.send('workspace:sync')
  assert.equal((fresh.frames.find(frame => frame.channel === 'workspace:files-state')!.args as any[])[0].id, state.id)
})

test('two pages get independent host file views even when a new tab copied sessionStorage', async t => {
  const f = await fileFixture(t)
  const path = join(f.root, 'shared.txt')
  await writeFile(path, 'initial')
  const first = f.client()
  await first.send('file-viewer:watch', path)
  await first.send('workspace:sync')
  const id = (first.frames.find(frame => frame.channel === 'workspace:files-state')!.args as any[])[0].id
  const second = f.client()
  await second.send('workspace:sync', { fileViewId: id })
  const state = (second.frames.find(frame => frame.channel === 'workspace:files-state')!.args as any[])[0]
  assert.notEqual(state.id, id)
  assert.deepEqual(state.files, [path])
  await second.send('file-viewer:unwatch', path)
  const secondCount = second.frames.length
  const changed = first.next('file-viewer:external-change', value => value.content === 'still watching')
  await writeFile(path, 'still watching')
  await changed
  assert.equal(second.frames.length, secondCount)
  assert.deepEqual(f.errors, [])
})
