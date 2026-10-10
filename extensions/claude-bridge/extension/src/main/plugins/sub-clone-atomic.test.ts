import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { syncPluginCacheFromSubClone } from './sub-clone'

const fixture = vi.hoisted(() => ({ home: '' }))
vi.mock('os', async original => ({
  ...(await original<typeof import('node:os')>()),
  default: { ...(await original<typeof import('node:os')>()), homedir: () => fixture.home },
}))
let root = '', source = '', installed = '', oldCache = '', parent = ''
const revision = 'b'.repeat(40)
beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'inc-0044-atomic-'))
  fixture.home = root
  source = path.join(root, 'market', 'plugins', 'synthetic')
  fs.mkdirSync(path.join(source, '.claude-plugin'), { recursive: true })
  fs.writeFileSync(path.join(source, 'payload.txt'), 'new payload')
  installed = path.join(root, '.claude', 'plugins', 'installed_plugins.json')
  parent = path.join(root, '.claude', 'plugins', 'cache', 'fixture', 'synthetic')
  oldCache = path.join(parent, '1.0.0')
  fs.mkdirSync(oldCache, { recursive: true })
  fs.writeFileSync(path.join(oldCache, 'payload.txt'), 'working payload')
  fs.writeFileSync(installed, JSON.stringify({ version: 2, plugins: {
    'synthetic@fixture': [{ version: '1.0.0', installPath: oldCache, scope: 'user' }],
    'unrelated@fixture': [{ installPath: 'synthetic-unrelated', version: '7' }],
  } }))
})
afterEach(() => {
  vi.restoreAllMocks()
  const resolved = path.resolve(root)
  if (path.dirname(resolved) !== path.resolve(os.tmpdir()) || !path.basename(resolved).startsWith('inc-0044-atomic-')) {
    throw new Error('Unexpected fixture cleanup path')
  }
  fs.rmSync(resolved, { recursive: true, force: true })
})

it.each(['1.0.0', '2.0.0'].flatMap(version =>
  ['copy', 'metadata write', 'payload rename', 'metadata rename'].map(failure => ({ version, failure })),
))('preserves the working cache and metadata after $failure ($version), then converges', ({ version, failure }) => {
  fs.writeFileSync(path.join(source, '.claude-plugin', 'plugin.json'), JSON.stringify({ version }))
  const before = fs.readFileSync(installed, 'utf8')
  const copy = fs.copyFileSync.bind(fs), write = fs.writeFileSync.bind(fs), rename = fs.renameSync.bind(fs)
  if (failure === 'copy') vi.spyOn(fs, 'copyFileSync').mockImplementation((from, to, mode) => {
    copy(from, to, mode)
    throw new Error('Synthetic partial copy failure')
  })
  if (failure === 'metadata write') vi.spyOn(fs, 'writeFileSync').mockImplementation((file, data, options) => {
    write(file, '{', options)
    throw new Error('Synthetic partial metadata write failure')
  })
  if (failure.endsWith('rename')) vi.spyOn(fs, 'renameSync').mockImplementation((from, to) => {
    const metadata = String(to) === installed
    if (metadata === (failure === 'metadata rename')) throw new Error('Synthetic Windows rename lock')
    rename(from, to)
  })
  const result = syncPluginCacheFromSubClone('synthetic', 'fixture', path.join(root, 'market'), revision)
  expect(result.ok).toBe(false)
  expect(fs.readFileSync(installed, 'utf8')).toBe(before)
  expect(fs.readFileSync(path.join(oldCache, 'payload.txt'), 'utf8')).toBe('working payload')
  expect(fs.readdirSync(parent)).toEqual(['1.0.0'])
  expect(fs.readdirSync(path.dirname(installed))).not.toContain(expect.stringMatching(/\.tmp$/))
  vi.restoreAllMocks()
  expect(syncPluginCacheFromSubClone('synthetic', 'fixture', path.join(root, 'market'), revision).ok).toBe(true)
  const data = JSON.parse(fs.readFileSync(installed, 'utf8'))
  const entry = data.plugins['synthetic@fixture'][0]
  expect(entry.version).toBe(version)
  expect(entry.sourceRevision).toBe(revision)
  expect(entry.installPath).not.toBe(oldCache)
  expect(fs.readFileSync(path.join(entry.installPath, 'payload.txt'), 'utf8')).toBe('new payload')
  expect(fs.readFileSync(path.join(oldCache, 'payload.txt'), 'utf8')).toBe('working payload')
  expect(data.plugins['unrelated@fixture']).toEqual(JSON.parse(before).plugins['unrelated@fixture'])
  const repeatedCopy = vi.spyOn(fs, 'copyFileSync')
  expect(syncPluginCacheFromSubClone('synthetic', 'fixture', path.join(root, 'market'), revision).changed).toBe(false)
  expect(repeatedCopy).not.toHaveBeenCalled()
})
