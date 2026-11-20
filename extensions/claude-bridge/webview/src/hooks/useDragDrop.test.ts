import { describe, expect, it } from 'vitest'
import { splitDroppedFolders } from './useDragDrop'

function item(name: string, isDirectory: boolean): DataTransferItem {
  const file = { name, type: '' } as File
  return {
    kind: 'file',
    type: '',
    getAsFile: () => file,
    webkitGetAsEntry: () => ({ name, isDirectory, isFile: !isDirectory }) as FileSystemEntry,
    getAsString: () => undefined,
  } as unknown as DataTransferItem
}

function transfer(items: DataTransferItem[]): DataTransfer {
  return { items, files: items.map(i => i.getAsFile()) } as unknown as DataTransfer
}

describe('splitDroppedFolders', () => {
  it('keeps every dropped file and names each folder for the refusal', () => {
    const { files, folders } = splitDroppedFolders(transfer([
      item('a.txt', false), item('photos', true), item('b.png', false),
    ]))
    expect(files.map(f => f.name)).toEqual(['a.txt', 'b.png'])
    expect(folders).toEqual(['photos'])
  })

  it('falls back to the file list when the drop carries no items', () => {
    const file = { name: 'c.md', type: '' } as File
    const { files, folders } = splitDroppedFolders({ items: [], files: [file] } as unknown as DataTransfer)
    expect(files).toEqual([file])
    expect(folders).toEqual([])
  })
})
