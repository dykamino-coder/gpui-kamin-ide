import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SessionStats } from './SessionStats'
import { appendJsonlEntries, clearJsonlEntries } from '../../signals/jsonl'
import { activeTabId, tabs } from '../../signals/tabs'
import { activeSegmentTsByTab } from '../../signals/compact-segments'
import { setReplayProgress } from '../../signals/replay-progress'
import { computeContextStats } from '../../utils/session-cost'
vi.mock('@preact/signals', async original => ({...(await original<typeof import('@preact/signals')>()),useComputed:(fn:()=>any) => ({get value(){return fn()}})}))
vi.mock('../../hooks/useBridge', () => ({useBridge:()=>({submitText:vi.fn()})}))
const model='claude-opus-5'
const time=(n:number)=>new Date(1700000000000+n*1000).toISOString()
const turn=(n:number,used:number,extra:object={})=>({type:'assistant',uuid:`turn-${n}`,timestamp:time(n),model,message:{id:`message-${n}`,content:[{type:'text',text:'Synthetic response'}],usage:{input_tokens:used,output_tokens:1}},...extra})
const boundary=(n:number,post?:number)=>({type:'system',subtype:'compact_boundary',uuid:`compact-${n}`,timestamp:time(n),compactMetadata:{preTokens:978010,postTokens:post}})
const summary=(n:number)=>turn(n,0,{model:'<synthetic>'})
function text(v:any):string {if(v==null)return '';if(typeof v==='string'||typeof v==='number')return String(v);if(Array.isArray(v))return v.map(text).join(' ');return text(v.props?.children)}
const add=(entries:any[],tab='a')=>appendJsonlEntries(tab,entries)
beforeEach(()=>{clearJsonlEntries('a');clearJsonlEntries('b');activeSegmentTsByTab.value=new Map();activeTabId.value='a';tabs.value=[{id:'a',model},{id:'b',model}] as never})
describe('compact boundary → context meter (INC-2026-0045)',()=>{
 it('shows post-compact occupancy before another model usage arrives, then fresh usage supersedes it',()=>{
  add([turn(1,978010)]);expect(text(SessionStats())).toContain('978.0K')
  add([boundary(2,15098),summary(3)]);let display=text(SessionStats());expect(display).toContain('15.1K');expect(display).not.toContain('978.0K')
  expect(display).toContain('≈');add([turn(4,18000)]);display=text(SessionStats());expect(display).toContain('18.0K');expect(display).not.toContain('≈')
 })
 it('shows explicit pending context when a completed compact has no usable post-token metadata',()=>{
  add([turn(1,978010),boundary(2),summary(3)]);const display=text(SessionStats());expect(display).toContain('Context pending');expect(display).not.toContain('978.0K');expect(display).not.toContain('0%')
 })
 it('preserves archived peak/cost while Current reflects the compact boundary',()=>{
  add([turn(1,978010),boundary(2,15098),summary(3)]);activeSegmentTsByTab.value=new Map([['a',time(2)]])
  expect(text(SessionStats())).toContain('978.0K');activeSegmentTsByTab.value=new Map();expect(text(SessionStats())).toContain('15.1K')
 })
 it('reverse replay/prepended old usage cannot restore pre-compact pressure',()=>{
  setReplayProgress('a',10);add([boundary(2,15098),summary(3)]);expect(text(SessionStats())).toContain('15.1K')
  add([turn(1,978010)]);expect(text(SessionStats())).toContain('15.1K');setReplayProgress('a',null);expect(text(SessionStats())).toContain('15.1K')
 })
 it('is isolated between tabs and repeated compacts, including a non-blocking hook error',()=>{
  add([turn(1,978010),boundary(2,15098),summary(3)]);add([turn(1,250000)],'b');activeTabId.value='b';expect(text(SessionStats())).toContain('250.0K')
  activeTabId.value='a';expect(text(SessionStats())).toContain('15.1K');add([boundary(4,8000),{type:'system',subtype:'hook_error',uuid:'hook',timestamp:time(5)}]);expect(text(SessionStats())).toContain('8.0K')
 })
 it('does not let sidechain boundaries or synthetic usage replace parent usage',()=>{
  expect(computeContextStats([turn(1,100000),{...boundary(2,100),isSidechain:true},summary(3)],true,model)?.used).toBe(100000)
 })
 it.each([NaN,-1,Infinity])('invalid post-token metadata stays unknown: %s',post=>{
  const stats=computeContextStats([turn(1,978010),boundary(2,post)],true,model)
  expect(stats?.used).toBeNull();expect(stats?.pct).toBeNull()
 })
 it('keeps pre-compact pressure on failure/cancellation without a successful boundary',()=>{
  add([turn(1,978010),{type:'system',subtype:'api_error',uuid:'failed',timestamp:time(2)}]);expect(text(SessionStats())).toContain('978.0K')
 })
 it('hydrates a fresh/new-file store from compact metadata without an old usage fallback',()=>{
  add([boundary(2,15098),summary(3)]);expect(text(SessionStats())).toContain('15.1K')
  clearJsonlEntries('a');add([boundary(4),summary(5)]);expect(text(SessionStats())).toContain('Context pending')
 })
 it('preserves archived cost accounting and ignores later subagent usage',()=>{
  const pre=computeContextStats([turn(1,978010)],false,model)!
  const archive=computeContextStats([turn(1,978010),boundary(2,15098)],false,model)!;expect(archive.used).toBe(pre.used);expect(archive.cost).toBe(pre.cost)
  const current=computeContextStats([boundary(2,15098),summary(3),turn(4,900000,{isSidechain:true})],true,model)!
  expect(current.used).toBe(15098);expect(current.cost).toBe(0)
 })

})
