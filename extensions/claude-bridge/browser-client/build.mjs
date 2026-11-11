// ESM bundles and declaration bundles of @kaminide/bridge-browser-client.
// Declarations are generated from src/, never written by hand.
import { build } from 'esbuild'
import { execFileSync } from 'node:child_process'
import path from 'node:path'

const here = import.meta.dirname
await build({
  absWorkingDir: here,
  entryPoints: { index: 'src/index.ts', preview: 'src/preview.ts' },
  outdir: 'dist',
  bundle: true,
  platform: 'browser',
  format: 'esm',
  target: 'es2022',
  sourcemap: true,
  logLevel: 'warning',
})
for (const entry of ['index', 'preview']) {
  execFileSync(process.execPath, [path.join(here, 'node_modules/dts-bundle-generator/dist/bin/dts-bundle-generator.js'),
    '--silent', '--no-banner', '--project', 'tsconfig.json', '--export-referenced-types', 'false', '-o', `dist/${entry}.d.ts`, `src/${entry}.ts`],
  { cwd: here, stdio: 'inherit' })
}
