// Run inside the built workspace image with WORKSPACE_CONTAINER_SMOKE=1.
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'

const skip = process.env.WORKSPACE_CONTAINER_SMOKE !== '1'

test('container runs the pinned Node and ripgrep releases', { skip }, () => {
  assert.equal(process.versions.node, '22.23.5')
  const version = spawnSync('rg', ['--version'], { encoding: 'utf8' })
  assert.equal(version.status, 0)
  assert.match(version.stdout, /^ripgrep 15\.2\.0\b/)
})

test('Grep excludes no lines when its pattern file is empty', { skip }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'workspace-rg-'))
  try {
    const patterns = join(dir, 'patterns')
    const input = join(dir, 'input.txt')
    writeFileSync(patterns, '')
    writeFileSync(input, 'first\nsecond\n')
    const result = spawnSync('rg', ['--no-config', '-v', '-f', patterns, input], { encoding: 'utf8' })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout, 'first\nsecond\n')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('Grep supports PCRE2 and Unicode in the shipped binary', { skip }, () => {
  const result = spawnSync('rg', ['--no-config', '--pcre2', '(?<=модель )GPT'], {
    input: 'модель GPT\nClaude\n', encoding: 'utf8',
  })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, 'модель GPT\n')
})
