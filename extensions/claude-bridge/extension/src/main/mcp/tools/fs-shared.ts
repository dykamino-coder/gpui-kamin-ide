// Shared primitives for filesystem MCP tools (Read / Write / Edit / Glob /
// Grep). Extracted from `file-system.ts` (Sprint 2 / Stage C, C6) so the
// per-tool modules don't pull in the whole 770-LOC file just to construct
// a result envelope.

export type McpResult = {
  content: Array<{ type: string; text?: string; data?: string; mimeType?: string }>
}

export function textResult(text: string): McpResult {
  return { content: [{ type: 'text', text }] }
}

export function errorResult(message: string): McpResult {
  // Match Claude Code SDK error shape: prefix `Error: ` so the model's
  // built-in error parsing kicks in. Anything past the colon becomes the
  // user-visible reason.
  return {
    content: [
      {
        type: 'text',
        text: message.startsWith('Error:') ? message : `Error: ${message}`,
      },
    ],
  }
}

// ---------------------------------------------------------------------------
// Hard caps shared across the tools.
// ---------------------------------------------------------------------------

/** 50 MB — refuse to read files larger than this entirely (model can't use them). */
export const MAX_READ_BYTES = 50 * 1024 * 1024
/** 256 KB — soft warn threshold; suggest offset/limit when crossed. */
export const WARN_READ_BYTES = 256 * 1024
/** 10 MB — ripgrep stdout cap before we kill the child and slice the buffer. */
export const MAX_GREP_BYTES = 10 * 1024 * 1024
/** Parallel `stat()` batch size for big glob results. */
export const STAT_CONCURRENCY = 32

/** Image extensions Read returns as base64 image content blocks. */
export const IMAGE_EXTENSIONS = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp'])

// ---------------------------------------------------------------------------
// Device path safety — paths whose `read()` would block forever / produce
// infinite output. Refused outright by the Read tool.
// ---------------------------------------------------------------------------

const BLOCKED_DEVICE_PATHS = new Set([
  '/dev/zero', '/dev/random', '/dev/urandom', '/dev/full',
  '/dev/stdin', '/dev/tty', '/dev/console',
  '/dev/stdout', '/dev/stderr',
  '/dev/fd/0', '/dev/fd/1', '/dev/fd/2',
])

export function isBlockedDevicePath(filePath: string): boolean {
  if (BLOCKED_DEVICE_PATHS.has(filePath)) return true
  if (filePath.startsWith('/proc/') && (filePath.endsWith('/fd/0') || filePath.endsWith('/fd/1') || filePath.endsWith('/fd/2'))) return true
  return false
}
