import { build } from 'esbuild'
import path from 'node:path'
import { readFile } from 'node:fs/promises'

await build({
  entryPoints: ['src/index.ts'],
  outfile: 'dist/index.js',
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node22',
  packages: 'external',
})

// Use the SAME BridgeHost and handler graph as the VSIX. Only the VS Code API
// boundary is replaced; the host-compat IPC facade itself remains shared.
await build({
  entryPoints: { runtime: 'src/runtime.ts', cli: 'src/cli.ts' },
  outdir: 'dist',
  outExtension: { '.js': '.cjs' },
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node22',
  define: { BRIDGE_VERSION: JSON.stringify(JSON.parse(await readFile('../../../builtin-extensions/claude-bridge/package.json', 'utf8')).version) },
  packages: 'external',
  alias: {
    vscode: path.resolve('src/environment.ts'),
    '@kaminide/host-compat': path.resolve('../extension/src/shim/host-compat.ts'),
  },
  logOverride: { 'import-is-undefined': 'error' },
})
