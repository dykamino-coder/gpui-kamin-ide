// Vitest config for the TypeScript unit suites. Native UI integration tests
// live outside this worker-based test contour.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Each Bridge package installs and tests its own dependency graph in a
    // dedicated CI job. Discovering those suites from the root made this job
    // fail on packages that intentionally are not root dependencies.
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["node_modules", "dist", "out", ".vite", "tests/e2e/**"],
    // Console output from PASSING tests is noise, and forwarding it is what
    // produced the intermittent `EnvironmentTeardownError: Closing rpc while
    // "onUserConsoleLog" was pending` — a log racing the worker shutdown, which
    // failed the run at random and had nothing to do with the code under test.
    // Failures still print everything they logged.
    silent: "passed-only",
  },
});
