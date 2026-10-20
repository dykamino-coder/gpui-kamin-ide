// Argument parsing and continuation cursors for the project session tools.
// Arguments come from the model, so every value is range-checked here and an
// invalid one fails the call with a short, actionable message.

import { KINDS, type Kind } from './render'

export const DEFAULT_MAX_CHARS = 60_000
export const MAX_MAX_CHARS = 200_000

export class ArgError extends Error {}

export function intArg(v: unknown, name: string, def: number, min: number, max: number): number {
  if (v === undefined || v === null) return def
  const n = typeof v === 'string' && v.trim() !== '' ? Number(v) : v
  if (typeof n !== 'number' || !Number.isInteger(n) || n < min || n > max) {
    throw new ArgError(`${name} must be an integer in [${min}, ${max}]`)
  }
  return n
}

export function optIntArg(v: unknown, name: string, min: number, max: number): number | null {
  return v === undefined || v === null ? null : intArg(v, name, 0, min, max)
}

export function boolArg(v: unknown, name: string, def: boolean): boolean {
  if (v === undefined || v === null) return def
  if (typeof v === 'boolean') return v
  if (v === 'true' || v === 'false') return v === 'true'
  throw new ArgError(`${name} must be a boolean`)
}

export function strArg(v: unknown, name: string, maxLen: number): string | null {
  if (v === undefined || v === null || v === '') return null
  if (typeof v !== 'string' || v.length > maxLen)
    throw new ArgError(`${name} must be a string of at most ${maxLen} chars`)
  return v
}

export function timeArg(v: unknown, name: string): number | null {
  if (v === undefined || v === null || v === '') return null
  const t = typeof v === 'number' ? v : typeof v === 'string' ? Date.parse(v) : NaN
  if (!Number.isFinite(t)) throw new ArgError(`${name} must be an ISO-8601 time`)
  return t
}

export function kindsArg(v: unknown): Set<Kind> | null {
  if (v === undefined || v === null) return null
  const list = typeof v === 'string' ? v.split(',').map((s) => s.trim()) : v
  if (!Array.isArray(list) || list.length === 0)
    throw new ArgError(`kinds must be a non-empty list of ${KINDS.join(', ')}`)
  const out = new Set<Kind>()
  for (const k of list) {
    if (typeof k !== 'string' || !(KINDS as readonly string[]).includes(k)) {
      throw new ArgError(`unknown kind ${JSON.stringify(k)}; expected ${KINDS.join(', ')}`)
    }
    out.add(k as Kind)
  }
  return out
}

export function maxCharsArg(v: unknown): number {
  return intArg(v, 'max_chars', DEFAULT_MAX_CHARS, 1000, MAX_MAX_CHARS)
}

/** Opaque continuation: entry index, byte offset of its line, offset inside it. */
export interface Cursor {
  i: number
  b: number
  c: number
  /** search only: the transcript key (`conversationId` or `conversationId/agentId`). */
  k?: string
  /** raw only: 1 when `c` counts bytes into an oversized line (byte paging). */
  o?: number
}

export function encodeCursor(c: Cursor): string {
  return Buffer.from(JSON.stringify(c)).toString('base64url')
}

export function decodeCursor(v: unknown): Cursor | null {
  if (v === undefined || v === null || v === '') return null
  if (typeof v !== 'string' || v.length > 512) throw new ArgError('cursor is invalid')
  try {
    const c = JSON.parse(Buffer.from(v, 'base64url').toString('utf8')) as Cursor
    const ok = (n: unknown) => typeof n === 'number' && Number.isSafeInteger(n) && n >= 0
    if (
      !ok(c.i) ||
      !ok(c.b) ||
      !ok(c.c) ||
      (c.k !== undefined && (typeof c.k !== 'string' || c.k.length > 300)) ||
      (c.o !== undefined && c.o !== 1)
    ) {
      throw new Error('shape')
    }
    return c
  } catch {
    throw new ArgError('cursor is invalid; pass next_cursor from the previous result unchanged')
  }
}
