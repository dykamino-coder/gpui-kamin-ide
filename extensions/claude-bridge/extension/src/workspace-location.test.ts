import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { parseWorkspaceLocation, projectTextFile, workspaceLocation } from './workspace-location'

describe('opening a chat location in the native text editor', () => {
  it.each([
    ['C:/Fixture/sample file.ts:4:2', 'C:/Fixture/sample file.ts', 4, 2],
    ['C:\\Fixture\\пример.ts:7', 'C:\\Fixture\\пример.ts', 7, 1],
    ['/C:/Fixture/sample.ts#L3C8', 'C:/Fixture/sample.ts', 3, 8],
    ['file:///C:/Fixture/space%20and%23L2.ts#L4', 'C:/Fixture/space and#L2.ts', 4, 1],
    ['file:///C:/Fixture/a%23L6', 'C:/Fixture/a#L6', 1, 1],
  ])('reads %s', (destination, file, line, column) => {
    expect(parseWorkspaceLocation(String(destination))).toEqual({ path: file, line, column })
  })
  it.each(['file://server/share', 'https://example.test/', '../sibling/file', 'C:/f:0', 'C:/f:4294967296'])(
    'rejects unsupported location %s', value => expect(parseWorkspaceLocation(value)).toBeNull())

  it('opens files under the active project and refuses directories, missing files and sibling projects', async () => {
    const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'kamin-location-'))
    try {
      const root = path.join(tmp, 'project')
      const other = path.join(tmp, 'project-other')
      await Promise.all([fs.mkdir(root), fs.mkdir(other)])
      const file = path.join(root, 'пример file.ts')
      const outside = path.join(other, 'other.ts')
      await Promise.all([fs.writeFile(file, 'one\ntwo'), fs.writeFile(outside, 'separate')])
      expect(await projectTextFile(root, file)).toBe(await fs.realpath(file))
      for (const target of [outside, root, path.join(root, 'missing')]) {
        expect(await projectTextFile(root, target)).toBeNull()
      }
      await fs.symlink(other, path.join(root, 'linked'), process.platform === 'win32' ? 'junction' : 'dir')
      expect(await projectTextFile(root, path.join(root, 'linked', 'other.ts'))).toBeNull()
      if (process.platform === 'win32') {
        expect(await workspaceLocation(root, file + ':2:1')).toEqual({ path: await fs.realpath(file), line: 2, column: 1 })
      }
    } finally { await fs.rm(tmp, { recursive: true, force: true }) }
  })
})
