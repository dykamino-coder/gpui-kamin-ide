import { describe, expect, it, vi } from "vitest"
import { Webviews } from "./webview.js"

describe("INC-2026-0110 resolve acknowledgement", () => {
  it("rejects before registration, allowing the shell to retry later", () => {
    const views = new Webviews(vi.fn())
    expect(() => { views.resolveView("late") }).toThrow("Webview view provider not registered")
    const resolve = vi.fn()
    views.registerWebviewViewProvider("late", { resolveWebviewView: resolve })
    views.resolveView("late")
    views.resolveView("late")
    expect(resolve).toHaveBeenCalledTimes(1)
  })
})
