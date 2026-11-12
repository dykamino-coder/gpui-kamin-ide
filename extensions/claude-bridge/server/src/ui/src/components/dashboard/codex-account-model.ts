// Pure state logic of the ChatGPT (codex) account card: server status → what
// the card shows, and when the card keeps polling. Kept free of Preact so the
// server's vitest suite can test it without the dashboard dependencies.

export type CodexErrorKind = 'login-required' | 'unauthorized' | 'proxy-auth' | 'tls' | 'network' | 'other'

export interface CodexErrorInfo {
  kind: CodexErrorKind
  message: string
  detail: string
}

export interface CodexDeviceLogin {
  loginId: string
  verificationUrl: string
  userCode: string
  startedAt: number
}

export interface CodexAccountStatus {
  loggedIn: boolean
  email: string | null
  planType: string | null
  accountType: string | null
  pendingLogin: CodexDeviceLogin | null
  error: CodexErrorInfo | null
}

/** Client-side state the server does not know about. */
export interface CodexCardUiState {
  /** First GET still in flight. */
  loading: boolean
  /** A login/cancel/logout request is in flight. */
  busy: boolean
  /** The status request itself failed (network, 5xx) — not a codex error. */
  fetchError: string | null
  /** Error returned by POST login/logout (502 body), newer than the status. */
  actionError: CodexErrorInfo | null
}

export type CodexCardMode = 'loading' | 'signed-in' | 'pending' | 'signed-out'

export interface CodexCardView {
  mode: CodexCardMode
  status: 'ok' | 'warning' | 'error'
  items: { label: string; value: string; color?: string }[]
  pending: CodexDeviceLogin | null
  error: CodexErrorInfo | null
  canSignIn: boolean
  canSignOut: boolean
  canCancel: boolean
}

const PLAN_LABELS: Record<string, string> = {
  free: 'Free',
  plus: 'Plus',
  pro: 'Pro',
  team: 'Team',
  business: 'Business',
  enterprise: 'Enterprise',
  edu: 'Edu',
}

export function formatPlan(planType: string | null): string {
  if (!planType) return '—'
  return PLAN_LABELS[planType.toLowerCase()] ?? planType
}

export function codexCardView(status: CodexAccountStatus | null, ui: CodexCardUiState): CodexCardView {
  const error: CodexErrorInfo | null =
    ui.actionError ??
    status?.error ??
    (ui.fetchError ? { kind: 'network', message: 'Could not reach the Bridge server.', detail: ui.fetchError } : null)

  if (!status) {
    return {
      mode: 'loading',
      status: error ? 'error' : 'warning',
      items: [{ label: 'Status', value: ui.loading ? 'Checking...' : 'Unavailable' }],
      pending: null,
      error,
      canSignIn: !ui.loading && !ui.busy,
      canSignOut: false,
      canCancel: false,
    }
  }

  if (status.loggedIn) {
    return {
      mode: 'signed-in',
      status: error ? 'warning' : 'ok',
      items: [
        { label: 'Email', value: status.email || '—' },
        { label: 'Plan', value: formatPlan(status.planType) },
      ],
      pending: null,
      error,
      canSignIn: false,
      canSignOut: !ui.busy,
      canCancel: false,
    }
  }

  if (status.pendingLogin) {
    return {
      mode: 'pending',
      status: 'warning',
      items: [{ label: 'Status', value: 'Waiting for sign-in', color: 'var(--accent-yellow)' }],
      pending: status.pendingLogin,
      error,
      canSignIn: false,
      canSignOut: false,
      canCancel: !ui.busy,
    }
  }

  const otherAuth = status.accountType && status.accountType !== 'chatgpt'
  return {
    mode: 'signed-out',
    status: 'error',
    items: [
      { label: 'Status', value: 'Not signed in', color: 'var(--accent-red)' },
      ...(otherAuth ? [{ label: 'Auth', value: `${status.accountType} (not used)` }] : []),
    ],
    pending: null,
    error,
    canSignIn: !ui.busy,
    canSignOut: false,
    canCancel: false,
  }
}

/** Runs a card action (sign in, cancel, sign out), then hands its result to
 *  `after`. Written out because `after?.(await action())` never calls the
 *  action when there is no `after`: optional call skips its arguments. */
export async function runCardAction<T>(action: () => Promise<T>, after?: (result: T) => void): Promise<void> {
  const result = await action()
  after?.(result)
}

/** Poll while a device login is waiting for the admin; stop once it resolves. */
export function shouldPollCodexAccount(status: CodexAccountStatus | null): boolean {
  return !!status && !status.loggedIn && status.pendingLogin !== null
}

export const CODEX_POLL_INTERVAL_MS = 3000

/** `account:updated` is shared with the Claude account; codex events carry `engine: 'codex'`. */
export function isCodexAccountEvent(data: unknown): boolean {
  return typeof data === 'object' && data !== null && (data as { engine?: unknown }).engine === 'codex'
}

/** Extracts `{ error }` from a 502 body; anything else becomes a generic error. */
export function toCodexError(body: unknown, fallback: string): CodexErrorInfo {
  const e = (body as { error?: Partial<CodexErrorInfo> } | null)?.error
  if (e && typeof e.message === 'string') {
    return { kind: (e.kind as CodexErrorKind) ?? 'other', message: e.message, detail: e.detail ?? '' }
  }
  return { kind: 'other', message: 'The request failed.', detail: fallback }
}

/** `codex` block of /api/dashboard/status. */
export interface CodexStatus {
  available: boolean
  version: string | null
  reason: string | null
  /** Version the image pins (BRIDGE_CODEX_VERSION), if any. */
  pinned: string | null
}

/** Status-page row: the codex version Bridge runs, or why it cannot. */
export function codexStatusRow(status: CodexStatus | undefined): { text: string; ok: boolean } {
  if (!status) return { text: '—', ok: true }
  if (!status.available) return { text: `unavailable — ${status.reason ?? 'unknown reason'}`, ok: false }
  const version = status.version ?? 'unknown version'
  if (status.pinned && status.version && status.pinned !== status.version) {
    return { text: `${version} (image pins ${status.pinned})`, ok: false }
  }
  return { text: version, ok: true }
}
