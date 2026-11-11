#!/usr/bin/env node
// Build and pack the embedding packages attached to every GitHub Release:
//   kaminide-bridge-workspace-host-<version>.tgz  (@kaminide/bridge-workspace-host)
//   kaminide-bridge-browser-client-<version>.tgz  (@kaminide/bridge-browser-client)
// <version> is the Bridge server version of the checkout, so asset names are
// stable per release. Dependencies of the root, extension, workspace and
// browser-client packages must already be installed (npm ci).
//   node scripts/release/pack-embedding.mjs --out <directory>
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const index = process.argv.indexOf('--out')
if (index < 0 || !process.argv[index + 1]) throw new Error('Usage: pack-embedding.mjs --out <directory>')
const out = path.resolve(process.argv[index + 1])
fs.mkdirSync(out, { recursive: true })
const version = JSON.parse(fs.readFileSync(path.join(root, 'extensions/claude-bridge/server/package.json'), 'utf8')).version

const packages = [
  { directory: 'extensions/claude-bridge/workspace', pack: 'pack-headless.mjs', asset: `kaminide-bridge-workspace-host-${version}.tgz` },
  { directory: 'extensions/claude-bridge/browser-client', pack: 'pack.mjs', asset: `kaminide-bridge-browser-client-${version}.tgz` },
]
for (const item of packages) {
  const cwd = path.join(root, item.directory)
  execFileSync(process.execPath, ['build.mjs'], { cwd, stdio: 'inherit' })
  const printed = execFileSync(process.execPath, [item.pack, '--version', version, '--out', out], { cwd, encoding: 'utf8' }).trim()
  if (path.basename(printed) !== item.asset || !fs.statSync(printed).size) throw new Error(`Unexpected package ${printed}`)
  process.stdout.write(`${printed}\n`)
}
