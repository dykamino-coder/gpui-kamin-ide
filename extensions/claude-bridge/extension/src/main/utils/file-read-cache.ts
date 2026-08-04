// Bounded LRU file-content cache keyed by mtime.
//
// The Read MCP tool is the single hottest tool in a long chat — model
// re-reads the same skill / agent / source file 5–10× per session as it
// browses around. Each call hits the disk: stat + readFile + utf-8 decode
// (≤256KB warm-path file ≈ ~3ms on SSD, but adds up over 50+ Read calls).
//
// Mtime is the cache key on top of path: when a tool just rewrote the
// file (Edit / Write / Bash) the mtime bumps and we re-read. No explicit
// invalidation required — the OS gives us free correctness via the stat.
//
// Mirrors CLI's `utils/fileReadCache.ts` — И его баг: до 2.1.208 CLI пиннил
// до 1000 ПОЛНЫХ файлов без байтового лимита и чинил это «bounding the cache
// to 16 MB». У нас была та же дыра (1024 записи × до 50MB файл = гигабайты в
// extension host): теперь суммарный байтовый бюджет + порог на файл.

import fs from 'node:fs/promises'

const CACHE_CAP = 1024
// Суммарный бюджет — как у CLI 2.1.208.
const CACHE_BYTES_BUDGET = 16 * 1024 * 1024
// Файл крупнее порога кэшировать бессмысленно: одна запись вытеснит пол-кэша,
// а перечитывание больших файлов редкое.
const MAX_CACHEABLE_FILE_BYTES = 2 * 1024 * 1024

interface CachedFile {
  content: string
  mtimeMs: number
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

/** Read a UTF-8 text file, returning a cached copy if the path's mtime
 *  hasn't changed since the previous read. Bypasses the cache for binary /
 *  encoding-sensitive callers — they should `fs.readFile` directly. */
export async function readFileCached(filePath: string): Promise<string> {
  const stat = await fs.stat(filePath)
  if (!stat.isFile()) throw new Error(`Not a file: ${filePath}`)

  const cached = cache.get(filePath)
  if (cached && cached.mtimeMs === stat.mtimeMs) {
    // LRU touch — re-insert moves to the end of the iteration order.
    cache.delete(filePath)
    cache.set(filePath, cached)
    return cached.content
  }

  const content = await fs.readFile(filePath, 'utf8')
  const bytes = content.length * 2
  if (bytes > MAX_CACHEABLE_FILE_BYTES) {
    // Крупный файл мимо кэша (и выкинуть возможную старую версию).
    invalidateFileCache(filePath)
    return content
  }
  const entry: CachedFile = { content, mtimeMs: stat.mtimeMs, bytes }

  const prev = cache.get(filePath)
  if (prev) {
    cacheBytes -= prev.bytes
    cache.delete(filePath)
  }
  // Evict до влезания: по числу записей И по байтовому бюджету.
  while (cache.size >= CACHE_CAP || (cache.size > 0 && cacheBytes + bytes > CACHE_BYTES_BUDGET)) {
    evictOldest()
  }
  cache.set(filePath, entry)
  cacheBytes += bytes
  return content
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
