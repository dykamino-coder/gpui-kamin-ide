import { describe, expect, it, vi } from "vitest";
import { submitSessionRename } from "./session-actions";

describe("session actions", () => {
  it("submits rename through the semantic coordinator path", () => {
    const bridge = { submitText: vi.fn() };

    submitSessionRename(bridge, "tab-1");

    expect(bridge.submitText).toHaveBeenCalledTimes(1);
    expect(bridge.submitText).toHaveBeenCalledWith("tab-1", "/rename");
  });

});
