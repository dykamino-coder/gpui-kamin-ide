// The wrong-recipient window: `activeTabId` flips on click, the transcript
// follows only when the replay lands. Anything sent in between goes to a session
// the user is not looking at.
import { describe, it, expect, beforeEach } from "vitest"
import { activeTabId } from "./tabs"
import { boundTabId, sessionBindingPending } from "./session-binding"

describe("sessionBindingPending", () => {
  beforeEach(() => {
    activeTabId.value = null
    boundTabId.value = null
  })

  it("is pending the moment a session is selected, before its transcript is up", () => {
    boundTabId.value = "small"
    activeTabId.value = "small"
    expect(sessionBindingPending.value).toBe(false)

    activeTabId.value = "huge" // clicked; the viewer still shows "small"
    expect(sessionBindingPending.value).toBe(true)
  })

  it("clears only once the viewer reports the SAME session it is showing", () => {
    activeTabId.value = "huge"
    boundTabId.value = "small" // stale paint from the previous session
    expect(sessionBindingPending.value).toBe(true)

    boundTabId.value = "huge" // replay landed, view caught up
    expect(sessionBindingPending.value).toBe(false)
  })

  it("stays pending across a switch that lands on a third session", () => {
    activeTabId.value = "a"
    boundTabId.value = "a"
    activeTabId.value = "b"
    activeTabId.value = "c" // user keeps clicking while b was still loading
    expect(sessionBindingPending.value).toBe(true)
    boundTabId.value = "b" // b's replay finally lands — but c is active now
    expect(sessionBindingPending.value).toBe(true)
    boundTabId.value = "c"
    expect(sessionBindingPending.value).toBe(false)
  })

  it("is not pending when there is no session at all", () => {
    expect(sessionBindingPending.value).toBe(false)
  })
})
