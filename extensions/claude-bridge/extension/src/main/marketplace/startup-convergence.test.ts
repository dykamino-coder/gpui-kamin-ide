import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { refreshAllMarketplaces } from './refresh'
const fixture = vi.hoisted(() => ({ home: '', failingPull: false }))
vi.mock('os', async original => ({ ...(await original<typeof import('node:os')>()), default: { ...(await original<typeof import('node:os')>()), homedir: () => fixture.home } }))
vi.mock('../lib/git-async', () => ({runGit: async (args: string[]) => {
 if (fixture.failingPull) throw new Error('Synthetic offline failure')
 return {stdout: args[0] === 'rev-parse' ? 'a'.repeat(40) + '\n' : 'Already up to date.', stderr:'', code:0}
}}))
vi.mock('../hooks/emit-bridge-event', () => ({emitBridgeHookEvent: vi.fn()}))
let root = ''
let plugins = ''
let source = ''
let installed = ''
const readInstalled = () => JSON.parse(fs.readFileSync(installed, 'utf8'))
function setup(options: { autoUpdate?: boolean; git?: boolean; directory?: boolean; installed?: boolean } = {}) {
 const market = path.join(root, 'market'); source = path.join(market, 'plugins', 'synthetic')
 fs.mkdirSync(path.join(source, '.claude-plugin'), {recursive:true})
 if (options.git !== false) fs.mkdirSync(path.join(source, '.git'))
 fs.writeFileSync(path.join(source, '.claude-plugin', 'plugin.json'), JSON.stringify({name:'synthetic',version:'2.0.0'}))
 fs.writeFileSync(path.join(source, 'payload.txt'), 'new source bytes')
 fs.mkdirSync(plugins, {recursive:true})
 fs.writeFileSync(path.join(plugins,'known_marketplaces.json'), JSON.stringify({fixture:{installLocation:market, autoUpdate:options.autoUpdate, source:{source:options.directory ? 'directory':'git'}}}))
 const oldCache = path.join(plugins, 'cache', 'fixture', 'synthetic', '1.0.0')
 fs.mkdirSync(oldCache,{recursive:true}); fs.writeFileSync(path.join(oldCache,'payload.txt'),'old installed bytes')
 fs.writeFileSync(installed,JSON.stringify({version:2,plugins:options.installed === false ? {} : {'synthetic@fixture':[{version:'1.0.0',installPath:oldCache,scope:'user',installedAt:'2026-01-01'}]}}))
}
beforeEach(() => {root=fs.mkdtempSync(path.join(os.tmpdir(),'inc-0044-')); fixture.home=root; fixture.failingPull=false; plugins=path.join(root,'.claude','plugins');installed=path.join(plugins,'installed_plugins.json')})
afterEach(() => {vi.restoreAllMocks(); const resolved=path.resolve(root); if(path.dirname(resolved)!==path.resolve(os.tmpdir()) || !path.basename(resolved).startsWith('inc-0044-')) throw new Error('Unexpected fixture cleanup path'); fs.rmSync(resolved,{recursive:true,force:true})})
describe('startup source/cache convergence (INC-2026-0044)', () => {
 it('repairs an older installed cache after an already advanced source has an unchanged pull', async () => {
  setup(); const sweep=await refreshAllMarketplaces();expect(sweep.ok).toBe(true)
  const entry=readInstalled().plugins['synthetic@fixture'][0]; expect(entry.version).toBe('2.0.0')
  expect(fs.readFileSync(path.join(entry.installPath,'payload.txt'),'utf8')).toBe('new source bytes')
  expect(sweep.results[0]?.changed).toBe(true)
 })
 it('reports cache-copy failure and repairs it on the next unchanged launch', async () => {
  setup(); const copy=vi.spyOn(fs,'copyFileSync').mockImplementationOnce(() => {throw new Error('Synthetic cache copy failure')})
  const failed=await refreshAllMarketplaces(); expect(failed.ok).toBe(false); expect(failed.results[0]?.error).toContain('cache')
  copy.mockRestore(); expect((await refreshAllMarketplaces()).ok).toBe(true)
  const entry=readInstalled().plugins['synthetic@fixture'][0]; expect(entry.version).toBe('2.0.0');expect(fs.readFileSync(path.join(entry.installPath,'payload.txt'),'utf8')).toBe('new source bytes')
 })
 it('does not recopy an installed cache already stamped with the same source revision', async () => {
  setup(); await refreshAllMarketplaces(); const copy=vi.spyOn(fs,'copyFileSync')
  const again=await refreshAllMarketplaces();expect(again.ok).toBe(true);expect(again.results[0]?.changed).toBe(false);expect(copy).not.toHaveBeenCalled()
 })
 it.each([{autoUpdate:false},{git:false},{directory:true},{installed:false}])('preserves exclusions without phantom installs: %j', async options => {
  setup(options); const before=fs.readFileSync(installed,'utf8'); await refreshAllMarketplaces();expect(fs.readFileSync(installed,'utf8')).toBe(before)
 })
 it('does not overwrite installed data on a failed pull', async () => {
  setup(); fixture.failingPull=true;const before=fs.readFileSync(installed,'utf8');expect((await refreshAllMarketplaces()).ok).toBe(false);expect(fs.readFileSync(installed,'utf8')).toBe(before)
 })
 it('preserves a missing sub-clone and continues independent marketplaces after cache failure', async () => {
  setup(); fs.renameSync(source,path.join(root,'absent-source'))
  const before=fs.readFileSync(installed,'utf8');await refreshAllMarketplaces();expect(fs.readFileSync(installed,'utf8')).toBe(before)
  fs.renameSync(path.join(root,'absent-source'),source)
  const known=path.join(plugins,'known_marketplaces.json');const data=JSON.parse(fs.readFileSync(known,'utf8'))
  const second=path.join(root,'second-market');fs.mkdirSync(second);data.second={installLocation:second,source:{source:'git'}};fs.writeFileSync(known,JSON.stringify(data))
  vi.spyOn(fs,'copyFileSync').mockImplementationOnce(() => {throw new Error('Synthetic cache failure')})
  const sweep=await refreshAllMarketplaces();expect(sweep.results.map(x=>[x.name,x.ok])).toEqual([['fixture',false],['second',true]])
 })

})
