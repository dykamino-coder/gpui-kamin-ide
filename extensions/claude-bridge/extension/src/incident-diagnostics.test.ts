import { describe, expect, it } from "vitest"

import { formatIncidentLine, normalizeConnectionTransition, normalizeRendererSample } from "./incident-diagnostics"

describe("incident diagnostics", () => {
  it("records only allowlisted connection fields", () => {
    const secretTab = "tab-token-prompt-secret"
    const record = normalizeConnectionTransition(secretTab, {
      status: "error",
      error: "401 token=secret prompt=private",
      retryAttempt: 4.9,
      payload: "must-not-survive",
    })
    const line = formatIncidentLine(record)

    expect(record).toMatchObject({ status: "error", cause: "auth", retryAttempt: 4 })
    expect(record.tabRef).toMatch(/^[a-f0-9]{12}$/)
    expect(line).not.toContain(secretTab)
    expect(line).not.toContain("token=secret")
    expect(line).not.toContain("prompt=private")
    expect(line).not.toContain("must-not-survive")
  })

  it("bounds renderer counters and rejects arbitrary fields", () => {
    const record = normalizeRendererSample({
      role: "chat",
      heapMB: Number.POSITIVE_INFINITY,
      retainedTabs: -3,
      retainedEntries: 9e20,
      activeEntries: 12.8,
      storeWindow: 4_000,
      scrollUpMax: 16_000,
      windowState: "over-configured-window",
      transcript: "private chat content",
    })
    const text = JSON.stringify(record)

    expect(record).toMatchObject({
      role: "chat",
      heapMB: null,
      retainedTabs: 0,
      retainedEntries: 1_000_000_000,
      activeEntries: 12,
      windowState: "over-configured-window",
    })
    expect(text).not.toContain("transcript")
    expect(text).not.toContain("private chat content")
  })

  it("maps malformed values to safe defaults", () => {
    expect(normalizeConnectionTransition(null, null)).toMatchObject({
      status: "error",
      cause: "unknown",
      retryAttempt: 0,
    })
    expect(normalizeRendererSample(null)).toMatchObject({
      role: "unknown",
      windowState: "unknown",
      heapMB: null,
    })
  })

  it("classifies abnormal WebSocket closes without persisting the reason", () => {
    const record = normalizeConnectionTransition("tab", {
      status: "disconnected",
      closeCode: 1006,
      error: "private close reason",
    })

    expect(record.cause).toBe("network")
    expect(JSON.stringify(record)).not.toContain("private close reason")
  })
})
