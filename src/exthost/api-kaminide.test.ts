// `kaminide.window.showTransientMessage` (BR-23): the explicit self-closing
// contract must reach the shell as `transientMs`, while the plain vscode path
// keeps calling the hook without options (buttons wait for a decision).
import { describe, expect, it, vi } from "vitest"
import type { SessionsApi } from "./api/sessions.js"
import { createKaminideApi, TRANSIENT_DEFAULT_MS, type ShowMessageHook } from "./api-kaminide.js"

const sessions = {} as SessionsApi

function api() {
  const show = vi.fn<ShowMessageHook>().mockResolvedValue("Open")
  return { show, k: createKaminideApi(sessions, show) }
}

describe("kaminide.window.showTransientMessage", () => {
  it("forwards the requested countdown with the buttons", async () => {
    const { show, k } = api()
    await expect(k.window.showTransientMessage("Session finished", { durationMs: 6000 }, "Open")).resolves.toBe("Open")
    expect(show).toHaveBeenCalledExactlyOnceWith("info", "Session finished", ["Open"], { transientMs: 6000 })
  })

  it("uses the default countdown and severity when none is valid", async () => {
    const { show, k } = api()
    await k.window.showTransientMessage("x", { severity: "warning", durationMs: Number.NaN })
    expect(show).toHaveBeenCalledExactlyOnceWith("warning", "x", [], { transientMs: TRANSIENT_DEFAULT_MS })
  })

  it("advertises the additive API version", () => {
    expect(api().k.version).toBe("2.1.0")
  })
})
