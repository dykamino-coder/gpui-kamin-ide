import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { resetWalkRoutingForTests, walk } from './walk'

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'walk-'))
afterAll(() => fs.rmSync(tmp, { recursive: true, force: true }))
beforeEach(() => resetWalkRoutingForTests())

const unlimited = () => ({ entries: 1e9, bytes: 1e12, deadline: Infinity })
const files = (root: string, budget = unlimited()): string[] | null => {
  const out: string[] = []
  return walk(root, budget, { dir: () => true, file: (_abs, rel) => void out.push(rel) }) ? out.sort() : null
}

describe('bounded walk', () => {
  const root = path.join(tmp, 'tree')
  for (let i = 0; i < 20; i++) fs.mkdirSync(path.join(root, `d${i % 4}`), { recursive: true })
  for (let i = 0; i < 20; i++) fs.writeFileSync(path.join(root, `d${i % 4}`, `f${i}.txt`), '')

  it('lists a tree within the budget and gives up past it', () => {
    expect(files(root)).toHaveLength(20)
    expect(files(root, { ...unlimited(), entries: 10 })).toBeNull()
  })

  it('sends a tree that ran out of budget, and searches covering it, straight to ripgrep', () => {
    expect(files(root, { ...unlimited(), entries: 10 })).toBeNull()
    expect(files(root)).toBeNull()
    expect(files(tmp)).toBeNull()
    // A smaller tree inside it is still walked.
    expect(files(path.join(root, 'd1'))).toEqual(['f1.txt', 'f13.txt', 'f17.txt', 'f5.txt', 'f9.txt'])
    resetWalkRoutingForTests()
    expect(files(root)).toHaveLength(20)
  })
})
