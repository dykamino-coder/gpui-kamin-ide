// Bundles bench/tool-latency.ts with the extension's own esbuild and runs it.
// Usage: node bench/run-tool-latency.mjs [--out <dir>] [--runs N] [--tree <dir>]
// Optional env: BENCH_RG_DIR (directory with rg/rg.exe for the "rg on PATH"
// variant and the bare-rg baseline), KAMIN_JOBRUN_EXE (Windows job runner).
import esbuild from 'esbuild'
import { spawnSync } from 'node:child_process'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const dir = path.dirname(fileURLToPath(import.meta.url))
const outfile = path.join(os.tmpdir(), `kamin-tool-latency-${process.pid}.cjs`)
await esbuild.build({
  entryPoints: [path.join(dir, 'tool-latency.ts')],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node18',
  outfile,
  external: ['vscode', 'kaminide', 'bufferutil', 'utf-8-validate'],
  alias: { '@kaminide/host-compat': path.resolve(dir, '../src/shim/host-compat.ts') },
  define: { __dirname: JSON.stringify(dir) },
  logLevel: 'warning',
})
const result = spawnSync(process.execPath, [outfile, ...process.argv.slice(2)], { stdio: 'inherit' })
process.exitCode = result.status ?? 1
