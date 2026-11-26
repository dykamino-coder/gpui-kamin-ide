import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { notebookEdit, notebookRead } from './notebook'

const dirs: string[] = []
afterEach(async () => {
  for (const dir of dirs.splice(0)) await fs.rm(dir, { recursive: true, force: true })
})

async function fixture(): Promise<string> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'bridge-notebook-'))
  dirs.push(dir)
  const file = path.join(dir, 'fixture.ipynb')
  await fs.writeFile(file, JSON.stringify({ nbformat: 4, nbformat_minor: 5, metadata: {}, cells: [
    { id: 'existing', cell_type: 'code', source: ['print("текст")'], metadata: {}, outputs: [], execution_count: null },
  ] }))
  return file
}

describe('Notebook tool result status', () => {
  it('marks failed Read and Edit results as MCP errors', async () => {
    const file = await fixture()
    const before = await fs.readFile(file)
    expect(await notebookEdit({ notebook_path: file, cell_id: 'missing', new_source: 'x' })).toMatchObject({ isError: true })
    expect(await notebookEdit({ notebook_path: file, edit_mode: 'unknown', new_source: 'x' })).toMatchObject({ isError: true })
    expect(await fs.readFile(file)).toEqual(before)
    expect(await notebookRead({ file_path: path.join(path.dirname(file), 'absent.ipynb') })).toMatchObject({ isError: true })
  })
})
