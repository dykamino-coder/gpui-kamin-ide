// Difftest for the three resync-on-reveal bugs found reviewing 0.2.56.
// Each case fails against the old design (one global `resyncPending` flag, set
// only on a `working=true` transition, never cleared on view dispose).
import { describe, it, expect } from "vitest"
import { ResyncTracker } from "./resync-tracker"

type View = { id: string }
const chat: View = { id: "chat" }
const tools: View = { id: "tools" }
const all = [chat, tools]

/** Mirrors BridgeHost.post(): withhold from hidden views, record the gap. */
function fanout(t: ResyncTracker<View>, views: View[] = all, resyncable = true): View[] {
  const delivered: View[] = []
  for (const v of views) {
    if (t.isHidden(v)) {
      if (resyncable) t.markStale(v)
      continue
    }
    delivered.push(v)
  }
  return delivered
}

describe("ResyncTracker", () => {
  it("resyncs a view hidden MID-turn (old code keyed off the working=true edge and missed it)", () => {
    const t = new ResyncTracker<View>()
    t.setVisible(chat, true) // visible when the turn starts → old code set no flag
    t.setVisible(tools, true)

    t.setVisible(chat, false) // user collapses chat while the CLI is generating
    expect(fanout(t)).toEqual([tools]) // chat's entries are dropped

    expect(t.setVisible(chat, true)).toBe(true) // reveal → must resync
  })

  it("does not let the first revealed panel consume the other's catch-up", () => {
    const t = new ResyncTracker<View>()
    t.setVisible(chat, true)
    t.setVisible(tools, true)
    t.setVisible(chat, false)
    t.setVisible(tools, false) // both collapsed
    fanout(t) // session switch behind both → both have a gap

    expect(t.setVisible(tools, true)).toBe(true) // open tools first
    expect(t.setVisible(chat, true)).toBe(true) // chat still gets its own resync
  })

  it("a view still hidden during another's resync keeps its own flag", () => {
    const t = new ResyncTracker<View>()
    t.setVisible(chat, false)
    t.setVisible(tools, false)
    fanout(t, all)

    expect(t.setVisible(chat, true)).toBe(true) // chat revealed → resync broadcasts
    expect(t.isStale(tools)).toBe(true) // tools was hidden, so it missed that broadcast too
    expect(t.setVisible(tools, true)).toBe(true) // and resyncs on its own reveal
  })

  it("forgets a view disposed while hidden (old map latched it as hidden forever)", () => {
    const t = new ResyncTracker<View>()
    t.setVisible(tools, false)
    fanout(t)
    expect(t.isStale(tools)).toBe(true)

    t.forget(tools) // user closes the panel for good
    expect(t.isHidden(tools)).toBe(false) // untracked, not "hidden"
    expect(t.isStale(tools)).toBe(false)
  })

  it("ignores non-resyncable drops — a lost toast must not latch a full replay", () => {
    const t = new ResyncTracker<View>()
    t.setVisible(chat, false)
    fanout(t, all, false) // e.g. toast:route — a resync cannot re-deliver it
    expect(t.isStale(chat)).toBe(false)
    expect(t.setVisible(chat, true)).toBe(false)
  })

  it("a plain open/close on an idle session costs nothing", () => {
    const t = new ResyncTracker<View>()
    t.setVisible(chat, true)
    t.setVisible(chat, false)
    expect(t.setVisible(chat, true)).toBe(false) // nothing happened while hidden
  })
})
