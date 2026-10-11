import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const h = vi.hoisted(() => ({ effects: [] as Array<() => void>, ref: 0, serialize: vi.fn(() => 'screen'), terminals: [] as any[], writes: vi.fn(), container: { querySelector: () => null, addEventListener: () => {} } }))
vi.mock('preact/hooks', () => ({ useRef: () => ({ current: h.ref++ % 3 === 0 ? h.container : null }), useState: () => [false, () => {}], useEffect: (fn: () => (() => void) | undefined) => { const end=fn(); if(end) h.effects.push(end) } }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: h.writes, removeItem: () => {} } }))
vi.mock('./useBridge', () => ({ useBridge: () => ({ resize: () => {}, sendInput: () => {} }) }))
vi.mock('../theme/terminal-theme', () => ({ buildTerminalTheme: () => ({}) }))
vi.mock('../theme/apply-theme', () => ({ resolvedTheme: { value: 'dark' }, themeNonce: { value: 0 } }))
vi.mock('@xterm/xterm', () => ({ Terminal: class {
  callbacks = new Set<() => void>(); cols=80; rows=24; options={}; buffer={ active: { length: 1, viewportY:0, baseY:0, getLine:()=>({translateToString:()=> 'screen'}) } };
  constructor(){h.terminals.push(this)}
  loadAddon(){} open(){} attachCustomKeyEventHandler(){} onData(){} onResize(){} onScroll(){} onRender(){} dispose(){}
  onWriteParsed(callback:()=>void){this.callbacks.add(callback);return {dispose:()=>this.callbacks.delete(callback)}}
  write(){for(const callback of [...this.callbacks]) callback()}
} }))
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit(){} } }))
vi.mock('@xterm/addon-web-links', () => ({ WebLinksAddon: class {} }))
vi.mock('@xterm/addon-webgl', () => ({ WebglAddon: class { onContextLoss(){} dispose(){} } }))
vi.mock('@xterm/addon-canvas', () => ({ CanvasAddon: class {} }))
vi.mock('@xterm/addon-serialize', () => ({ SerializeAddon: class { serialize = h.serialize; dispose(){} } }))
import { useTerminal } from './useTerminal'
import { terminalSnapshots } from '../signals/terminal-snapshots'
import { terminalRegistry, resetConsoleForReconnect } from '../signals/terminal-registry'
beforeEach(()=> {vi.useFakeTimers(); h.ref=0; h.serialize.mockClear(); h.writes.mockClear(); h.terminals=[]; vi.stubGlobal('document',{documentElement:{}});vi.stubGlobal('getComputedStyle',()=>({getPropertyValue:()=>''}));vi.stubGlobal('requestAnimationFrame',()=>0)})
afterEach(()=>{for(const end of h.effects.splice(0)) end();for(const id of terminalRegistry.keys()) terminalSnapshots.close(id);terminalRegistry.clear();vi.clearAllTimers();vi.useRealTimers();vi.unstubAllGlobals()})
describe('INC-2026-0010 terminal snapshot integration',()=>{
 it('does not serialize unchanged terminals or send their multi-tab map',()=>{
  for(let i=0;i<10;i++) useTerminal(`tab-${i}`)
  vi.advanceTimersByTime(30_000)
  expect(h.serialize).not.toHaveBeenCalled()
  expect(h.writes).not.toHaveBeenCalled()
 })
 it('checkpoints only dirty terminals and stops after a clean tick',()=>{
  useTerminal('dirty');useTerminal('idle')
  h.terminals[0].write('output')
  vi.advanceTimersByTime(10_000)
  expect(h.serialize).toHaveBeenCalledTimes(1)
  expect(terminalSnapshots.get('dirty')).toBe('screen')
  vi.advanceTimersByTime(20_000)
  expect(h.serialize).toHaveBeenCalledTimes(1)
  expect(h.writes).not.toHaveBeenCalled()
 })
 it('does not resurrect a closed or reconnect-reset snapshot during cleanup',()=>{
  useTerminal('closed'); h.terminals[0].write('output')
  terminalSnapshots.close('closed')
  for(const end of h.effects.splice(0)) end()
  expect(h.serialize).not.toHaveBeenCalled()
  expect(terminalSnapshots.get('closed')).toBeUndefined()
  useTerminal('reset'); h.terminals[1].write('output')
  resetConsoleForReconnect('reset')
  h.terminals[1].write('late old output')
  for(const end of h.effects.splice(0)) end()
  expect(terminalSnapshots.get('reset')).toBeUndefined()
  expect(h.serialize).not.toHaveBeenCalled()
 })
})
