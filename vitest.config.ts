// Vitest config — runs unit tests under `src/`. The Playwright-driven
// e2e file lives at `tests/e2e/` and is invoked separately via
// `npm run e2e` (it spawns a real Electron, so vitest's worker model
// can't host it).
import { defineConfig } from "vitest/config"

export default defineConfig({
  test: {
    include: [
      "src/**/*.{test,spec}.{ts,tsx}",
      // Bridge units that are free of the vscode/electron/preact-signal trees.
      "extensions/claude-bridge/extension/src/**/*.{test,spec}.ts",
      "extensions/claude-bridge/webview/src/**/*.{test,spec}.ts",
      "extensions/claude-bridge/server/src/**/*.{test,spec}.ts",
    ],
    exclude: ["node_modules", "dist", "out", ".vite", "tests/e2e/**"],
    // Console output from PASSING tests is noise, and forwarding it is what
    // produced the intermittent `EnvironmentTeardownError: Closing rpc while
    // "onUserConsoleLog" was pending` — a log racing the worker shutdown, which
    // failed the run at random and had nothing to do with the code under test.
    // Failures still print everything they logged.
    silent: "passed-only",
  },
})
