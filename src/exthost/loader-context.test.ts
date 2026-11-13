import { describe, expect, it } from "vitest"
import type { ExtensionFacade } from "./api/types.js"
import type { EnvHost, StorageHost } from "./host-services.js"
import { makeContext } from "./loader-context.js"

describe("profile storage context", () => {
  for (const isDefault of [true, false, undefined]) {
    it(`passes the host storage root and default-profile opt-in (${String(isDefault)})`, () => {
      const storage = {
        globalStorageDir: () => "/profile-a/globalStorage/bridge",
        logDir: () => "/profile-a/logs/bridge",
        storageDir: () => null,
        onSecretChange: () => () => { /* no-op unsubscribe */ },
        ...(isDefault === undefined ? {} : { isDefaultProfile: () => isDefault }),
      } as unknown as StorageHost
      const context = makeContext("/extension", [], {} as ExtensionFacade, storage, {} as EnvHost, "bridge")
      expect(context.globalStorageUri.fsPath.replaceAll("\\", "/")).toContain("/profile-a/globalStorage/bridge")
      expect(context.kaminProfile.isDefault).toBe(isDefault ?? false)
    })
  }
})
