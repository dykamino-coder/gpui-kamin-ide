import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'
import { runGit } from '../lib/git-async'
import { refreshAllMarketplaces } from './refresh'
const fixture=vi.hoisted(() => ({home:''}))
vi.mock('os', async original => ({...(await original<typeof import('node:os')>()),default:{...(await original<typeof import('node:os')>()),homedir:()=>fixture.home}}))
vi.mock('../hooks/emit-bridge-event', () => ({emitBridgeHookEvent:vi.fn()}))
let root=''
async function git(args:string[],cwd:string) {return runGit(['-c',`core.hooksPath=${path.join(root,'empty-hooks')}`, ...args],{cwd,timeoutMs:10000})}
async function origin(name:string,payload:Record<string,string>):Promise<string> {
 const dir=path.join(root,name);fs.mkdirSync(dir,{recursive:true});await git(['-c','init.templateDir=','init','--initial-branch=main'],dir)
 for(const [name,bytes] of Object.entries(payload)){const file=path.join(dir,name);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,bytes)}
 await git(['add','.'],dir);await git(['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-m','Synthetic fixture'],dir);return dir
}
afterEach(() => {vi.restoreAllMocks();if(root){const resolved=path.resolve(root);if(path.dirname(resolved)!==path.resolve(os.tmpdir()) || !path.basename(resolved).startsWith('inc-0044-git-'))throw new Error('Unexpected fixture cleanup path');fs.rmSync(resolved,{recursive:true,force:true})}})
it('real local Git: a failed cache copy converges on the next unchanged startup without a recopy loop', async () => {
 root=fs.mkdtempSync(path.join(os.tmpdir(),'inc-0044-git-'));fixture.home=root;fs.mkdirSync(path.join(root,'empty-hooks'))
 const remoteMarket=await origin('market-origin',{'README':'Synthetic marketplace','.gitignore':'plugins/\n'})
 const remotePlugin=await origin('plugin-origin',{'.claude-plugin/plugin.json':JSON.stringify({name:'synthetic',version:'2.0.0'}),'payload.txt':'new committed bytes'})
 const market=path.join(root,'market');await git(['clone','--template=',remoteMarket,market],root);await git(['config','core.hooksPath',path.join(root,'empty-hooks')],market)
 fs.mkdirSync(path.join(market,'plugins'));const source=path.join(market,'plugins','synthetic');await git(['clone','--template=',remotePlugin,source],root);await git(['config','core.hooksPath',path.join(root,'empty-hooks')],source)
 const plugins=path.join(root,'.claude','plugins');fs.mkdirSync(plugins,{recursive:true});const installed=path.join(plugins,'installed_plugins.json')
 const old=path.join(plugins,'cache','fixture','synthetic','1.0.0');fs.mkdirSync(old,{recursive:true});fs.writeFileSync(path.join(old,'payload.txt'),'old installed bytes')
 fs.writeFileSync(installed,JSON.stringify({version:2,plugins:{'synthetic@fixture':[{version:'1.0.0',installPath:old,scope:'user'}]}}))
 fs.writeFileSync(path.join(plugins,'known_marketplaces.json'),JSON.stringify({fixture:{source:{source:'git'},installLocation:market}}))
 const copy=vi.spyOn(fs,'copyFileSync').mockImplementationOnce(() => {throw new Error('Synthetic interrupted cache copy')})
 expect((await refreshAllMarketplaces()).ok).toBe(false);copy.mockRestore()
 const repaired=await refreshAllMarketplaces();expect(repaired.ok).toBe(true);expect(repaired.results[0]?.changed).toBe(true)
 const entry=JSON.parse(fs.readFileSync(installed,'utf8')).plugins['synthetic@fixture'][0];expect(entry.version).toBe('2.0.0');expect(fs.readFileSync(path.join(entry.installPath,'payload.txt'),'utf8')).toBe('new committed bytes')
 expect(entry.sourceRevision).toBe((await git(['rev-parse','HEAD'],source)).stdout.trim())
 const repeatCopy=vi.spyOn(fs,'copyFileSync');const unchanged=await refreshAllMarketplaces();expect(unchanged.ok).toBe(true);expect(unchanged.results[0]?.changed).toBe(false);expect(repeatCopy).not.toHaveBeenCalled()
},20000)
