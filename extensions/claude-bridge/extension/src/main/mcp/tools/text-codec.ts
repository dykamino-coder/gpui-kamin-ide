// Text encoding of files the Read/Edit/Write tools touch. A file keeps the
// encoding it had: UTF-8, UTF-8 with BOM, or UTF-16 LE/BE with BOM. Text is
// handled without the BOM and written back with it, so an edit never turns a
// UTF-16 file into mojibake or drops a BOM a toolchain relies on.

export type TextEncoding = 'utf8' | 'utf8bom' | 'utf16le' | 'utf16be'

export interface DecodedText {
  text: string
  encoding: TextEncoding
}

export function detectEncoding(bytes: Buffer): TextEncoding {
  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) return 'utf8bom'
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) return 'utf16le'
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) return 'utf16be'
  return 'utf8'
}

/** A NUL byte in the first 8 KiB of a file without a UTF-16 BOM means binary —
 *  the heuristic of git and ripgrep. */
export function looksBinary(bytes: Buffer, encoding: TextEncoding): boolean {
  if (encoding === 'utf16le' || encoding === 'utf16be') return false
  return bytes.subarray(0, 8192).includes(0)
}

function swap16(bytes: Buffer): Buffer {
  const out = Buffer.from(bytes.subarray(0, bytes.length - (bytes.length % 2)))
  return out.swap16()
}

export function decodeText(bytes: Buffer): DecodedText {
  const encoding = detectEncoding(bytes)
  switch (encoding) {
    case 'utf8bom':
      return { text: bytes.subarray(3).toString('utf8'), encoding }
    case 'utf16le':
      return { text: bytes.subarray(2).toString('utf16le'), encoding }
    case 'utf16be':
      return { text: swap16(bytes.subarray(2)).toString('utf16le'), encoding }
    default:
      return { text: bytes.toString('utf8'), encoding }
  }
}

export function encodeText(text: string, encoding: TextEncoding): Buffer {
  switch (encoding) {
    case 'utf8bom':
      return Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(text, 'utf8')])
    case 'utf16le':
      return Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(text, 'utf16le')])
    case 'utf16be':
      return Buffer.concat([Buffer.from([0xfe, 0xff]), Buffer.from(text, 'utf16le').swap16()])
    default:
      return Buffer.from(text, 'utf8')
  }
}
