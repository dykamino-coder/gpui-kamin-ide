import { beforeEach, expect, it, vi } from 'vitest'
const posted = vi.hoisted(() => ({ frames: [] as any[] }))
vi.mock('./webview-api.js', () => ({ vscodeApi: { postMessage: (frame: unknown) => posted.frames.push(frame) } }))
let listeners: Map<string, (event: any) => void>
beforeEach(() => {
  vi.resetModules()
  posted.frames = []
  listeners = new Map()
  vi.stubGlobal('window', { addEventListener: (name: string, cb: (event: any) => void) => listeners.set(name, cb) })
})
it('correlates request and reply with document identity without including arguments or result in diagnostics', async () => {
  const { inv } = await import('./bridge-transport')
  const promise = inv('config:get', { token: 'synthetic-private-argument' })
  const request = posted.frames.find(frame => frame.kind === 'invoke')
  expect(request.generation).toEqual(expect.any(String))
  const sent = posted.frames.find(frame => frame.stage === 'renderer-sent')
  expect(sent).toMatchObject({ kind: 'invoke-diagnostic', id: request.id, generation: request.generation, channel: 'config:get' })
  listeners.get('message')!({ data: { kind: 'invoke-reply', id: request.id, generation: request.generation, ok: true, result: 'synthetic-private-result' } })
  expect(await promise).toBe('synthetic-private-result')
  expect(posted.frames.at(-1)).toMatchObject({ stage: 'renderer-received', id: request.id })
  const diagnostics = JSON.stringify(posted.frames.filter(frame => frame.kind === 'invoke-diagnostic'))
  expect(diagnostics).not.toContain('synthetic-private')
})
it('distinguishes duplicate, unknown and another-document replies without retrying an invoke', async () => {
  const { inv } = await import('./bridge-transport')
  const promise = inv('hooks:set-plugin-approval', { synthetic: 'mutation' })
  const request = posted.frames.find(frame => frame.kind === 'invoke')
  const receive = (id: number, generation = request.generation) => listeners.get('message')!({ data: { kind: 'invoke-reply', id, generation, ok: true } })
  receive(request.id); await promise
  receive(request.id); receive(999); receive(1, 'previous-document')
  const stages = posted.frames.filter(frame => frame.kind === 'invoke-diagnostic').map(frame => frame.stage)
  expect(stages).toContain('renderer-duplicate')
  expect(stages).toContain('renderer-unknown')
  expect(stages).toContain('renderer-other-document')
  expect(posted.frames.filter(frame => frame.kind === 'invoke')).toHaveLength(1)
})
it('records document end as diagnostics without pretending pending mutation was cancelled', async () => {
  const { inv } = await import('./bridge-transport')
  let settled = false
  void inv('hooks:set-plugin-approval', 'synthetic').then(() => { settled = true })
  expect(listeners.has('pagehide')).toBe(true)
  listeners.get('pagehide')!({})
  await Promise.resolve()
  expect(posted.frames.at(-1)).toMatchObject({ kind: 'invoke-diagnostic', stage: 'renderer-ended' })
  expect(settled).toBe(false)
})
