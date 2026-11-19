import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

// No CI job builds workspace/Dockerfile, so a new cross-package import from the
// bundled sources silently breaks the documented image build. Every relative
// import that leaves a copied source tree must land inside another COPY source
// of the build stage.
const workspaceDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoRoot = path.resolve(workspaceDir, '../../..')
const dockerfile = readFileSync(path.join(workspaceDir, 'Dockerfile'), 'utf8')

function buildStageCopySources(): string[] {
  const buildStage = dockerfile.split(/^FROM /m).find((stage) => /\bAS build\b/i.test(stage.split('\n')[0]))
  assert.ok(buildStage, 'Dockerfile has a build stage')
  return [...buildStage.matchAll(/^COPY (?!--from)(?:--\S+ )*(\S+) \S+$/gm)].map((m) => path.posix.normalize(m[1]))
}

function sourceFiles(dir: string): string[] {
  const out: string[] = []
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'dist') continue
    const full = path.join(dir, name)
    if (statSync(full).isDirectory()) out.push(...sourceFiles(full))
    else if (/\.(ts|tsx|mts|mjs|js)$/.test(name) && !/\.test\.|\.spec\./.test(name)) out.push(full)
  }
  return out
}

test('workspace Dockerfile build stage copies every directory the bundled sources import', () => {
  const sources = buildStageCopySources()
  const covered = (rel: string) => sources.some((src) => src === rel || (src.endsWith('/') && `${rel}/`.startsWith(src)))
  const bundled = ['webview/src', 'browser-client/src', 'workspace/src', 'extension/src']
    .map((dir) => `extensions/claude-bridge/${dir}`)
  for (const dir of bundled) assert.ok(covered(`${dir}/`), `${dir} is copied`)
  const missing = new Set<string>()
  for (const dir of bundled) {
    for (const file of sourceFiles(path.join(repoRoot, dir))) {
      const text = readFileSync(file, 'utf8')
      for (const m of text.matchAll(/(?:from|import\()\s*['"](\.\.?\/[^'"]+)['"]/g)) {
        const target = path.relative(repoRoot, path.resolve(path.dirname(file), m[1])).split(path.sep).join('/')
        if (!covered(target)) missing.add(`${path.relative(repoRoot, file).split(path.sep).join('/')} -> ${target}`)
      }
    }
  }
  assert.deepEqual([...missing], [])
})
