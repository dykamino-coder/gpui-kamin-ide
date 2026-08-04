// DPAPI relay marker handling: seal adds `dpapi:`, unseal strips it, legacy
// plaintext passes through, and relay failures degrade gracefully.
import { describe, expect, it, vi } from "vitest"
import { sealSecret, unsealSecret } from "./secret-crypto.js"

describe("secret-crypto", () => {
  it("seals via the relay and marks the ciphertext", async () => {
    const relay = vi.fn().mockResolvedValue("deadbeef")
    expect(await sealSecret(relay, "token")).toBe("dpapi:deadbeef")
    expect(relay).toHaveBeenCalledWith("secret.encrypt", "token")
  })

  it("unseals a marked value via the relay (passing only the hex)", async () => {
    const relay = vi.fn().mockResolvedValue("token")
    expect(await unsealSecret(relay, "dpapi:deadbeef")).toBe("token")
    expect(relay).toHaveBeenCalledWith("secret.decrypt", "deadbeef")
  })

  it("returns unmarked (legacy plaintext) values as-is without calling the relay", async () => {
    const relay = vi.fn()
    expect(await unsealSecret(relay, "plain")).toBe("plain")
    expect(relay).not.toHaveBeenCalled()
  })

  it("falls back to plaintext when sealing fails (no DPAPI)", async () => {
    const relay = vi.fn().mockRejectedValue(new Error("no shell"))
    expect(await sealSecret(relay, "token")).toBe("token")
  })

  it("yields undefined when a sealed value can't be unsealed", async () => {
    const relay = vi.fn().mockRejectedValue(new Error("wrong user"))
    expect(await unsealSecret(relay, "dpapi:deadbeef")).toBeUndefined()
  })
})
