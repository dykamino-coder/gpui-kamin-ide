import { describe, expect, it, vi } from "vitest";
import { submitSessionRename } from "./session-actions";
// @ts-expect-error Vitest/Vite resolves raw source imports at test runtime.
import sessionItemSource from "../components/sidebar/sessions/SessionItem.tsx?raw";
// @ts-expect-error Vitest/Vite resolves raw source imports at test runtime.
import treeNodeSource from "../components/sidebar/tree/TreeNode.tsx?raw";

describe("session actions", () => {
  it("submits rename through the semantic coordinator path", () => {
    const bridge = { submitText: vi.fn() };

    submitSessionRename(bridge, "tab-1");

    expect(bridge.submitText).toHaveBeenCalledTimes(1);
    expect(bridge.submitText).toHaveBeenCalledWith("tab-1", "/rename");
  });

  it("stays connected to both sidebar rename callers", () => {
    for (const source of [sessionItemSource, treeNodeSource]) {
      expect(source).toContain("submitSessionRename(bridge,");
      expect(source).not.toMatch(/sendInput\([^\n]*\/rename/);
    }
  });
});
