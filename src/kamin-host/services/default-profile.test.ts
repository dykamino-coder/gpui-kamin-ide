import { describe, expect, it } from "vitest"
import { isDefaultDataDir } from "./default-profile.js"

describe("default profile migration eligibility", () => {
  it("accepts the owner's default root, with Windows path casing", () => {
    expect(isDefaultDataDir("c:\\users\\owner\\appdata\\local\\kaminide-gpui-dev\\data\\", "win32", "C:\\Users\\Owner")).toBe(true)
  })
  it("rejects a redirected LOCALAPPDATA and an explicit user-data-dir", () => {
    expect(isDefaultDataDir("C:\\profiles\\test\\kaminide-gpui-dev\\data", "win32", "C:\\Users\\Owner")).toBe(false)
    expect(isDefaultDataDir("C:\\profiles\\test", "win32", "C:\\Users\\Owner")).toBe(false)
  })
  it("uses the shell's temp fallback outside Windows", () => {
    expect(isDefaultDataDir("/tmp/kaminide-gpui-dev/data", "linux", "/home/owner", "/tmp")).toBe(true)
    expect(isDefaultDataDir("/tmp/test/kaminide-gpui-dev/data", "linux", "/home/owner", "/tmp")).toBe(false)
  })
})
