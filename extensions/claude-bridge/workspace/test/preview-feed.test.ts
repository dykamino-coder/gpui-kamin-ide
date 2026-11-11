import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'
import { test } from 'node:test'
import { WorkspacePreview, type PreviewSnapshot } from '../src/preview.js'
import { createPreviewFeed } from '../src/preview-feed.js'

const until = async (check: () => boolean) => {
  const deadline = Date.now() + 5000
  while (!check()) { assert.ok(Date.now() < deadline, 'condition timed out'); await new Promise(resolve => setTimeout(resolve, 5)) }
}

test('one host-side sampler pushes mount, props, rebuild and unmount to every subscriber', async t => {
  let build = 'one'
  let requests = 0
  const dev = createServer((_request, response) => { requests++; response.end(build) })
  await new Promise<void>(resolve => dev.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise<void>(resolve => dev.close(() => resolve())))
  const preview = new WorkspacePreview()
  const feed = createPreviewFeed(preview, 20)
  t.after(() => feed.close())

  const first: PreviewSnapshot[] = []
  const stopFirst = feed.subscribe(state => first.push(state))
  await until(() => first.length === 1)
  assert.deepEqual(first[0], { descriptor: null, build: null, available: false })
  // Nothing is mounted: no sampling traffic.
  await new Promise(resolve => setTimeout(resolve, 60))
  assert.equal(requests, 0)

  preview.mount({ name: 'demo', entry: `http://127.0.0.1:${(dev.address() as AddressInfo).port}/remoteEntry.js`, exposedModule: './Panel' })
  await until(() => first.at(-1)?.available === true)
  const mounted = first.at(-1)!
  const second: PreviewSnapshot[] = []
  const stopSecond = feed.subscribe(state => second.push(state))
  assert.deepEqual(second, [mounted])

  preview.update({ title: 'changed' })
  await until(() => second.at(-1)?.descriptor?.props.title === 'changed')
  build = 'two'
  await until(() => second.at(-1)?.build !== mounted.build)
  assert.equal(first.at(-1)!.build, second.at(-1)!.build)
  const delivered = first.length
  await new Promise(resolve => setTimeout(resolve, 80))
  assert.equal(first.length, delivered, 'unchanged samples are not re-sent')

  stopFirst(); stopSecond()
  const idle = requests
  await new Promise(resolve => setTimeout(resolve, 80))
  assert.ok(requests <= idle + 1, 'sampling stops without subscribers')

  const third: PreviewSnapshot[] = []
  feed.subscribe(state => third.push(state))
  await until(() => third.length > 0)
  preview.unmount()
  await until(() => third.at(-1)?.descriptor === null)
})
