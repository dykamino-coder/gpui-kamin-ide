// ============================================================================
// Plain-language codex failures for the chat and the dashboard.
//
// codex reports transport failures as free text (reqwest/rustls messages,
// HTTP status lines). The common deployment problems behind a corporate proxy
// each have one fix on the Bridge side, so the user-facing text names it.
// ============================================================================

export type CodexErrorKind = 'login-required' | 'unauthorized' | 'proxy-auth' | 'tls' | 'network' | 'other'

export interface CodexErrorInfo {
  kind: CodexErrorKind
  /** One-line explanation with the fix, shown in chat / dashboard. */
  message: string
  /** The original text, kept for the log and the details view. */
  detail: string
}

const HINTS: Record<Exclude<CodexErrorKind, 'other'>, string> = {
  'login-required':
    'ChatGPT is not signed in on this Bridge server. An administrator can sign in from the dashboard (ChatGPT card).',
  unauthorized:
    'ChatGPT rejected the Bridge sign-in (expired or revoked). An administrator must sign in again from the dashboard.',
  'proxy-auth':
    'The corporate proxy requires authentication (HTTP 407). Put the credentials into the proxy URL in Bridge network settings.',
  tls: 'TLS verification failed. Add the corporate CA certificate in Bridge network settings; codex receives it as CODEX_CA_CERTIFICATE.',
  network: 'Bridge cannot reach the OpenAI API. Check the proxy settings and network access of the Bridge server.',
}

const PATTERNS: Array<[Exclude<CodexErrorKind, 'other' | 'login-required'>, RegExp]> = [
  ['proxy-auth', /\b407\b|proxy authentication required/i],
  [
    'tls',
    /certificate|self[- ]signed|unknown ?issuer|unable to get local issuer|invalid peer cert|handshake failure|\btls\b|\bssl\b/i,
  ],
  ['unauthorized', /\b401\b|unauthori[sz]ed|token (?:is )?(?:expired|invalid|revoked)|refresh[_ ]token|re-?login/i],
  [
    'network',
    /ECONNREFUSED|ECONNRESET|ENOTFOUND|EAI_AGAIN|connection (?:refused|reset)|dns error|timed? ?out|stream disconnected/i,
  ],
]

export function classifyCodexError(text: string): CodexErrorInfo {
  const detail = text.trim()
  for (const [kind, re] of PATTERNS) if (re.test(detail)) return { kind, message: HINTS[kind], detail }
  return { kind: 'other', message: detail || 'codex failed', detail }
}

export function loginRequired(): CodexErrorInfo {
  return { kind: 'login-required', message: HINTS['login-required'], detail: 'account/read: no account' }
}

/** `hint (detail)` unless the hint already is the detail. */
export function describeCodexError(info: CodexErrorInfo): string {
  return info.kind === 'other' || info.message === info.detail ? info.message : `${info.message} (${info.detail})`
}
