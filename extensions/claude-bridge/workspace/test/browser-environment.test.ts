import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { createBrowserEnvironment } from '../src/browser-environment.js'
import type { InvokeFrame, HostAttachment, HostFrame, WorkspaceBackend } from '../src/protocol.js'

test('Tools usage relay reaches attached views and stops at detach', async () => {
  let delegated = 0
  const backend = createBrowserEnvironment({ root: os.tmpdir() }).wrap({ attach() { return {
    async receive() { delegated++ }, dispose() {},
  } } })
  const outputs: HostFrame[][] = [[], []]
  const views = outputs.map(out => backend.attach({ async postMessage(frame) { out.push(frame); return true } }))
  await views[0].receive({ kind: 'send', channel: 'toolusage:open', args: ['Bash'] })
  assert.deepEqual(outputs.map(out => out.at(-1)?.args), [['Bash'], ['Bash']])
  views[1].dispose()
  await views[0].receive({ kind: 'send', channel: 'toolusage:open', args: ['Read'] })
  assert.equal(outputs[0].length, 2)
  assert.equal(outputs[1].length, 1)
  assert.equal(delegated, 0)
  views[0].dispose()
})

test('browser environment owns file effects and replies per view without disposing the backend', async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'bridgeweb-environment-'))
  t.after(async () => {
    assert.equal(await fs.realpath(path.dirname(root)), await fs.realpath(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-environment-'))
    await fs.rm(root, { recursive: true, force: true })
  })
  const browser = createBrowserEnvironment({ root, timeoutMs: 1000 })
  let disposed = 0
  let selected: string[] | undefined
  let exportPath = ''
  const backend: WorkspaceBackend = { attach(view) {
    return {
      async receive(frame) {
        if (frame.kind !== 'invoke') return
        let result: unknown
        try {
          if (frame.channel === 'clipboard') result = await browser.environment.clipboard.readText()
          else if (frame.channel === 'upload') {
            selected = await browser.environment.pickPaths({ canSelectFiles: true, canSelectMany: true })
            result = selected
          } else if (frame.channel === 'download') {
            exportPath = (await browser.environment.savePath({ defaultUri: { fsPath: '/arbitrary/export.txt' } }))!
            await fs.writeFile(exportPath, 'actual exported bytes')
            result = { ok: true }
          } else if (frame.channel === 'folder') result = await browser.environment.pickPaths({ canSelectFolders: true })
          await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true, result })
        } catch (error) {
          await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: false, error: String(error) })
        }
      },
      dispose() { disposed++ },
    }
  } }
  const outputs: HostFrame[][] = [[], []]
  const views = outputs.map(out => browser.wrap(backend).attach({ async postMessage(frame) { out.push(frame); return true } }))
  const invoke = (channel: string, id = 1): InvokeFrame => ({ kind: 'invoke', channel, id, generation: 'document', args: [] })
  const effect = async (index: number, operation: string) => {
    for (let count = 0; count < 100; count++) {
      const found = outputs[index].find(frame => frame.channel === 'workspace:effect' && (frame.args as any[])[0].operation === operation)
      if (found) return (found.args as any[])[0]
      await new Promise(resolve => setTimeout(resolve, 5))
    }
    throw new Error(`Missing ${operation}`)
  }
  const answer = (view: HostAttachment, id: string, value: unknown) => view.receive({ kind: 'send', channel: 'workspace:effect-result', args: [id, value] })

  const copying = views[0].receive(invoke('clipboard'))
  const clipboard = await effect(0, 'clipboard-read')
  await answer(views[1], clipboard.id, 'other view')
  assert.equal(outputs[0].some(frame => frame.kind === 'invoke-reply'), false)
  await answer(views[0], clipboard.id, 'correct view')
  await copying
  assert.equal(outputs[0].at(-1)?.result, 'correct view')
  assert.equal(outputs[1].length, 0)

  const uploading = views[0].receive(invoke('upload', 2))
  const upload = await effect(0, 'upload')
  await answer(views[0], upload.id, [{ name: '../../note.txt', base64: Buffer.from('uploaded bytes').toString('base64') }])
  await uploading
  assert.ok(selected?.[0].startsWith(path.join(root, '.bridge-uploads') + path.sep))
  assert.equal(await fs.readFile(selected![0], 'utf8'), 'uploaded bytes')
  assert.equal(path.basename(selected![0]), '.._.._note.txt')

  const downloading = views[0].receive(invoke('download', 3))
  const download = await effect(0, 'download')
  assert.equal(download.payload.name, 'export.txt')
  assert.equal(Buffer.from(download.payload.base64, 'base64').toString(), 'actual exported bytes')
  assert.equal(outputs[0].some(frame => frame.kind === 'invoke-reply' && frame.id === 3), false)
  await answer(views[0], download.id, true)
  await downloading
  assert.equal(outputs[0].at(-1)?.ok, true)
  await assert.rejects(fs.stat(path.dirname(exportPath)), { code: 'ENOENT' })

  await views[0].receive({ ...invoke('save-dropped-file', 4), args: [Buffer.from('drop bytes').toString('base64'), 'drop.txt'] })
  const dropped = outputs[0].at(-1)?.result as string
  assert.equal(await fs.readFile(dropped, 'utf8'), 'drop bytes')
  assert.ok(dropped.startsWith(path.join(root, '.bridge-uploads') + path.sep))

  const choosing = views[1].receive(invoke('folder'))
  await effect(1, 'container-folder')
  views[1].dispose()
  await choosing
  assert.equal(outputs[1].at(-1)?.ok, false)
  assert.match(String(outputs[1].at(-1)?.error), /view closed/)
  assert.equal(disposed, 1)
  views[0].dispose()
})

test('browser environment rejects invalid uploads, native effects, unsafe URLs and abandoned requests', async () => {
  const browser = createBrowserEnvironment({ root: os.tmpdir(), timeoutMs: 10 })
  await assert.rejects(browser.environment.openDocument('/workspace/file'), /KaminIDE host feature, not Bridge/)
  await assert.rejects(browser.environment.openExternal('file:///etc/passwd'), /Unsupported external URL/)
  await assert.rejects(browser.environment.clipboard.readText(), /originating view/)
  let result: HostFrame | undefined
  const wrapped = browser.wrap({ attach(view) { return {
    async receive(frame) {
      await assert.rejects(browser.environment.clipboard.readText(), /timed out/)
      if (frame.kind === 'invoke') await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ok: true })
    }, dispose() {},
  } } }).attach({ async postMessage(frame) { result = frame; return true } })
  await wrapped.receive({ kind: 'invoke', channel: 'clipboard', id: 1, generation: 'doc', args: [] })
  assert.equal(result?.ok, true)
  await wrapped.receive({ kind: 'invoke', channel: 'save-dropped-file', id: 2, generation: 'doc', args: ['!!!!', 'bad.txt'] })
  assert.equal(result?.ok, false)
  assert.match(String(result?.error), /encoding/)
  wrapped.dispose()
})
