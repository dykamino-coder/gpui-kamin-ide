import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { test } from 'node:test'
import { readFilePanelState, type FilePanelState } from '../../webview/shared/file-panel-state.js'
import { fileFixture } from './file-fixture.js'

function panel(root = resolve('/workspace')): FilePanelState {
  const a = join(root, 'a.txt'), b = join(root, 'b.html')
  return { version: 1, tabs: {
    first: { files: [b, a], active: a, selection: { path: a, startLine: 2, endLine: 3, text: 'selected context', anchor: 24, head: 6 } },
    second: { files: [b], active: b, selection: null },
  }, previewModes: [[b, 'preview']], attachActiveFile: true }
}
function snapshot(client: Awaited<ReturnType<typeof fileFixture>>['client'] extends () => infer T ? T : never): any {
  return (client.frames.filter(frame => frame.channel === 'workspace:files-state').at(-1)!.args as any[])[0]
}

test('bounded FilePanel schema whitelists data, clones inputs and rejects corrupt or oversized projections', () => {
  const input = { ...panel(), documents: 'never retained', credentials: 'never retained' }
  const result = readFilePanelState(input)
  assert.deepEqual(result, panel())
  result.tabs.first.files.pop()
  assert.equal(input.tabs.first.files.length, 2)
  const corrupt: unknown[] = [null, { ...panel(), version: 2 }, { ...panel(), attachActiveFile: 1 }]
  const alter = (change: (value: FilePanelState) => void) => { const value = panel(); change(value); corrupt.push(value) }
  alter(p => { p.tabs.first.files = ['relative.txt'] })
  alter(p => { p.tabs.first.files.push(p.tabs.first.files[0]) })
  alter(p => { p.tabs.first.active = '/workspace/closed' })
  alter(p => { p.tabs.first.selection!.path = '/workspace/closed' })
  alter(p => { p.tabs.first.selection!.startLine = 0 })
  alter(p => { p.tabs.first.selection!.head = -1 })
  alter(p => { delete p.tabs.first.selection!.head })
  alter(p => { p.previewModes = [['/workspace/a', 'invalid' as 'code']] })
  alter(p => { p.tabs = Object.fromEntries(Array.from({ length: 65 }, (_, i) => [`chat-${i}`, p.tabs.first])) })
  alter(p => { p.tabs.first.files = Array.from({ length: 129 }, (_, i) => `/workspace/${i}`) })
  alter(p => { p.previewModes = Array.from({ length: 257 }, (_, i) => [`/workspace/${i}`, 'code']) })
  // Byte bound is UTF-8, not JavaScript string length.
  alter(p => { p.tabs.first.selection!.text = 'я'.repeat(1024 * 1024) })
  for (const value of corrupt) assert.throws(() => readFilePanelState(value))
})

test('zero-page FilePanel recovery retains chat mapping, order, active editor, context and explicit closes without mutations', async t => {
  const f = await fileFixture(t), first = f.client(), state = panel(f.root)
  await first.invoke('file-viewer:write', join(f.root, 'a.txt'), 'disk bytes')
  await first.send('file-viewer:watch', join(f.root, 'a.txt'))
  await first.send('workspace:file-panel', state)
  // Host must clone the caller's object, not retain a reference.
  state.tabs.first.files.reverse()
  await first.send('workspace:sync')
  assert.deepEqual(snapshot(first).panel, panel(f.root))
  const id = snapshot(first).id
  first.attachment.dispose()
  const reopened = f.client()
  await reopened.send('workspace:sync', { fileViewId: id })
  assert.equal(snapshot(reopened).id, id)
  assert.deepEqual(snapshot(reopened).panel, panel(f.root))
  // Watches and open files have different lifetimes; a watched but closed path
  // must not be resurrected by the fallback used for legacy views.
  const closed = panel(f.root)
  closed.tabs.first = { files: [], active: null, selection: null }
  await reopened.send('workspace:file-panel', closed)
  reopened.attachment.dispose()
  const fresh = f.client()
  await fresh.send('workspace:sync')
  assert.deepEqual(snapshot(fresh).panel, closed)
  assert.deepEqual(snapshot(fresh).files, [join(f.root, 'a.txt')])
  assert.equal(await readFile(join(f.root, 'a.txt'), 'utf8'), 'disk bytes')
  assert.deepEqual(f.forwarded.map(frame => frame.channel), ['workspace:sync', 'workspace:sync', 'workspace:sync'])
})

test('concurrent pages clone independent controls; invalid frames preserve the previous host state', async t => {
  const f = await fileFixture(t), first = f.client(), original = panel(f.root)
  await first.send('workspace:file-panel', original)
  await first.send('workspace:sync')
  const id = snapshot(first).id
  const second = f.client()
  // The first child effect may arrive before the parent's sync.
  const independent = panel(f.root)
  independent.attachActiveFile = false
  independent.tabs.first = { files: [], active: null, selection: null }
  await second.attachment.receive({ kind: 'send', channel: 'workspace:file-panel', fileViewId: id, args: [independent] })
  await second.send('workspace:sync', { fileViewId: id })
  assert.notEqual(snapshot(second).id, id)
  assert.deepEqual(snapshot(second).panel, independent)
  await first.send('workspace:sync')
  assert.deepEqual(snapshot(first).panel, original)
  for (const value of [{ ...independent, version: 9 }, { ...independent, tabs: { broken: {} } }]) {
    await assert.rejects(second.send('workspace:file-panel', value))
  }
  await assert.rejects(second.send('workspace:file-panel', independent, independent))
  await second.send('workspace:sync')
  assert.deepEqual(snapshot(second).panel, independent)
  assert.deepEqual(f.errors, [])
})
