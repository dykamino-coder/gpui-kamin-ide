// Local Git fixture for INC-0044; only a disposable, explicitly supplied home.
import fs from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { execFileSync } from 'node:child_process'
export function seedPluginFixture(home, mode = 'stale-cache') {
  home = path.resolve(home)
  if (!['stale-cache', 'changed', 'disabled', 'offline'].includes(mode)) throw new Error('Unknown plugin fixture mode')
  const base = path.join(home, '.claude', 'plugins')
  if (
    fs.existsSync(path.join(base, 'known_marketplaces.json')) ||
    fs.existsSync(path.join(base, 'installed_plugins.json'))
  ) {
    throw new Error('Refusing to overwrite an existing plugin profile')
  }
  const write = (file, data) => {
    fs.mkdirSync(path.dirname(file), { recursive: true })
    fs.writeFileSync(file, JSON.stringify(data, null, 2))
  }
  const git = (cwd, ...args) =>
    execFileSync('git', ['-c', 'user.name=Synthetic Fixture', '-c', 'user.email=fixture@example.invalid', ...args], {
      cwd,
      stdio: 'pipe',
      env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
      windowsHide: true,
    })
  const source = path.join(home, 'synthetic-source')
  fs.mkdirSync(source, { recursive: true })
  git(source, 'init', '-b', 'main')
  const manifest = path.join(source, '.claude-plugin', 'plugin.json')
  write(manifest, { name: 'synthetic-worker', version: '1.0.0', description: 'Public synthetic fixture' })
  git(source, 'add', '.')
  git(source, 'commit', '-m', 'fixture: initial version')
  const marketplaceSource = path.join(home, 'synthetic-marketplace-source')
  fs.mkdirSync(marketplaceSource, { recursive: true })
  git(marketplaceSource, 'init', '-b', 'main')
  write(path.join(marketplaceSource, '.claude-plugin', 'marketplace.json'), {
    name: 'synthetic-marketplace',
    owner: { name: 'Synthetic Fixture' },
    plugins: [{ name: 'synthetic-worker', source: { source: 'url', url: source }, version: '2.0.0' }],
  })
  git(marketplaceSource, 'add', '.')
  git(marketplaceSource, 'commit', '-m', 'fixture: local marketplace')
  const checkout = path.join(base, 'marketplaces', 'synthetic-marketplace')
  fs.mkdirSync(path.dirname(checkout), { recursive: true })
  git(home, 'clone', marketplaceSource, checkout)
  const subclone = path.join(checkout, 'plugins', 'synthetic-worker')
  fs.mkdirSync(path.dirname(subclone), { recursive: true })
  git(home, 'clone', source, subclone)
  const cache = path.join(base, 'cache', 'synthetic-marketplace', 'synthetic-worker', '1.0.0')
  fs.cpSync(source, cache, { recursive: true, filter: (p) => !p.split(path.sep).includes('.git') })
  write(manifest, { name: 'synthetic-worker', version: '2.0.0', description: 'Public synthetic fixture' })
  git(source, 'add', '.')
  git(source, 'commit', '-m', 'fixture: advanced source')
  if (mode !== 'changed') git(subclone, 'pull', '--ff-only')
  if (mode === 'offline') git(subclone, 'remote', 'set-url', 'origin', path.join(home, 'missing-local-remote'))
  write(path.join(base, 'known_marketplaces.json'), {
    'synthetic-marketplace': {
      source: { source: 'git', url: marketplaceSource },
      installLocation: checkout,
      autoUpdate: mode !== 'disabled',
      lastUpdated: '2026-01-01T00:00:00.000Z',
    },
  })
  write(path.join(base, 'installed_plugins.json'), {
    version: 2,
    plugins: {
      'synthetic-worker@synthetic-marketplace': [
        { scope: 'user', installPath: cache, version: '1.0.0', installedAt: '2026-01-01T00:00:00.000Z' },
      ],
    },
  })
  return { source, checkout, subclone, cache }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  seedPluginFixture(process.argv[2], process.argv[3])
}
