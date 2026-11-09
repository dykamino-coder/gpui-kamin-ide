import { fileURLToPath } from 'node:url'
import vm from 'node:vm'
import { transformSync } from 'esbuild'
import { normalizePath } from 'vite'
import { expect, it } from 'vitest'
import { developmentPreview } from '../../vite.config'

it('builds React, DOM, both JSX runtimes and Refresh as one development graph in a production browser build', () => {
  const id = normalizePath(fileURLToPath(new URL('./preview-react.ts', import.meta.url)))
  const bundled = developmentPreview.load(id)!
  expect(bundled).toBeTruthy()
  expect(bundled).not.toContain('process.env.NODE_ENV')
  const context = vm.createContext({ module: { exports: {} }, console, setTimeout, clearTimeout })
  vm.runInContext(`window = globalThis;
    __REACT_DEVTOOLS_GLOBAL_HOOK__ = { supportsFiber: true, renderers: new Map(),
      inject(renderer) { this.renderers.set(0, renderer); this.preambleReady = typeof $RefreshSig$ === 'function'; return 0 },
      onCommitFiberRoot() {}, onScheduleFiberRoot() {}, onCommitFiberUnmount() {} }`, context)
  vm.runInContext(transformSync(bundled, { format: 'cjs', target: 'es2022' }).code, context)
  const { React, ReactDOM, ReactClient, JSXRuntime, JSXDevRuntime, reactVersion } = context.module.exports
  expect(React.version).toBe(reactVersion)
  expect(ReactDOM.version).toBe(reactVersion)
  expect(ReactClient.createRoot).toBeTypeOf('function')
  expect(React.createElement('div')._store).toBeDefined()
  expect(JSXRuntime.jsx('div', {})._store).toBeDefined()
  expect(JSXDevRuntime.jsxDEV('div', {}, undefined, false)._store).toBeDefined()
  // Injection precedes renderer registration, rather than merely filling the
  // globals after the production renderer has already initialized.
  expect(context.__REACT_DEVTOOLS_GLOBAL_HOOK__.renderers.size).toBe(1)
  expect(context.__REACT_DEVTOOLS_GLOBAL_HOOK__.preambleReady).toBe(true)
  expect(context.__REACT_DEVTOOLS_GLOBAL_HOOK__.renderers.get(0).scheduleRefresh).toBeTypeOf('function')
  expect(context.$RefreshSig$).toBeTypeOf('function')
})
