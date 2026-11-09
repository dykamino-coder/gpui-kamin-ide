// Trusted application bytes are snapshotted at startup, never read from a
// browser-supplied filesystem path or the agent's mutable project directory.
import { lstat, readdir, readFile, realpath } from 'node:fs/promises'
import type { IncomingMessage, ServerResponse } from 'node:http'
import path from 'node:path'

const MAX_FILE = 16 * 1024 * 1024
const MAX_TOTAL = 64 * 1024 * 1024
const MAX_FILES = 4096
const MIME: Record<string, string> = {
  '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp',
  '.avif': 'image/avif', '.ico': 'image/x-icon', '.woff': 'font/woff',
  '.woff2': 'font/woff2', '.ttf': 'font/ttf', '.otf': 'font/otf',
  '.wasm': 'application/wasm', '.mp3': 'audio/mpeg', '.wav': 'audio/wav',
}
interface Asset { bytes: Buffer; mime: string }

/** Load only browser.html and supported assets/ files from a dedicated Vite
 * build directory. Missing entry, links, special files and oversized builds
 * fail before listening. Replacing on-disk files cannot change a running UI. */
export async function createAppAssetHandler(directory: string) {
  const root = await realpath(directory)
  const files = new Map<string, Asset>()
  let total = 0
  let entries = 0
  async function load(relative: string, mime: string): Promise<void> {
    const filename = path.join(root, relative)
    const stat = await lstat(filename)
    if (!stat.isFile()) throw new Error('Application assets must be regular files without symlinks')
    if (stat.size > MAX_FILE) throw new Error('Application asset exceeds 16 MiB')
    const bytes = await readFile(filename)
    total += bytes.length
    if (bytes.length > MAX_FILE || total > MAX_TOTAL) throw new Error('Application build exceeds asset byte limits')
    files.set('/' + relative.split(path.sep).join('/'), { bytes, mime })
  }
  async function walk(relative: string, depth: number): Promise<void> {
    if (depth > 8) throw new Error('Application assets exceed directory depth limit')
    const filename = path.join(root, relative)
    if (!(await lstat(filename)).isDirectory()) throw new Error('Application assets must be a directory without symlinks')
    for (const entry of await readdir(filename, { withFileTypes: true })) {
      if (++entries > MAX_FILES) throw new Error('Application assets exceed file count limit')
      if (entry.isSymbolicLink() || (!entry.isFile() && !entry.isDirectory())) {
        throw new Error('Application assets must be regular files without symlinks')
      }
      if (!/^[A-Za-z0-9_-][A-Za-z0-9_.-]*$/.test(entry.name)) continue
      const next = path.join(relative, entry.name)
      if (entry.isDirectory()) await walk(next, depth + 1)
      else if (MIME[path.extname(entry.name)]) await load(next, MIME[path.extname(entry.name)])
    }
  }
  await load('browser.html', 'text/html; charset=utf-8')
  await walk('assets', 0)

  return function serveApp(request: IncomingMessage, response: ServerResponse): void {
    // Do not normalize traversal into a valid asset, accept absolute-form URLs,
    // or fall back to HTML for unknown paths (including project filenames).
    let pathname = ''
    try { pathname = decodeURIComponent((request.url ?? '').split('?')[0]) } catch { /* 404 below */ }
    const valid = pathname.startsWith('/') && !/[\\\0#]/.test(pathname) &&
      !pathname.slice(1).split('/').some(part => part === '.' || part === '..' || part === '')
    const asset = valid || pathname === '/' ? files.get(pathname === '/' ? '/browser.html' : pathname) : undefined
    response.setHeader('Cache-Control', 'no-store')
    response.setHeader('X-Content-Type-Options', 'nosniff')
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      response.writeHead(405, { Allow: 'GET, HEAD' }); response.end(); return
    }
    if (!asset) { response.writeHead(404); response.end(); return }
    response.writeHead(200, { 'Content-Type': asset.mime, 'Content-Length': asset.bytes.length })
    response.end(request.method === 'HEAD' ? undefined : asset.bytes)
  }
}
