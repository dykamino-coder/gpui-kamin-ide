// Pack @kaminide/bridge-browser-client as an installable npm tarball:
//   node pack.mjs [--version <semver>] [--out <directory>]
// The tarball name is stable: kaminide-bridge-browser-client-<version>.tgz. Its
// version defaults to the Bridge server version of this checkout, the same
// version as the matching @kaminide/bridge-workspace-host tarball.
import { execFileSync } from 'node:child_process'
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
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/.test(version)) throw new Error('--version must be semver')
const destination = path.resolve(argument('--out') ?? path.join(here, 'dist-packages'))
const stage = path.join(destination, 'browser-client')
const files = ['index.js', 'index.js.map', 'index.d.ts', 'preview.js', 'preview.js.map', 'preview.d.ts']
for (const file of files) if (!fs.existsSync(path.join(here, 'dist', file))) throw new Error(`dist/${file} is missing; run npm run build first`)
fs.rmSync(stage, { recursive: true, force: true })
fs.mkdirSync(stage, { recursive: true })
for (const file of files) fs.copyFileSync(path.join(here, 'dist', file), path.join(stage, file))
fs.copyFileSync(path.join(here, 'README.md'), path.join(stage, 'README.md'))

// Preview rendering takes React and the federation runtime as arguments; the
// peers document the versions the bundled UI is tested with.
const webview = JSON.parse(fs.readFileSync(path.join(here, '../webview/package.json'), 'utf8')).dependencies
fs.writeFileSync(path.join(stage, 'package.json'), JSON.stringify({
  name: '@kaminide/bridge-browser-client',
  version,
  description: 'Framework-agnostic browser client of a Kamin Bridge workspace host: /bridge transport, typed channels and Module Federation preview.',
  type: 'module',
  main: './index.js',
  types: './index.d.ts',
  exports: {
    '.': { types: './index.d.ts', default: './index.js' },
    './preview': { types: './preview.d.ts', default: './preview.js' },
  },
  sideEffects: false,
  files: [...files, 'README.md'],
  bridge: { server: serverVersion },
  peerDependencies: {
    '@module-federation/enhanced': webview['@module-federation/enhanced'],
    react: webview.react,
    'react-dom': webview['react-dom'],
    'react-refresh': webview['react-refresh'],
  },
  peerDependenciesMeta: Object.fromEntries(['@module-federation/enhanced', 'react', 'react-dom', 'react-refresh']
    .map(name => [name, { optional: true }])),
}, null, 2) + '\n')

const [packed] = JSON.parse(execFileSync('npm', ['pack', '--json', '--pack-destination', destination], {
  cwd: stage, encoding: 'utf8', shell: process.platform === 'win32',
}))
const expected = `kaminide-bridge-browser-client-${version}.tgz`
if (packed.filename !== expected) throw new Error(`Unexpected tarball name ${packed.filename}`)
process.stdout.write(`${path.join(destination, packed.filename)}\n`)
