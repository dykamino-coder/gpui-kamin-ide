import fsp from 'node:fs/promises'

export type McpResult = { content: Array<{ type: string; text: string }> }

// Same ceiling the Read tool enforces. .ipynb is JSON we sync-parse, so an
// uncapped read would block the shared exthost event loop (whole-app freeze)
// for the entire load of a multi-GB file. Reject early on size instead.
const MAX_NOTEBOOK_BYTES = 50 * 1024 * 1024

async function readNotebookCapped(path: string): Promise<string> {
  const st = await fsp.stat(path)
  if (st.size > MAX_NOTEBOOK_BYTES) {
    throw new Error(`Notebook too large (${Math.round(st.size / 1024 / 1024)} MB > 50 MB cap)`)
  }
  return fsp.readFile(path, 'utf8')
}

function textResult(text: string): McpResult {
  return { content: [{ type: 'text', text }] }
}

function errorResult(message: string): McpResult {
  return { content: [{ type: 'text', text: `Error: ${message}` }] }
}

interface NotebookCell {
  cell_type: string
  source: string | string[]
  metadata?: Record<string, unknown>
  outputs?: unknown[]
  execution_count?: number | null
  id?: string
}

interface Notebook {
  nbformat: number
  nbformat_minor: number
  metadata: Record<string, unknown>
  cells: NotebookCell[]
}

/**
 * NotebookRead — Read and render a Jupyter notebook's cells with outputs.
 */
export async function notebookRead(input: Record<string, unknown>): Promise<McpResult> {
  const notebookPath = input.notebook_path as string || input.file_path as string
  if (!notebookPath) return errorResult('notebook_path is required')

  try {
    const raw = await readNotebookCapped(notebookPath)
    const notebook: Notebook = JSON.parse(raw)

    if (!notebook.cells || !Array.isArray(notebook.cells)) {
      return errorResult('Invalid notebook format: no cells array found')
    }

    const lines: string[] = []
    lines.push(`Notebook: ${notebookPath}`)
    lines.push(`Format: ${notebook.nbformat}.${notebook.nbformat_minor}`)
    lines.push(`Cells: ${notebook.cells.length}`)
    lines.push('')

    for (let i = 0; i < notebook.cells.length; i++) {
      const cell = notebook.cells[i]!
      const source = Array.isArray(cell.source) ? cell.source.join('') : cell.source
      const execStr = cell.execution_count != null ? ` [${cell.execution_count}]` : ''

      lines.push(`--- Cell ${i} (${cell.cell_type})${execStr} ---`)
      lines.push(source)

      // Render outputs for code cells
      if (cell.cell_type === 'code' && cell.outputs && Array.isArray(cell.outputs)) {
        for (const output of cell.outputs) {
          const out = output as Record<string, unknown>
          if (out.output_type === 'stream') {
            const text = Array.isArray(out.text) ? (out.text as string[]).join('') : out.text as string
            lines.push(`[Output: ${out.name || 'stdout'}]`)
            lines.push(text)
          } else if (out.output_type === 'execute_result' || out.output_type === 'display_data') {
            const data = out.data as Record<string, unknown> | undefined
            if (data?.['text/plain']) {
              const text = Array.isArray(data['text/plain']) ? (data['text/plain'] as string[]).join('') : data['text/plain'] as string
              lines.push('[Output]')
              lines.push(text)
            }
          } else if (out.output_type === 'error') {
            lines.push(`[Error: ${out.ename}] ${out.evalue}`)
          }
        }
      }

      lines.push('')
    }

    return textResult(lines.join('\n'))
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    if (msg.includes('ENOENT')) return errorResult(`Notebook not found: ${notebookPath}`)
    if (msg.includes('JSON')) return errorResult(`Failed to parse notebook: ${msg}`)
    return errorResult(msg)
  }
}

export async function notebookEdit(input: Record<string, unknown>): Promise<McpResult> {
  const notebookPath = input.notebook_path as string
  if (!notebookPath) return errorResult('notebook_path is required')

  const newSource = input.new_source as string
  const cellType = (input.cell_type as string) || undefined
  const editMode = (input.edit_mode as string) || 'replace'

  // Support both cell_id (string, CLI format) and cell_number (number, legacy)
  const cellId = input.cell_id as string | undefined
  const cellNumberRaw = typeof input.cell_number === 'number' ? input.cell_number : undefined
  // If cell_id is provided, resolve to index; otherwise use cell_number
  let cellNumber: number | undefined = cellNumberRaw

  try {
    const raw = await readNotebookCapped(notebookPath)
    const notebook: Notebook = JSON.parse(raw)

    if (!notebook.cells || !Array.isArray(notebook.cells)) {
      return errorResult('Invalid notebook format: no cells array found')
    }

    // Resolve cell_id to cell index if provided
    if (cellId && cellNumber === undefined) {
      const idx = notebook.cells.findIndex(c => c.id === cellId)
      if (idx === -1) {
        return errorResult(`Cell with id "${cellId}" not found in notebook`)
      }
      cellNumber = idx
    }

    switch (editMode) {
      case 'replace': {
        if (cellNumber === undefined) return errorResult('cell_number is required for replace mode')
        if (cellNumber < 0 || cellNumber >= notebook.cells.length) {
          return errorResult(
            `cell_number ${cellNumber} is out of range (notebook has ${notebook.cells.length} cells)`
          )
        }
        if (typeof newSource !== 'string') return errorResult('new_source is required for replace mode')

        const cell = notebook.cells[cellNumber]!
        cell.source = newSource.split('\n').map((line, i, arr) =>
          i < arr.length - 1 ? line + '\n' : line
        )
        if (cellType) {
          cell.cell_type = cellType
        }
        // Clear outputs for code cells
        if (cell.cell_type === 'code') {
          cell.outputs = []
          cell.execution_count = null
        }

        await fsp.writeFile(notebookPath, JSON.stringify(notebook, null, 1) + '\n', 'utf8')
        return textResult(`Replaced cell ${cellNumber} in ${notebookPath}`)
      }

      case 'insert': {
        if (!cellType) return errorResult('cell_type is required for insert mode')
        if (typeof newSource !== 'string') return errorResult('new_source is required for insert mode')

        const insertIndex = cellNumber !== undefined ? cellNumber : notebook.cells.length

        if (insertIndex < 0 || insertIndex > notebook.cells.length) {
          return errorResult(
            `cell_number ${insertIndex} is out of range for insert (valid: 0-${notebook.cells.length})`
          )
        }

        const newCell: NotebookCell = {
          cell_type: cellType,
          source: newSource.split('\n').map((line, i, arr) =>
            i < arr.length - 1 ? line + '\n' : line
          ),
          metadata: {},
        }

        if (cellType === 'code') {
          newCell.outputs = []
          newCell.execution_count = null
        }

        notebook.cells.splice(insertIndex, 0, newCell)
        await fsp.writeFile(notebookPath, JSON.stringify(notebook, null, 1) + '\n', 'utf8')
        return textResult(`Inserted ${cellType} cell at position ${insertIndex} in ${notebookPath}`)
      }

      case 'delete': {
        if (cellNumber === undefined) return errorResult('cell_number is required for delete mode')
        if (cellNumber < 0 || cellNumber >= notebook.cells.length) {
          return errorResult(
            `cell_number ${cellNumber} is out of range (notebook has ${notebook.cells.length} cells)`
          )
        }

        const deleted = notebook.cells.splice(cellNumber, 1)[0]!
        await fsp.writeFile(notebookPath, JSON.stringify(notebook, null, 1) + '\n', 'utf8')
        return textResult(
          `Deleted ${deleted.cell_type} cell at position ${cellNumber} from ${notebookPath}`
        )
      }

      default:
        return errorResult(`Unknown edit_mode: ${editMode}. Valid modes: replace, insert, delete`)
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    if (msg.includes('ENOENT')) return errorResult(`Notebook not found: ${notebookPath}`)
    if (msg.includes('JSON')) return errorResult(`Failed to parse notebook JSON: ${msg}`)
    return errorResult(msg)
  }
}
