import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PluginCard } from './PluginCard'
import { PluginsTabs } from './PluginsTabs'
import { PluginsPanel } from './PluginsPanel'
const fixture = vi.hoisted(() => ({states:[] as any[], refs:[] as any[], effects:[] as any[], cursor:0, refCursor:0, effectCursor:0, onUpdated:null as any, loads:[] as any[], write:vi.fn(), off:vi.fn()}))
vi.mock('preact/hooks', () => ({
 useState:(initial:any) => {const i=fixture.cursor++; if(!(i in fixture.states)) fixture.states[i]=initial;return [fixture.states[i], (v:any) => {fixture.states[i]=typeof v==='function'?v(fixture.states[i]):v}]},
 useRef:(initial:any) => {const i=fixture.refCursor++;return fixture.refs[i] ?? (fixture.refs[i]={current:initial})},
 useEffect:(fn:any,deps:any[]) => {const i=fixture.effectCursor++;const prior=fixture.effects[i];if(prior && deps.length===prior.deps.length && deps.every((x,j)=>x===prior.deps[j])) return;prior?.off?.();fixture.effects[i]={deps,off:fn()}},
}))
vi.mock('../../../hooks/useBridge', () => ({useBridge:() => bridge}))
vi.mock('./PluginCard', () => ({PluginCard:() => null}))
vi.mock('./PluginsTabs', () => ({PluginsTabs:() => null}))
vi.mock('./MarketplaceRow', () => ({MarketplaceRow:() => null}))
vi.mock('./MarketplaceCloneForm', () => ({MarketplaceCloneForm:() => null}))
vi.mock('./PluginsGrid', () => ({PluginsGrid:() => null}))
vi.mock('./plugins-cache', () => ({readPluginsCache:() => [{name:'synthetic',marketplace:'fixture',version:'1.0.0'}],writePluginsCache:fixture.write}))
const bridge = {
 onPluginsBrowseProgress: () => () => {},
 onMarketplaceUpdated: (cb:any) => {fixture.onUpdated=cb;return fixture.off},
 listInstalledPlugins: vi.fn(() => new Promise<any[]>(resolve => fixture.loads.push(resolve))),
 listMarketplaces: vi.fn(async () => []),
}
function render() {fixture.cursor=fixture.refCursor=fixture.effectCursor=0; return PluginsPanel()}
function nodes(v:any):any[] {if(!v || typeof v!=='object')return [];if(Array.isArray(v))return v.flatMap(nodes);return [v,...nodes(v.props?.children)]}
const card = () => nodes(render()).find(v=>v.type===PluginCard)
const next = async () => {await Promise.resolve();await Promise.resolve();await Promise.resolve()}
beforeEach(() => {fixture.states=[];fixture.refs=[];fixture.effects=[];fixture.loads=[];fixture.onUpdated=null;fixture.write.mockClear();fixture.off.mockClear();bridge.listInstalledPlugins.mockClear()})
afterEach(() => {for(const effect of fixture.effects)effect?.off?.()})
describe('Active plugin listing convergence (INC-2026-0044)', () => {
 it('revalidates Active when a startup sweep completes while the marketplace chips are hidden', async () => {
  render();fixture.loads[0]([{name:'synthetic',marketplace:'fixture',version:'1.0.0'}]);await next()
  expect(fixture.onUpdated).toBeTypeOf('function');fixture.onUpdated({name:'fixture',ok:true})
  expect(bridge.listInstalledPlugins).toHaveBeenCalledTimes(2)
  fixture.loads[1]([{name:'synthetic',marketplace:'fixture',version:'2.0.0'}]);await next();expect(card().props.version).toBe('2.0.0')
 })
 it('rejects an older initial listing after a newer card refresh resolves', async () => {
  render();card().props.onRefresh();expect(fixture.loads).toHaveLength(2)
  fixture.loads[1]([{name:'synthetic',marketplace:'fixture',version:'2.0.0'}]);await next()
  fixture.loads[0]([{name:'synthetic',marketplace:'fixture',version:'1.0.0'}]);await next();expect(card().props.version).toBe('2.0.0')
 })
 it('does not publish or persist a listing response after panel disposal', async () => {
  render();for(const effect of fixture.effects)effect?.off?.()
  fixture.loads[0]([{name:'synthetic',marketplace:'fixture',version:'2.0.0'}]);await next()
  expect(fixture.write).not.toHaveBeenCalled()
 })
 it('does not apply an Active listing after changing to another tab', async () => {
  render();nodes(render()).find(v=>v.type===PluginsTabs).props.onTabChange('personal');render();await next()
  fixture.loads[0]([{name:'synthetic',marketplace:'fixture',version:'2.0.0'}]);await next();expect(fixture.write).not.toHaveBeenCalled()
 })
 it('coalesces a marketplace completion burst into one running read and one trailing read', async () => {
  render();for(let i=0;i<100;i++)fixture.onUpdated({name:`fixture-${i}`,ok:true})
  expect(fixture.loads).toHaveLength(2)
  fixture.loads[1]([{name:'synthetic',marketplace:'fixture',version:'2.0.0'}]);await next();expect(fixture.loads).toHaveLength(3)
  fixture.loads[2]([{name:'synthetic',marketplace:'fixture',version:'3.0.0'}]);await next();expect(card().props.version).toBe('3.0.0')
 })

})
