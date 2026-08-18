import fs from "node:fs"
import { describe, expect, it } from "vitest"
import { VERSION } from "./index"

describe("server version", () => {
  it("uses the package manifest version reported by the release", () => {
    const manifest = JSON.parse(
      fs.readFileSync(
        new URL("../../../package.json", import.meta.url),
        "utf-8",
      ),
    ) as { version: string }

    expect(VERSION).toBe(manifest.version)
  })
})
