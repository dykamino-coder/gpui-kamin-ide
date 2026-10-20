import { fileURLToPath } from 'node:url'

export default {
  // Same alias as vite.config.ts, so tests can import real renderer modules
  // (JsonlEntry → … → theme) instead of re-implementing their decisions.
  resolve: {
    alias: {
      '@bridge/storage': fileURLToPath(new URL('./src/lib/storage.ts', import.meta.url)),
    },
  },
  test: {
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist'],
    silent: 'passed-only',
  },
}
