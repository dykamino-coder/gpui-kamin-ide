// Build on the host, then mount the result in the pinned Bridge runtime image.
// Usage: node bench/build-native-tools.mjs <external-output-directory>
import esbuild from 'esbuild'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const dir = path.dirname(fileURLToPath(import.meta.url))
const out = process.argv[2]
if (!out) throw new Error('An external output directory is required')
fs.mkdirSync(out, { recursive: true })
await esbuild.build({
  entryPoints: [path.join(dir, 'native-tool-worker.ts')],
  outfile: path.join(out, 'native-tool-worker.cjs'),
  bundle: true, platform: 'node', format: 'cjs', target: 'node22',
  logLevel: 'warning',
})
fs.copyFileSync(path.join(dir, 'native-tools.mjs'), path.join(out, 'native-tools.mjs'))
