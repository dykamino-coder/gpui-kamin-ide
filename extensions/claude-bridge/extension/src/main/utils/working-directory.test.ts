import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { resolveToolPath, resolveWorkingDirectory } from './working-directory'
import { editFile, globSearch, grepSearch, readFile, writeFile } from '../mcp/tools/file-system'
import { tabWorkingDirectory } from '../ws/mcp-call-pipeline'

let project: string

beforeAll(() => {
  project = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'kamin-wd-')))
})
afterAll(() => {
  fs.rmSync(project, { recursive: true, force: true })
})

function text(result: { content: Array<{ text?: string }> }): string {
  return result.content.map(part => part.text ?? '').join('\n')
}

describe('working directory rule', () => {
  it('takes the first directory that exists and falls back to home', () => {
    const missing = path.join(project, 'missing')
    const file = path.join(project, 'plain.txt')
    fs.writeFileSync(file, 'x')
    expect(resolveWorkingDirectory(missing, project)).toBe(project)
    expect(resolveWorkingDirectory(project, os.homedir())).toBe(project)
    expect(resolveWorkingDirectory(undefined, '', '  ', null, file, missing)).toBe(os.homedir())
    expect(resolveWorkingDirectory('/home/bridge/.claude/bridge-sessions/id/id')).toBe(os.homedir())
  })

  it('gives a tab without a folder the home directory, never the host process directory', () => {
    expect(tabWorkingDirectory(undefined)).toBe(os.homedir())
    expect(tabWorkingDirectory('')).toBe(os.homedir())
    expect(tabWorkingDirectory(path.join(project, 'removed'))).toBe(os.homedir())
    expect(tabWorkingDirectory(project)).toBe(project)
  })

  it('resolves relative tool paths against the working directory', () => {
    const absolute = path.join(project, 'a.txt')
    expect(resolveToolPath(absolute, os.homedir())).toBe(absolute)
    expect(resolveToolPath(path.join('sub', 'a.txt'), project)).toBe(path.join(project, 'sub', 'a.txt'))
    expect(resolveToolPath('a.txt', path.join(project, 'missing'))).toBe(path.join(os.homedir(), 'a.txt'))
  })
})

describe('file tools in the session working directory', () => {
  it('writes, reads and edits a relative path inside it', async () => {
    const relative = path.join('nested', 'note.txt')
    const onDisk = path.join(project, relative)
    const inProcessDir = path.resolve(relative)

    await writeFile({ file_path: relative, content: 'alpha working-directory-probe\n' }, { cwd: project })
    expect(fs.readFileSync(onDisk, 'utf8')).toBe('alpha working-directory-probe\n')
    expect(fs.existsSync(inProcessDir)).toBe(false)

    expect(text(await readFile({ file_path: relative }, { cwd: project }))).toContain('alpha working-directory-probe')

    await editFile({ file_path: relative, old_string: 'alpha', new_string: 'beta' }, { cwd: project })
    expect(fs.readFileSync(onDisk, 'utf8')).toBe('beta working-directory-probe\n')
  })

  it('searches the working directory when no path is given', async () => {
    fs.mkdirSync(path.join(project, 'nested'), { recursive: true })
    fs.writeFileSync(path.join(project, 'nested', 'found.kaminwd'), 'needle-kaminwd\n')

    const globbed = text(await globSearch({ pattern: '**/*.kaminwd' }, { cwd: project }))
    expect(globbed).toContain(path.join(project, 'nested', 'found.kaminwd').replace(/\\/g, '/'))

    const relativeGlob = text(await globSearch({ pattern: '*.kaminwd', path: 'nested' }, { cwd: project }))
    expect(relativeGlob).toContain('found.kaminwd')

    const grepped = text(await grepSearch({ pattern: 'needle-kaminwd' }, { cwd: project }))
    expect(grepped).toContain('found.kaminwd')
  })
})
