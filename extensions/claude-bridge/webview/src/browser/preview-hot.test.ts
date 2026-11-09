import { afterEach, expect, it, vi } from 'vitest'
import * as Refresh from 'react-refresh/runtime'
import { captureRuntime, connectHot, shareRefresh, type RemoteRequire } from './preview-hot'
import { prepareEntry } from './preview-entry'

afterEach(() => vi.unstubAllGlobals())

it('delegates remote registrations and signatures to one runtime, preserving plugin boundary decisions', () => {
  const earlyImport = { ...Refresh, register: vi.fn() }
  const require: RemoteRequire = { i: [], c: { runtime: { exports: earlyImport } } }
  const dispose = shareRefresh(require, 'demo', Refresh)
  expect(require.c!.runtime.exports).toBe(earlyImport)
  expect(earlyImport.performReactRefresh).toBe(Refresh.performReactRefresh)
  const options = { module: { exports: {} }, factory(module: { exports: unknown }) { module.exports = { ...Refresh, register: vi.fn() } } }
  require.i!.forEach(intercept => intercept(options))
  options.factory(options.module)
  const remote = options.module.exports as typeof Refresh
  expect(remote.performReactRefresh).toBe(Refresh.performReactRefresh)
  expect(remote.createSignatureFunctionForTransform).toBe(Refresh.createSignatureFunctionForTransform)
  function Before() {}
  function After() {}
  function NewHook() {}
  remote.setSignature(Before, 'useState')
  remote.register(Before, 'App')
  remote.setSignature(After, 'useState')
  remote.register(After, 'App')
  const family = Refresh.getFamilyByID('demo:App')!
  expect(remote.performReactRefresh()?.updatedFamilies.has(family)).toBe(true)
  remote.setSignature(NewHook, 'useState,useState')
  remote.register(NewHook, 'App')
  expect(remote.performReactRefresh()?.staleFamilies.has(family)).toBe(true)
  expect(Refresh.getFamilyByID('App')).toBeUndefined()
  function Early() {}
  earlyImport.register(Early, 'early')
  expect(Refresh.getFamilyByID('demo:early')?.current).toBe(Early)
  dispose()
  expect(require.i).toEqual([])
})

it('captures the native runtime before requesting the exposed factory', () => {
  const require: RemoteRequire = { c: {} }
  const receive = vi.fn()
  const queue: unknown[] = []
  queue.push = chunk => { (chunk as [unknown, unknown, (runtime: RemoteRequire) => void])[2](require); return 0 }
  captureRuntime(queue, receive)
  expect(receive).toHaveBeenCalledWith(require)
})

it.each(['webpack', 'rspack'])('attaches before %s startup, delegates injection and suppresses the duplicate dev client', prefix => {
  const globals: Record<string, unknown> = {}
  const name = `${prefix}Chunkdemo`
  const require: RemoteRequire = { i: [], c: {} }
  const inject = vi.fn(), client = vi.fn()
  const disposeRuntime: (() => void)[] = []
  const dispose = prepareEntry(`var chunks = self["${name}"] = self["${name}"] || [];`, globals,
    require => disposeRuntime.push(shareRefresh(require, 'demo', { ...Refresh, injectIntoGlobalHook: inject })))
  const queue = globals[name] as unknown[]
  queue.push = chunk => { (chunk as [unknown, unknown, (runtime: RemoteRequire) => void])[2](require); return 0 }
  const execute = (id: string, factory: (module: { exports: unknown }) => void) => {
    const options = { id, module: { exports: {} }, factory }
    require.i!.forEach(intercept => intercept(options))
    options.factory(options.module)
    return options.module.exports as typeof Refresh
  }
  const remote = execute('react-refresh/runtime.js', module => { module.exports = { ...Refresh, injectIntoGlobalHook: vi.fn() } })
  remote.injectIntoGlobalHook({} as Window)
  expect(inject).not.toHaveBeenCalled()
  execute('/deps/@rspack/dev-server/client/index.js?hot=true', client)
  execute('/deps/webpack-dev-server/client/index.js?hot=true', client)
  expect(client).not.toHaveBeenCalled()
  disposeRuntime.forEach(dispose => dispose())
  dispose()
  expect(Object.getOwnPropertyDescriptor(globals, name)?.get).toBeUndefined()
})

it('applies native HMR and remounts only when the runtime rejects an update', async () => {
  let socket: { onmessage?: (event: { data: string }) => void; close(): void }
  vi.stubGlobal('location', new URL('https://workspace.test/'))
  const close = vi.fn()
  vi.stubGlobal('WebSocket', class {
    constructor(url: URL) { expect(url.href).toBe('wss://workspace.test/preview/remotes/demo/ws'); socket = this }
    close = close
  })
  let hash = 'before'
  const hot = { status: () => 'idle', addStatusHandler: vi.fn(), removeStatusHandler: vi.fn(),
    check: vi.fn(async () => { hash = 'after'; return ['App'] as unknown[] | null }) }
  const applied = vi.fn(), invalidated = vi.fn()
  const dispose = connectHot({ c: { App: { exports: {}, hot } }, h: () => hash }, 'demo', applied, invalidated)
  const message = (type: string, data?: string) => socket.onmessage?.({ data: JSON.stringify({ type, data }) })
  message('hash', 'after'); message('ok')
  await vi.waitFor(() => expect(applied).toHaveBeenCalledOnce())
  expect(hot.check).toHaveBeenCalledWith(true)
  expect(invalidated).not.toHaveBeenCalled()
  hot.check.mockResolvedValueOnce(null)
  message('hash', 'restart'); message('ok')
  await vi.waitFor(() => expect(invalidated).toHaveBeenCalledOnce())
  dispose()
  expect(close).toHaveBeenCalledOnce()
  expect(hot.removeStatusHandler).toHaveBeenCalledWith(hot.addStatusHandler.mock.calls[0][0])
})
