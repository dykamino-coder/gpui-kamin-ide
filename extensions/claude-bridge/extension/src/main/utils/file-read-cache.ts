// Bounded LRU file-content cache keyed by mtime and size.
//
// The Read MCP tool is the single hottest tool in a long chat — model
// re-reads the same skill / agent / source file 5–10× per session as it
// browses around. Each call hits the disk: stat + readFile + decode.
//
// Mtime + size is the cache key on top of path: when a tool just rewrote the
// file (Edit / Write / Bash) the stat changes and we re-read. Write/Edit also
// invalidate explicitly because mtime resolution can be coarse.
//
// Mirrors CLI's `utils/fileReadCache.ts` — И его баг: до 2.1.208 CLI пиннил
// до 1000 ПОЛНЫХ файлов без байтового лимита и чинил это «bounding the cache
// to 16 MB». У нас была та же дыра (1024 записи × до 50MB файл = гигабайты в
// extension host): теперь суммарный байтовый бюджет + порог на файл.

import fs from 'node:fs/promises'
import type { Stats } from 'node:fs'
import { decodeText, detectEncoding, looksBinary, type TextEncoding } from '../mcp/tools/text-codec'

const CACHE_CAP = 1024
// Суммарный бюджет — как у CLI 2.1.208.
const CACHE_BYTES_BUDGET = 16 * 1024 * 1024
// Файл крупнее порога кэшировать бессмысленно: одна запись вытеснит пол-кэша,
// а перечитывание больших файлов редкое.
const MAX_CACHEABLE_FILE_BYTES = 2 * 1024 * 1024

/** File text without its BOM, plus what is needed to write it back unchanged. */
export interface CachedText {
  /** Decoded text; empty for a binary file. */
  content: string
  encoding: TextEncoding
  binary: boolean
}

interface CachedFile extends CachedText {
  mtimeMs: number
  size: number
  /** UTF-16 длина строки ×2 ≈ байты в куче — дешёвая оценка без Buffer. */
  bytes: number
}

const cache = new Map<string, CachedFile>()
let cacheBytes = 0

function evictOldest(): void {
  const oldest = cache.keys().next().value
  if (oldest === undefined) return
  const e = cache.get(oldest)
  if (e) cacheBytes -= e.bytes
  cache.delete(oldest)
}

/** Read a text file in its own encoding, returning a cached copy if the path's
 *  mtime and size haven't changed since the previous read. `stat` saves a
 *  second stat when the caller already has one. */
export async function readTextCached(filePath: string, stat?: Stats): Promise<CachedText> {
  stat ??= await fs.stat(filePath)
  if (!stat.isFile()) throw new Error(`Not a file: ${filePath}`)

  const cached = cache.get(filePath)
  if (cached && cached.mtimeMs === stat.mtimeMs && cached.size === stat.size) {
    // LRU touch — re-insert moves to the end of the iteration order.
    cache.delete(filePath)
    cache.set(filePath, cached)
    return cached
  }

  const raw = await fs.readFile(filePath)
  const binary = looksBinary(raw, detectEncoding(raw))
  const decoded = binary ? { text: '', encoding: 'utf8' as const } : decodeText(raw)
  const entry: CachedFile = {
    content: decoded.text,
    encoding: decoded.encoding,
    binary,
    mtimeMs: stat.mtimeMs,
    size: stat.size,
    bytes: decoded.text.length * 2,
  }
  invalidateFileCache(filePath)
  if (entry.bytes > MAX_CACHEABLE_FILE_BYTES) return entry
  // Evict до влезания: по числу записей И по байтовому бюджету.
  while (cache.size >= CACHE_CAP || (cache.size > 0 && cacheBytes + entry.bytes > CACHE_BYTES_BUDGET)) {
    evictOldest()
  }
  cache.set(filePath, entry)
  cacheBytes += entry.bytes
  return entry
}

/** UTF-8/UTF-16 text of a file (BOM removed). */
export async function readFileCached(filePath: string): Promise<string> {
  return (await readTextCached(filePath)).content
}

/** Drop an entry — call after Write/Edit so the next Read re-fetches even
 *  if mtime resolution is too coarse to detect a same-millisecond rewrite. */
export function invalidateFileCache(filePath: string): void {
  const e = cache.get(filePath)
  if (e) cacheBytes -= e.bytes
  cache.delete(filePath)
}

/** Drop all entries. Reserved for tests / memory pressure responses. */
export function clearFileCache(): void {
  cache.clear()
  cacheBytes = 0
}

/** Diagnostic snapshot for telemetry / tests. Don't persist — the Map is
 *  the source of truth, and `entries` snapshots may go stale by the time
 *  the caller acts on them. */
export function fileCacheStats(): { size: number; capacity: number; bytes: number; bytesBudget: number } {
  return { size: cache.size, capacity: CACHE_CAP, bytes: cacheBytes, bytesBudget: CACHE_BYTES_BUDGET }
}
