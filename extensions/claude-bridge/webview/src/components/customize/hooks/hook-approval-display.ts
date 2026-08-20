export type ApprovalHandler = unknown

const DANGEROUS_PATTERNS: RegExp[] = [
  /\brm\s+-[^\n]*r[^\n]*f\b/i,
  /\bsudo\b/i,
  /\bcurl\b[^\n|]*\|\s*(?:ba|z|k)?sh\b/i,
  /\bwget\b[^\n|]*\|\s*(?:ba|z|k)?sh\b/i,
  /\beval\b/i,
  /\bdd\s+[^\n]*\bif=.*\bof=\/dev\//i,
  />\s*\/dev\/sd[a-z]/i,
  /\bnc\s+-l\b/i,
  /:\(\)\s*\{\s*:\s*\|\s*:\s*&\s*\}\s*;:/,
  /\bRemove-Item\b[^\n]*(?:-Recurse[^\n]*-Force|-Force[^\n]*-Recurse)/i,
  /\b(?:Invoke-Expression|iex)\b/i,
  /\bStart-Process\b[^\n]*-Verb\s+RunAs\b/i,
]

const SENSITIVE_KEY = /(?:authorization|proxy[-_]?authorization|cookie|set[-_]?cookie|(?:access|refresh|auth)?[-_]?token|api[-_]?key|(?:client[-_]?)?secret|password|passwd|credential|private[-_]?key)$/i
const ENV_CONTAINER_KEY = /^(?:env|environment|environmentVariables)$/i
const SENSITIVE_QUERY_KEY = /(?:(?:access|refresh|auth)?[-_]?token|api[-_]?key|(?:client[-_]?)?secret|password|credential)$/i

function redactUrl(value: string): string {
  try {
    const parsed = new URL(value)
    if (parsed.username) parsed.username = '***'
    if (parsed.password) parsed.password = '***'
    for (const key of Array.from(parsed.searchParams.keys())) {
      if (SENSITIVE_QUERY_KEY.test(key)) parsed.searchParams.set(key, '***')
    }
    return parsed.toString()
  } catch {
    return value
  }
}

function escapeVisualControls(value: string): string {
  return value.replace(/[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, char =>
    `[U+${char.codePointAt(0)!.toString(16).toUpperCase().padStart(4, '0')}]`)
}

export function formatReviewText(value: string): string {
  return escapeVisualControls(value)
}

function redactValue(value: unknown, key = '', envValue = false): unknown {
  if (envValue || SENSITIVE_KEY.test(key)) return value == null ? value : '***'
  if (typeof value === 'string') return escapeVisualControls(key === 'url' ? redactUrl(value) : value)
  if (Array.isArray(value)) return value.map(item => redactValue(item, key, false))
  if (!value || typeof value !== 'object') return value

  const record = value as Record<string, unknown>
  const redactChildren = ENV_CONTAINER_KEY.test(key)
  return Object.fromEntries(Object.entries(record).map(([childKey, childValue]) => [
    escapeVisualControls(childKey),
    redactValue(childValue, childKey, redactChildren),
  ]))
}

function collectInspectionStrings(value: unknown, out: string[]): void {
  if (typeof value === 'string') {
    out.push(value)
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) collectInspectionStrings(item, out)
    return
  }
  if (!value || typeof value !== 'object') return
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    out.push(key)
    collectInspectionStrings(child, out)
  }
}

/** Warning-only heuristic. Approval never depends on a negative result. */
export function looksDangerous(handler: ApprovalHandler): boolean {
  const strings: string[] = []
  collectInspectionStrings(handler, strings)
  let serialized = ''
  try { serialized = JSON.stringify(handler) } catch { /* malformed runtime value */ }
  const text = `${strings.join(' ')}\n${serialized}`
  return DANGEROUS_PATTERNS.some(pattern => pattern.test(text))
}

/** Raw plugin declaration before Bridge rewrites command hooks to a relay. */
export function formatHookDeclaration(handler: ApprovalHandler): string {
  try {
    return JSON.stringify(redactValue(handler), null, 2) ?? '(empty declaration)'
  } catch {
    return '(unreadable declaration)'
  }
}

export function initiallySelectedHookHashes(
  hooks: ReadonlyArray<{ hash: string; handler: ApprovalHandler }>,
  approvedHashes: readonly string[],
): Set<string> {
  const available = new Set(hooks.filter(hook => isReviewableHandler(hook.handler)).map(hook => hook.hash))
  return new Set(approvedHashes.filter(hash => available.has(hash)))
}

export function approvalSelectionKey(
  pluginId: string,
  hooks: ReadonlyArray<{ hash: string }>,
  approvedHashes: readonly string[],
): string {
  return JSON.stringify([
    pluginId,
    hooks.map(hook => hook.hash).sort(),
    [...approvedHashes].sort(),
  ])
}

const SUPPORTED_TYPES = new Set(['command', 'prompt', 'agent', 'http', 'mcp_tool'])

export function handlerTypeLabel(handler: ApprovalHandler): string {
  if (!handler || typeof handler !== 'object') return 'invalid'
  const type = (handler as Record<string, unknown>).type
  return typeof type === 'string' && type ? type : 'invalid'
}

export function isReviewableHandler(handler: ApprovalHandler): handler is Record<string, unknown> {
  if (!handler || typeof handler !== 'object' || Array.isArray(handler)) return false
  const record = handler as Record<string, unknown>
  if (typeof record.type !== 'string' || !SUPPORTED_TYPES.has(record.type)) return false
  if (record.type === 'command') {
    return typeof record.command === 'string'
      && (record.args === undefined || (Array.isArray(record.args) && record.args.every(arg => typeof arg === 'string')))
  }
  if (record.type === 'prompt' || record.type === 'agent') return typeof record.prompt === 'string'
  if (record.type === 'http') return typeof record.url === 'string'
  return typeof record.server === 'string' && typeof record.tool === 'string'
}
