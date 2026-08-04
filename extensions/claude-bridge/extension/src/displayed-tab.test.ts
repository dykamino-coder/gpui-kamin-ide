import { describe, it, expect, beforeEach } from 'vitest'
import { reportDisplayedTab, getDisplayedTab, onDisplayedTabChange, forgetDisplayedTab, clearDisplayedTab } from './displayed-tab'

describe('displayed tab', () => {
  beforeEach(() => { clearDisplayedTab() })

  it('knows nothing before the chat has reported', () => {
    expect(getDisplayedTab()).toBeNull()
  })

  it('records what the chat says it painted', () => {
    reportDisplayedTab('tab-a')
    expect(getDisplayedTab()).toBe('tab-a')
  })

  it('stays quiet on a repeat report', () => {
    // The chat re-reports whenever its bind effect re-runs; waking every host
    // surface for an unchanged value would be a broadcast per render.
    const seen: string[] = []
    const off = onDisplayedTabChange((id) => seen.push(id))
    reportDisplayedTab('tab-a')
    reportDisplayedTab('tab-a')
    reportDisplayedTab('tab-b')
    off()
    expect(seen).toEqual(['tab-a', 'tab-b'])
  })

  it('stops notifying after unsubscribe', () => {
    const seen: string[] = []
    const off = onDisplayedTabChange((id) => seen.push(id))
    off()
    reportDisplayedTab('tab-c')
    expect(seen).toEqual([])
  })

  it('forgets a closed tab so the gap check cannot compare against a dead one', () => {
    reportDisplayedTab('tab-a')
    forgetDisplayedTab('tab-a')
    expect(getDisplayedTab()).toBeNull()
  })

  it('leaves the record alone when a DIFFERENT tab closes', () => {
    reportDisplayedTab('tab-a')
    forgetDisplayedTab('tab-b')
    expect(getDisplayedTab()).toBe('tab-a')
  })

  it('reports again after the tab it forgot comes back', () => {
    reportDisplayedTab('tab-a')
    forgetDisplayedTab('tab-a')
    const seen: string[] = []
    const off = onDisplayedTabChange((id) => seen.push(id))
    reportDisplayedTab('tab-a') // not deduped — the record was cleared
    off()
    expect(seen).toEqual(['tab-a'])
  })

  it('forgetting the shown tab notifies listeners so the gap re-publishes', () => {
    // The gap check runs on notification; if forget stayed silent, a closed tab
    // would leave bridgeShowing:true until the next unrelated event.
    reportDisplayedTab('tab-a')
    const seen: string[] = []
    const off = onDisplayedTabChange((id) => seen.push(id))
    forgetDisplayedTab('tab-a')
    off()
    expect(seen).toEqual([''])
    expect(getDisplayedTab()).toBeNull()
  })

  it('clearDisplayedTab notifies once when there was something, and is a no-op when empty', () => {
    // The chat webview being torn down: nothing shows any transcript now.
    reportDisplayedTab('tab-a')
    const seen: string[] = []
    const off = onDisplayedTabChange((id) => seen.push(id))
    clearDisplayedTab()
    clearDisplayedTab() // already null — must not fire again
    off()
    expect(seen).toEqual([''])
    expect(getDisplayedTab()).toBeNull()
  })
})
