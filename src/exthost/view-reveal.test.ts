import { expect, it, vi } from "vitest"
import { Registry } from "./registry.js"
import { registerViewFocusCommand, setRevealBroadcast } from "./view-reveal.js"

it("requests keyboard focus from the shell only on explicit opt-in", async () => {
  const registry = new Registry()
  const broadcast = vi.fn()
  setRevealBroadcast(broadcast)
  const focus = registerViewFocusCommand(registry, "sample.console", "sample")
  try {
    await registry.executeCommand("sample.console.focus")
    await registry.executeCommand("sample.console.focus", { keyboardFocus: true })
    expect(broadcast.mock.calls).toEqual([
      ["kamin:view:reveal", { container: "sample", view: "sample.console" }],
      ["kamin:view:reveal", { container: "sample", view: "sample.console", keyboardFocus: true }],
    ])
  } finally {
    focus.dispose()
    setRevealBroadcast(vi.fn())
  }
})
