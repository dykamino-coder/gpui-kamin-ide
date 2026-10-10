import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TerminalSnapshotCache, TERMINAL_SNAPSHOT_SLOTS, TERMINAL_SNAPSHOT_BYTES } from './terminal-snapshots'
beforeEach(()=>vi.useFakeTimers());afterEach(()=>{vi.clearAllTimers();vi.useRealTimers()})
describe('Console document-local snapshot contract',()=>{
 it('batches dirty writers on one timer with no state IPC or idle serializers',()=>{
  const cache=new TerminalSnapshotCache();const serialize=vi.fn(()=> 'screen')
  const writers=Array.from({length:30},(_,i)=>cache.attach(String(i),serialize))
  expect(vi.getTimerCount()).toBe(1)
  writers[1].markDirty();writers[5].markDirty();vi.advanceTimersByTime(10_000)
  expect(serialize).toHaveBeenCalledTimes(2)
  vi.advanceTimersByTime(10_000);expect(serialize).toHaveBeenCalledTimes(2)
  for(const writer of writers) writer.detach();expect(vi.getTimerCount()).toBe(0)
 })
 it('restores remounts in this document, bounds keys and starts fresh on recreation',()=>{
  const cache=new TerminalSnapshotCache()
  for(let i=0;i<40;i++){const writer=cache.attach(String(i),()=> 'screen');writer.markDirty();writer.detach()}
  expect(cache.get('0')).toBeUndefined();expect(cache.get(String(40-TERMINAL_SNAPSHOT_SLOTS))).toBe('screen')
  expect(cache.get('39')).toBe('screen');expect(new TerminalSnapshotCache().get('39')).toBeUndefined()
  cache.close('39');expect(cache.get('39')).toBeUndefined()
 })
 it('measures Unicode bytes and retains a complete screen instead of cutting ANSI data',()=>{
  const cache=new TerminalSnapshotCache();const serialize=vi.fn((scrollback:number)=> scrollback ? '🙂'.repeat(TERMINAL_SNAPSHOT_BYTES/2) : '\x1b[31mcurrent screen🙂\x1b[0m')
  const writer=cache.attach('unicode',serialize);writer.markDirty();writer.detach()
  expect(serialize.mock.calls.map(([value])=>value)).toEqual([1000,0])
  expect(cache.get('unicode')).toBe('\x1b[31mcurrent screen🙂\x1b[0m')
  const huge=cache.attach('huge',()=> '🙂'.repeat(TERMINAL_SNAPSHOT_BYTES));huge.markDirty();vi.advanceTimersByTime(20_000)
  expect(cache.get('huge')).toBeUndefined();huge.detach()
 })
 it('ignores late callbacks from a replaced or closed mount',()=>{
  const cache=new TerminalSnapshotCache();const old=cache.attach('tab',()=> 'old');const newer=cache.attach('tab',()=> 'new')
  old.markDirty();old.detach();newer.markDirty();vi.advanceTimersByTime(10_000);expect(cache.get('tab')).toBe('new')
  cache.close('tab');newer.markDirty();newer.detach();expect(cache.get('tab')).toBeUndefined()
 })
})
