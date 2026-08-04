// The standard VS Code semantic-token legend. Both sides must agree on the
// EXACT order: the renderer registers Monaco's provider with this legend, and
// the host remaps each extension provider's own legend indices into this one
// (Monaco's API takes a single static legend per provider, so we can't pass
// each extension's legend through). Shared (src/api) so host + renderer import
// the same source of truth. Mirrors the types/modifiers VS Code documents at
// https://code.visualstudio.com/api/language-extensions/semantic-highlight-guide
export const STANDARD_SEMANTIC_TOKEN_TYPES: readonly string[] = [
  "namespace", "class", "enum", "interface", "struct", "typeParameter", "type", "parameter",
  "variable", "property", "enumMember", "decorator", "event", "function", "method", "macro",
  "label", "comment", "string", "keyword", "number", "regexp", "operator",
]

export const STANDARD_SEMANTIC_TOKEN_MODIFIERS: readonly string[] = [
  "declaration", "definition", "readonly", "static", "deprecated", "abstract", "async",
  "modification", "documentation", "defaultLibrary",
]
