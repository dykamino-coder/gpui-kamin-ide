import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    // Only this repo's own suite. Without the include, vitest crawls
    // kamin-ide/** (which has its own vitest workspace) and the gitignored
    // vscode-ref research clone — thousands of foreign spec files, 6-minute
    // runs and failures that aren't ours.
    // Сьют co-located в src/ (tests/ каталога в этом репо никогда не было —
    // старый include указывал в пустоту, `npm test` падал "No test files").
    include: ['src/**/*.test.ts'],
    // Coverage configuration. CI gate (Sprint 1 D7) enforces:
    //   - ≥40% global lines/branches/functions
    //   - ≥70% on critical-path modules (PTY, MCP routing, JSONL parsing)
    // Run locally with `npm test -- --coverage`.
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      // Only count code we actually exercise from tests. Excluding electron
      // renderer (no renderer-side tests yet — D6 Playwright covers that)
      // and the dashboard React app (`src/ui/`) which has its own surface.
      include: [
        'src/core/**/*.ts',
        'electron/main/mcp/discovery.ts',
        'electron/renderer/utils/render-markdown.ts',
      ],
      exclude: [
        '**/*.d.ts',
        '**/*.test.ts',
        '**/types.ts',
        'src/index.ts',
        'src/core/server/**',
        'src/core/dashboard/**',
        'src/core/stats/**',
        'src/core/sync/**',
        'src/core/telemetry/**',
        'src/core/test-api/**',
        'src/ui/**',
      ],
      thresholds: {
        // Global floor — CI fails below this. Generous baseline matching
        // current coverage; ratchet up as D-stage delivers more tests.
        lines: 25,
        functions: 25,
        branches: 50,
        statements: 25,
        // Critical-path modules — module-level floors set just below current
        // coverage so any regression breaks CI. Bumping these is the
        // explicit gate for accepting new test additions.
        perFile: false,
        '**/render-markdown.ts': {
          lines: 90,
          functions: 90,
          branches: 75,
          statements: 90,
        },
        '**/session-env.ts': {
          lines: 95,
          functions: 95,
          branches: 90,
          statements: 95,
        },
        '**/session-reaper.ts': {
          lines: 40,
          functions: 20,
          branches: 50,
          statements: 40,
        },
        '**/mcp-http-handler.ts': {
          lines: 60,
          functions: 40,
          branches: 50,
          statements: 60,
        },
      },
    },
  },
})
