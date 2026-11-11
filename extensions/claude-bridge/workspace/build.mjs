import { build } from 'esbuild'
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
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

// cli.cjs requires ./headless.cjs instead of bundling a second copy: the
// container executable and @kaminide/bridge-workspace-host run one composition.
const sharedComposition = {
  name: 'shared-composition',
  setup(builder) {
    builder.onResolve({ filter: /^\.\/headless\.js$/ }, args =>
      path.basename(args.importer) === 'cli.ts' ? { path: './headless.cjs', external: true } : undefined)
  },
}

// Use the SAME BridgeHost and handler graph as the VSIX. Only the VS Code API
// boundary is replaced; the host-compat IPC facade itself remains shared.
const result = await build({
  entryPoints: { runtime: 'src/runtime.ts', headless: 'src/headless.ts', cli: 'src/cli.ts' },
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
  plugins: [sharedComposition],
  logOverride: { 'import-is-undefined': 'error' },
  metafile: true,
})

// Packages the host bundle loads at run time; pack-headless.mjs declares them.
const headless = Object.entries(result.metafile.outputs).find(([file]) => file.endsWith('headless.cjs'))[1]
fs.writeFileSync('dist/headless.imports.json', JSON.stringify([...new Set(headless.imports
  .filter(entry => entry.external).map(entry => entry.path))].sort(), null, 2) + '\n')

// Public declarations come from src/headless.ts itself, never a hand-written copy.
if (!process.argv.includes('--no-types')) {
  execFileSync(process.execPath, [path.resolve('node_modules/dts-bundle-generator/dist/bin/dts-bundle-generator.js'),
    '--silent', '--no-banner', '--project', 'tsconfig.runtime.json', '--export-referenced-types', 'false',
    '-o', 'dist/headless.d.ts', 'src/headless.ts'], { stdio: 'inherit' })
}
