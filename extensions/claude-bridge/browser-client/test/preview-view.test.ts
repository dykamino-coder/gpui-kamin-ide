import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createPreviewView } from '../src/preview'
import type { PreviewState } from '../src/types'

// Minimal DOM: the view only creates, appends and clears plain containers.
class Element {
  children: Element[] = []
  append(child: Element) { this.children.push(child) }
  replaceChildren() { this.children = [] }
}
globalThis.document = { createElement: () => new Element() } as never

const descriptor = (revision: number, props: Record<string, unknown> = {}) => ({
  name: 'demo', entry: '/preview/remotes/demo/remoteEntry.js', exposedModule: './Panel', props, type: 'global' as const, revision,
})

function harness(hot = false) {
  let push!: (state: PreviewState) => void
  const mounts: { revision: number; build: string }[] = []
  const updates: unknown[] = []
  let unmounted = 0
  const view = createPreviewView({
    states: listener => { push = listener; return () => {} },
    render: async (_element, value, build) => {
      mounts.push({ revision: value.revision, build })
      return { hot, update: props => updates.push(props), unmount: () => { unmounted++ } }
    },
    rebuildDelayMs: 20,
  })
  return { view, push: (state: PreviewState) => push(state), mounts, updates, unmounted: () => unmounted }
}
const settle = () => new Promise(resolve => setTimeout(resolve, 0))

test('mounts a revision, applies props in place, remounts on a new revision and clears on unmount', async () => {
  const h = harness()
  h.push({ descriptor: descriptor(1), build: 'b1', available: true })
  await settle()
  assert.equal(h.view.status, 'ready')
  h.push({ descriptor: descriptor(1, { title: 'x' }), build: 'b1', available: true })
  assert.deepEqual(h.updates, [{ title: 'x' }])
  h.push({ descriptor: descriptor(2), build: 'b1', available: true })
  await settle()
  assert.deepEqual(h.mounts.map(m => m.revision), [1, 2])
  h.push({ descriptor: descriptor(2), build: null, available: false })
  assert.equal(h.view.status, 'unavailable')
  h.push({ descriptor: null, build: null, available: false })
  assert.equal(h.view.status, 'empty')
  assert.equal(h.unmounted(), 2)
  h.view.dispose()
})

test('a non-HMR rebuild remounts after the quiet period; an HMR remote keeps its mount', async () => {
  const plain = harness(false)
  plain.push({ descriptor: descriptor(1), build: 'b1', available: true })
  await settle()
  plain.push({ descriptor: descriptor(1), build: 'b2', available: true })
  assert.equal(plain.mounts.length, 1)
  await new Promise(resolve => setTimeout(resolve, 40))
  assert.deepEqual(plain.mounts.map(m => m.build), ['b1', 'b2'])
  plain.view.dispose()

  const hot = harness(true)
  hot.push({ descriptor: descriptor(1), build: 'b1', available: true })
  await settle()
  hot.push({ descriptor: descriptor(1), build: 'b2', available: true })
  await new Promise(resolve => setTimeout(resolve, 40))
  assert.equal(hot.mounts.length, 1)
  hot.view.dispose()
})
