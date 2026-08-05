import path from 'node:path'
import { fileURLToPath } from 'node:url'

const dir = path.dirname(fileURLToPath(import.meta.url))

export default {
  resolve: {
    alias: {
      '@kaminide/host-compat': path.resolve(dir, 'src/shim/host-compat.ts'),
    },
  },
  test: {
    include: ['src/**/*.{test,spec}.ts'],
    exclude: ['node_modules', 'dist'],
    silent: 'passed-only',
  },
}
