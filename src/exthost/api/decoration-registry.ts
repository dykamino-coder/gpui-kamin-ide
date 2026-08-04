// Decoration-type registry (B5b-2c). Keeps each type's render options keyed by
// a generated key so `TextEditorDecorationType` stays the d.ts-pure shape
// `{ key, dispose }` — `setDecorations` looks options up here rather than off
// the type object (which lets it work even with a foreign decoration type).
import type { DecorationRenderOptionsDto } from "../host-services.js"

const optionsByKey = new Map<string, DecorationRenderOptionsDto>()
let seq = 0

/** Allocate a unique key for a new decoration type and stash its options. */
export function createDecorationKey(extId: string, options: DecorationRenderOptionsDto): string {
  const key = `deco-${extId}-${String(++seq)}`
  optionsByKey.set(key, options)
  return key
}

/** The render options registered for `key`, or `{}` for an unknown/foreign key. */
export function decorationOptions(key: string): DecorationRenderOptionsDto {
  return optionsByKey.get(key) ?? {}
}

/** Drop a type's options on its `dispose()`. */
export function disposeDecorationKey(key: string): void {
  optionsByKey.delete(key)
}
