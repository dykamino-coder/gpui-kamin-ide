import { constants } from 'node:fs'
import { open, opendir, rename, stat } from 'node:fs/promises'
import { resolve } from 'node:path'

export const FILE_INVOKES = Object.freeze([
  'file-viewer:read', 'file-viewer:read-binary', 'file-viewer:write',
  'file-tree:list-dir', 'file-tree:rename',
] as const)

// Leave room for base64/JSON inside the transport's 16 MiB frame limit.
export const MAX_FILE_BYTES = 8 * 1024 * 1024
const MAX_DIRECTORY_ENTRIES = 10_000

export function fileText(bytes: Buffer): string {
  const text = bytes.toString('utf8')
  // Control bytes and invalid UTF-8 may expand substantially in JSON. Reject
  // explicitly instead of letting an otherwise valid read close the socket.
  if (Buffer.byteLength(JSON.stringify(text)) > 12 * 1024 * 1024) {
    throw new Error('Text exceeds file viewer encoded frame limit')
  }
  return text
}

export function filePath(root: string, value: unknown): string {
  if (typeof value !== 'string' || !value || value.includes('\0') || value.length > 32_768) {
    throw new Error('Expected a nonempty container file path')
  }
  // The container is the machine boundary. Absolute paths remain absolute;
  // relative paths resolve against the configured workspace, never host cwd.
  return resolve(root, value)
}

function metadata(value: { mtimeMs: number; size: number }) {
  return { mtimeMs: value.mtimeMs, size: value.size }
}

export async function readContainerFile(path: string): Promise<{ bytes: Buffer; mtimeMs: number; size: number }> {
  // NONBLOCK prevents a user-selected FIFO from holding a browser request open.
  const handle = await open(path, constants.O_RDONLY | constants.O_NONBLOCK)
  try {
    const before = await handle.stat()
    if (!before.isFile()) throw new Error('File viewer requires a regular file')
    if (before.size > MAX_FILE_BYTES) throw new Error('File exceeds 8 MiB viewer limit')
    const chunks: Buffer[] = []
    let size = 0
    while (true) {
      // Do not trust the initial stat: a process may grow the file while reading.
      const buffer = Buffer.allocUnsafe(Math.min(64 * 1024, MAX_FILE_BYTES + 1 - size))
      const { bytesRead } = await handle.read(buffer)
      if (!bytesRead) break
      size += bytesRead
      if (size > MAX_FILE_BYTES) throw new Error('File exceeds 8 MiB viewer limit')
      chunks.push(buffer.subarray(0, bytesRead))
    }
    const after = await handle.stat()
    if (before.size !== after.size || before.mtimeMs !== after.mtimeMs || size !== after.size) {
      throw new Error('File changed while reading; retry the read')
    }
    return { bytes: Buffer.concat(chunks, size), mtimeMs: after.mtimeMs, size }
  } finally { await handle.close() }
}

export async function invokeFile(root: string, channel: string, args: unknown[]): Promise<unknown> {
  const count = channel === 'file-viewer:write' || channel === 'file-tree:rename' ? 2 : 1
  if (args.length !== count) throw new Error(`Expected ${count} file arguments`)
  const path = filePath(root, args[0])
  if (channel === 'file-viewer:read' || channel === 'file-viewer:read-binary') {
    const { bytes, ...info } = await readContainerFile(path)
    return channel === 'file-viewer:read' ? { content: fileText(bytes), ...info }
      : { base64: bytes.toString('base64'), ...info }
  }
  if (channel === 'file-viewer:write') {
    const content = args[1]
    if (typeof content !== 'string') throw new Error('Expected file content as text')
    if (Buffer.byteLength(content) > MAX_FILE_BYTES) throw new Error('File exceeds 8 MiB viewer limit')
    const handle = await open(path, constants.O_WRONLY | constants.O_CREAT | constants.O_NONBLOCK, 0o666)
    try {
      if (!(await handle.stat()).isFile()) throw new Error('File viewer requires a regular file')
      await handle.truncate(0)
      await handle.writeFile(content, 'utf8')
      return metadata(await handle.stat())
    } finally { await handle.close() }
  }
  if (channel === 'file-tree:rename') {
    const destination = filePath(root, args[1])
    await rename(path, destination)
    return metadata(await stat(destination))
  }
  if (channel !== 'file-tree:list-dir') throw new Error('Unsupported file operation')
  const entries = []
  let scanned = 0
  const directory = await opendir(path)
  for await (const entry of directory) {
    if (++scanned > MAX_DIRECTORY_ENTRIES) throw new Error('Directory exceeds 10000 entries')
    const child = resolve(path, entry.name)
    try {
      const info = await stat(child)
      entries.push({ name: entry.name, path: child, isDir: info.isDirectory(), ...metadata(info) })
    } catch (error) {
      // A concurrent rename/unlink must not turn an otherwise valid listing
      // into failure. Permission and I/O errors remain explicit.
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    }
  }
  return entries.sort((a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name))
}
