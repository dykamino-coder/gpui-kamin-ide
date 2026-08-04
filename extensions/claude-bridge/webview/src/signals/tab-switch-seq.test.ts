// The overtake guard: a `tab:switched` that arrives late — for a session the
// user has already moved past — must not win. Sequence order is assigned by the
// extension in intent order; the webview keeps the highest and rejects lower.
import { describe, it, expect, beforeEach } from 'vitest'
import { activeTabId, applyTabSwitch, switchTabLocal } from './tabs'

describe('tab switch sequencing', () => {
  beforeEach(() => {
    switchTabLocal(null) // clear any local intent left by a prior test
    activeTabId.value = null
    // Reset the module's internal seq by replaying an ever-higher one below;
    // each test starts from a known-applied high-water mark.
    applyTabSwitch('reset', 1_000_000)
  })

  it('applies a switch with a higher seq', () => {
    expect(applyTabSwitch('A', 1_000_001)).toBe(true)
    expect(activeTabId.value).toBe('A')
  })

  it('rejects a switch whose seq the user has already moved past', () => {
    applyTabSwitch('A', 1_000_010)
    // A late frame for B, emitted BEFORE A but delivered after it (the overtake).
    expect(applyTabSwitch('B', 1_000_005)).toBe(false)
    expect(activeTabId.value).toBe('A') // stayed on A
  })

  it('rejects a re-delivery of the same seq', () => {
    applyTabSwitch('A', 1_000_020)
    expect(applyTabSwitch('A', 1_000_020)).toBe(false)
  })

  it('models the reported A→B→A overtake end to end', () => {
    // Click A (seq 30) → click B (31) → click A again (32). B's frame is slow and
    // lands LAST. Without the guard it would leave the view on B.
    applyTabSwitch('A', 1_000_030)
    applyTabSwitch('B', 1_000_031)
    applyTabSwitch('A', 1_000_032) // user is now on A
    expect(applyTabSwitch('B', 1_000_031)).toBe(false) // late B frame — rejected
    expect(activeTabId.value).toBe('A')
  })

  it('still applies a genuinely newer switch after a rejection', () => {
    applyTabSwitch('A', 1_000_040)
    applyTabSwitch('B', 1_000_035) // rejected
    expect(applyTabSwitch('C', 1_000_041)).toBe(true) // newer — applies
    expect(activeTabId.value).toBe('C')
  })

  it('local intent (optimistic click) beats a stale extension frame for another tab', () => {
    // The overtake the seq alone missed: optimistic writes carry NO seq, so a
    // late tab:switched(B) with a high seq used to win. Now the latest local
    // intent (A) rejects it.
    switchTabLocal('B')
    switchTabLocal('A') // user clicked B then A
    expect(applyTabSwitch('B', 1_000_050)).toBe(false) // stale B — rejected
    expect(activeTabId.value).toBe('A')
  })

  it('confirming the intended tab clears the intent so extension switches resume', () => {
    switchTabLocal('A')
    expect(applyTabSwitch('A', 1_000_060)).toBe(true) // confirms + clears intent
    expect(applyTabSwitch('B', 1_000_061)).toBe(true) // extension-initiated switch now applies
    expect(activeTabId.value).toBe('B')
  })
})
