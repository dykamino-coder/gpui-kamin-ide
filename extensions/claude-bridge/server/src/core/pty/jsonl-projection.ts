// Lean-DTO projection for JSONL entries on their way to the client.
//
// The CLI writes several heavy fields into each JSONL row that NO Bridge client
// ever reads (verified by grep across webview + electron renderers). Stripping
// them at send time — the server keeps the full row on disk as source of truth —
// cuts 40-60% of jsonl:entries wire bytes on tool-heavy sessions and shrinks the
// extension's 20k-entry RAM cache proportionally.
//
//   toolUseResult   — a structured DUPLICATE of a tool_result's content written
//                     onto the following user entry. For a 2000-line Read that's
//                     the whole file text a second time (~80KB → ~160KB/entry).
//   thinking.signature — 1-7KB base64 per thinking block; never rendered.
//
// Everything else is preserved untouched — the renderer reads many loose
// top-level fields, so this is a strip-list (remove only the known-dead), not an
// allowlist.

interface JsonlLike {
  toolUseResult?: unknown
  message?: { content?: unknown }
  [k: string]: unknown
}

function leanOne(entry: unknown): unknown {
  if (!entry || typeof entry !== 'object') return entry
  const e = entry as JsonlLike
  let out: JsonlLike = e

  if ('toolUseResult' in e) {
    const { toolUseResult: _drop, ...rest } = e
    out = rest
  }

  const content = out.message?.content
  if (Array.isArray(content) && content.some(b => b && typeof b === 'object' && 'signature' in (b as object))) {
    out = {
      ...out,
      message: {
        ...(out.message as object),
        content: content.map(b => {
          if (b && typeof b === 'object' && 'signature' in (b as object)) {
            const { signature: _s, ...rb } = b as { signature?: unknown }
            return rb
          }
          return b
        }),
      },
    }
  }
  return out
}

/** Strip verified-unused heavy fields from a batch of JSONL entries (new objects
 *  — never mutates the server's cached originals). */
export function leanEntries(entries: unknown[]): unknown[] {
  return entries.map(leanOne)
}
