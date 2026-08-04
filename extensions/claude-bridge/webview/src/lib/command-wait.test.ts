import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

// The waiters read live signals through `snapshot()`; drive that directly.
const state = {
  activeTabId: 't1' as string | null, boundTabId: 't1' as string | null,
  mcpLoading: false, promptReady: true, working: false,
  replayPct: null as number | null, entryCount: 10, segmentCount: 1,
  activeSegment: 0, archived: false, settling: false, busy: false, queued: 0,
}
// Replace the module WITHOUT loading the real one: it reads live signals, whose
// tree pulls `@bridge/storage` — unaliased in the repo-wide vitest config, so
// importing it here would fail to resolve there.
vi.mock('./command-state', () => ({ snapshot: () => ({ ...state }) }))

const { waitFor, waitForIdle, waitForChange } = await import('./command-wait')

function reset(): void {
  Object.assign(state, {
    activeTabId: 't1', boundTabId: 't1', mcpLoading: false, promptReady: true,
    working: false, replayPct: null, entryCount: 10, segmentCount: 1,
    activeSegment: 0, archived: false, settling: false, busy: false, queued: 0,
  })
}

describe('command-wait', () => {
  beforeEach(reset)
  afterEach(() => vi.useRealTimers())

  it('resolves once the predicate holds', async () => {
    state.entryCount = 42
    const s = await waitFor((st) => st.entryCount === 42, 1000)
    expect(s.entryCount).toBe(42)
  })

  it('rejects with the observed state, so a timeout is diagnosable', async () => {
    await expect(waitFor((s) => s.entryCount === 999, 200)).rejects.toThrow(/timed out.*entryCount/s)
  })

  it('waitForChange resolves only once the field actually moves', async () => {
    const p = waitForChange('entryCount', 2000)
    setTimeout(() => { state.entryCount = 11 }, 150)
    await expect(p).resolves.toMatchObject({ entryCount: 11 })
  })

  // The bug this whole module exists for: a compaction ends its turn (busy
  // false) and only THEN rebuilds the store. Stopping at the turn boundary reads
  // a half-built view.
  it('does not settle while the view is still rebuilding', async () => {
    let resolved = false
    const p = waitForIdle({ stableMs: 300, timeoutMs: 5000 }).then((s) => { resolved = true; return s })
    // Churn the entry count past the stable window a few times.
    for (let i = 0; i < 4; i++) {
      await new Promise((r) => setTimeout(r, 150))
      state.entryCount++
    }
    expect(resolved).toBe(false)
    const s = await p
    expect(s.entryCount).toBe(14)
  })

  it('does not settle while busy even if nothing is changing', async () => {
    state.busy = true
    await expect(waitForIdle({ stableMs: 100, timeoutMs: 400 })).rejects.toThrow(/timed out/)
  })
})
