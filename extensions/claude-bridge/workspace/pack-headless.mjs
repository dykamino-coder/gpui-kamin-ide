// Pack the UI-less workspace host as an installable npm tarball:
//   node pack-headless.mjs [--version <semver>] [--out <directory>]
// The package holds the exact dist/headless.cjs that dist/cli.cjs runs, its
// generated declarations and the runtime packages that bundle keeps external.
// The tarball name is stable: kaminide-bridge-workspace-host-<version>.tgz.
// Its version defaults to the Bridge server version of this checkout, the
// server it is released and tested with.
import { execFileSync } from 'node:child_process'
import { builtinModules } from 'node:module'
import fs from 'node:fs'
import path from 'node:path'

const here = import.meta.dirname
const readJson = file => JSON.parse(fs.readFileSync(path.join(here, file), 'utf8'))
const argument = name => {
  const index = process.argv.indexOf(name)
  if (index < 0) return undefined
  const value = process.argv[index + 1]
  if (!value || value.startsWith('--')) throw new Error(`${name} needs a value`)
  return value
}

const serverVersion = readJson('../server/package.json').version
const version = argument('--version') ?? serverVersion
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/.test(version)) throw new Error('--version must be semver, e.g. 6.3.170 or 6.3.170-studio.1')
const destination = path.resolve(argument('--out') ?? path.join(here, 'dist-packages'))
const stage = path.join(destination, 'workspace-host')
for (const file of ['dist/headless.cjs', 'dist/headless.d.ts', 'dist/headless.imports.json']) {
  if (!fs.existsSync(path.join(here, file))) throw new Error(`${file} is missing; run npm run build first`)
}
fs.rmSync(stage, { recursive: true, force: true })
fs.mkdirSync(stage, { recursive: true })

// Declare exactly the packages the bundle loads. A missing declaration would
// install cleanly and fail at the first require.
const workspace = readJson('package.json')
const builtins = new Set(builtinModules)
const packageName = specifier => specifier.startsWith('@') ? specifier.split('/').slice(0, 2).join('/') : specifier.split('/')[0]
const dependencies = {}
const missing = []
for (const specifier of readJson('dist/headless.imports.json')) {
  if (specifier.startsWith('node:') || builtins.has(packageName(specifier))) continue
  const name = packageName(specifier)
  if (workspace.dependencies[name]) dependencies[name] = workspace.dependencies[name]
  else missing.push(name)
}
if (missing.length) throw new Error(`Bundle imports undeclared packages: ${[...new Set(missing)].sort().join(', ')}`)

fs.copyFileSync(path.join(here, 'dist/headless.cjs'), path.join(stage, 'headless.cjs'))
fs.copyFileSync(path.join(here, 'dist/headless.d.ts'), path.join(stage, 'headless.d.ts'))
fs.copyFileSync(path.join(here, 'HEADLESS.md'), path.join(stage, 'README.md'))
fs.writeFileSync(path.join(stage, 'package.json'), JSON.stringify({
  name: '@kaminide/bridge-workspace-host',
  version,
  description: 'Kamin Bridge workspace service without a browser UI, for embedding in another application.',
  main: './headless.cjs',
  types: './headless.d.ts',
  exports: { '.': { types: './headless.d.ts', default: './headless.cjs' } },
  files: ['headless.cjs', 'headless.d.ts', 'README.md'],
  engines: workspace.engines,
  bridge: { server: serverVersion },
  dependencies: Object.fromEntries(Object.entries(dependencies).sort(([a], [b]) => a.localeCompare(b))),
}, null, 2) + '\n')

const [packed] = JSON.parse(execFileSync('npm', ['pack', '--json', '--pack-destination', destination], {
  cwd: stage, encoding: 'utf8', shell: process.platform === 'win32',
}))
const expected = `kaminide-bridge-workspace-host-${version}.tgz`
if (packed.filename !== expected) throw new Error(`Unexpected tarball name ${packed.filename}`)
process.stdout.write(`${path.join(destination, packed.filename)}\n`)
