// Vitest config for the TypeScript unit suites. Native UI integration tests
// live outside this worker-based test contour.
import { defineConfig } from "vitest/config"

export default defineConfig({
  test: {
    include: [
      "src/**/*.{test,spec}.{ts,tsx}",
      // Bridge units that can run without the live vscode/webview runtimes.
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
