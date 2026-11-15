// Line ranges for Read with offset/limit.
//
// Lines are numbered as `text.split('\n')` numbers them: a final line break
// opens one more, empty line, as in native Read. A file larger than the read
// cache keeps (2 MB) was read and decoded whole on every ranged call (~30 ms
// for 4 MB, more than the native tool spends in total); a UTF-8 file is now
// read only up to the last line asked for.
import fsp from 'node:fs/promises'
import { detectEncoding, looksBinary } from './text-codec'

export interface LineWindow {
  lines: string[]
  /** Another line follows the window. */
  more: boolean
  /** Line count of the whole file, computed when asked (a streamed window
   *  knows it only when it was read with `countAll`). */
  total: () => number
}

/** Lines [offset, offset + limit) of `text`, found without splitting all of it. */
export function lineWindow(text: string, offset: number, limit: number): LineWindow {
  let pos = 0
  for (let skipped = 0; skipped < offset; skipped++) {
    const nl = text.indexOf('\n', pos)
    if (nl < 0) return { lines: [], more: false, total: () => skipped + 1 }
    pos = nl + 1
  }
  const lines: string[] = []
  while (lines.length < limit) {
    const nl = text.indexOf('\n', pos)
    if (nl < 0) {
      lines.push(text.slice(pos))
      const count = offset + lines.length
      return { lines, more: false, total: () => count }
    }
    lines.push(text.slice(pos, nl))
    pos = nl + 1
  }
  const rest = pos
  const shown = offset + lines.length
  return {
    lines,
    more: true,
    total: () => {
      let count = shown + 1
      for (let at = text.indexOf('\n', rest); at >= 0; at = text.indexOf('\n', at + 1)) count++
      return count
    },
  }
}

const CHUNK = 1024 * 1024
const NEWLINE = 0x0a

/** The window of a UTF-8 file read from disk only as far as needed (to its end
 *  when `countAll`, for the "of N lines" note), or null when the file is not
 *  UTF-8 text (UTF-16, binary): those are read whole as before. Lines are found
 *  on the raw bytes (a 0x0A byte is always a line break in UTF-8); only the
 *  window is decoded. */
export async function readUtf8LineWindow(
  filePath: string,
  offset: number,
  limit: number,
  countAll: boolean,
): Promise<LineWindow | null> {
  const file = await fsp.open(filePath, 'r')
  try {
    let first = true
    let index = 0 // number of the line that starts at the current position
    const kept: Buffer[] = [] // bytes of the window, lines joined by '\n'
    let collected = 0 // complete window lines in `kept`
    const decodeWindow = (): string[] => {
      const text = Buffer.concat(kept).toString('utf8')
      return text.split('\n')
    }
    for (;;) {
      const buffer = Buffer.allocUnsafe(CHUNK)
      const { bytesRead } = await file.read(buffer, 0, CHUNK, null)
      let bytes = buffer.subarray(0, bytesRead)
      if (first) {
        first = false
        const encoding = detectEncoding(bytes)
        if ((encoding !== 'utf8' && encoding !== 'utf8bom') || looksBinary(bytes, encoding)) return null
        if (encoding === 'utf8bom') bytes = bytes.subarray(3)
      }
      if (bytesRead === 0) {
        // End of file: the line in progress is the last one.
        if (index < offset) return { lines: [], more: false, total: () => index + 1 }
        const lines = decodeWindow()
        const count = offset + lines.length
        return { lines, more: false, total: () => count }
      }
      let pos = 0
      while (pos <= bytes.length) {
        const nl = bytes.indexOf(NEWLINE, pos)
        if (nl < 0) {
          if (index >= offset) kept.push(bytes.subarray(pos))
          break
        }
        if (index >= offset) {
          kept.push(bytes.subarray(pos, collected + 1 === limit ? nl : nl + 1))
          collected++
        }
        index++
        pos = nl + 1
        if (collected === limit) {
          const lines = decodeWindow()
          let count = offset + lines.length + 1
          if (countAll) {
            for (let at = bytes.indexOf(NEWLINE, pos); at >= 0; at = bytes.indexOf(NEWLINE, at + 1)) count++
            for (;;) {
              const more = Buffer.allocUnsafe(CHUNK)
              const read = (await file.read(more, 0, CHUNK, null)).bytesRead
              if (read === 0) break
              const view = more.subarray(0, read)
              for (let at = view.indexOf(NEWLINE); at >= 0; at = view.indexOf(NEWLINE, at + 1)) count++
            }
          }
          return { lines, more: true, total: () => count }
        }
      }
    }
  } finally {
    await file.close()
  }
}