// `vscode.l10n` — message localization. We ship no translation bundle, so `t`
// just performs the placeholder SUBSTITUTION vscode does: `{0}`/`{1}` positional
// (spread args) or `{key}` named (a single Record arg). Without substitution,
// messages like "directory not found: {0}" reached the UI with a literal `{0}`.

/** Coerce an l10n arg (string | number | boolean) to its display string. */
function argToString(v: unknown): string {
  return typeof v === "string" ? v : typeof v === "number" || typeof v === "boolean" ? String(v) : JSON.stringify(v)
}

function l10nFormat(template: string, args: unknown[]): string {
  if (args.length === 0) return template
  const named = args.length === 1 && typeof args[0] === "object" && args[0] !== null && !Array.isArray(args[0])
    ? (args[0] as Record<string, unknown>) : null
  return template.replace(/\{([^}]+)\}/g, (whole, key: string) => {
    const v = named ? named[key] : args[Number(key)]
    return v === undefined ? whole : argToString(v)
  })
}

export function buildL10n() {
  return {
    t(message: string | { message: string; args?: unknown[] | Record<string, unknown> }, ...rest: unknown[]): string {
      if (typeof message === "object" && message !== null && "message" in message) {
        const a = message.args
        return l10nFormat(message.message, Array.isArray(a) ? a : a !== undefined ? [a] : [])
      }
      if (typeof message === "string") return l10nFormat(message, rest)
      return String(message)
    },
    bundle: undefined, uri: undefined,
  }
}
