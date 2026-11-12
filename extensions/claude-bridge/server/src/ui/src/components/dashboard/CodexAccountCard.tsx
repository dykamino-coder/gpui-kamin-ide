import { useEffect, useRef, useState } from 'preact/compat'
import { StatusCard } from './StatusCard'
import { api, CodexApiError } from '../../services/api-client'
import { codexAccountEventAt } from '../../signals/server'
import {
  CODEX_POLL_INTERVAL_MS,
  codexCardView,
  shouldPollCodexAccount,
  toCodexError,
  type CodexAccountStatus,
  type CodexErrorInfo,
} from './codex-account-model'
import styles from './CodexAccountCard.module.css'

function actionError(error: unknown): CodexErrorInfo {
  return error instanceof CodexApiError ? error.info : toCodexError(null, (error as Error)?.message ?? String(error))
}

/** The Bridge server's shared ChatGPT account (codex engine), signed in with the device-code flow. */
export function CodexAccountCard() {
  const [status, setStatus] = useState<CodexAccountStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [busy, setBusy] = useState(false)
  const [fetchError, setFetchError] = useState<string | null>(null)
  const [actionErr, setActionErr] = useState<CodexErrorInfo | null>(null)
  const [copied, setCopied] = useState(false)
  const [showDetail, setShowDetail] = useState(false)
  // Drops responses that arrive after a newer request started.
  const seq = useRef(0)

  const refresh = (force = false) => {
    const id = ++seq.current
    if (force) setRefreshing(true)
    return api
      .getCodexAccount(force)
      .then((next) => {
        if (id !== seq.current) return
        setStatus(next)
        setFetchError(null)
      })
      .catch((error: Error) => {
        if (id === seq.current) setFetchError(error.message)
      })
      .finally(() => {
        if (id !== seq.current) return
        setLoading(false)
        setRefreshing(false)
      })
  }

  useEffect(() => {
    void refresh()
  }, [])

  // Live push from the server when the device login completes or the account changes.
  const eventAt = codexAccountEventAt.value
  useEffect(() => {
    if (eventAt > 0) void refresh()
  }, [eventAt])

  // Fallback while the admin completes the login (e.g. the dashboard socket is down).
  const polling = shouldPollCodexAccount(status)
  useEffect(() => {
    if (!polling) return
    const timer = setInterval(() => void refresh(), CODEX_POLL_INTERVAL_MS)
    return () => clearInterval(timer)
  }, [polling])

  const run = async (action: () => Promise<unknown>, after?: (result: unknown) => void) => {
    setBusy(true)
    setActionErr(null)
    setShowDetail(false)
    try {
      after?.(await action())
    } catch (error) {
      setActionErr(actionError(error))
    } finally {
      setBusy(false)
      void refresh()
    }
  }

  const signIn = () =>
    run(api.startCodexLogin, (login) => {
      setCopied(false)
      // Show the code at once; the follow-up refresh confirms it.
      setStatus((prev) =>
        prev ? { ...prev, error: null, pendingLogin: login as CodexAccountStatus['pendingLogin'] } : prev,
      )
    })
  const cancel = () => run(api.cancelCodexLogin)
  const signOut = () => {
    if (
      !confirm(
        'Sign out of the ChatGPT account?\nCodex chats on this Bridge server will stop working until an administrator signs in again.',
      )
    )
      return
    void run(api.logoutCodex)
  }

  const copyCode = (code: string) => {
    void navigator.clipboard
      ?.writeText(code)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      })
      .catch(() => undefined)
  }

  const view = codexCardView(status, { loading, busy, fetchError, actionError: actionErr })

  return (
    <StatusCard
      icon="fa-solid fa-robot"
      iconColor="var(--accent-green)"
      title="ChatGPT (Codex)"
      status={view.status}
      items={view.items}
      action={{
        icon: 'fa-solid fa-rotate',
        label: 'Refresh ChatGPT account',
        onClick: () => void refresh(true),
        loading: loading || refreshing,
      }}
    >
      {view.error && (
        <div class={styles.error} role="alert">
          <div class={styles.errorMessage}>
            <i class="fa-solid fa-triangle-exclamation" /> {view.error.message}
          </div>
          {view.error.detail && (
            <button type="button" class={styles.detailToggle} onClick={() => setShowDetail(!showDetail)}>
              {showDetail ? 'Hide details' : 'Details'}
            </button>
          )}
          {showDetail && view.error.detail && <div class={styles.errorDetail}>{view.error.detail}</div>}
        </div>
      )}

      {view.pending && (
        <div class={styles.pending}>
          <div class={styles.hint}>Open the link, sign in to ChatGPT and enter the code</div>
          <a class={styles.link} href={view.pending.verificationUrl} target="_blank" rel="noopener noreferrer">
            {view.pending.verificationUrl} <i class="fa-solid fa-arrow-up-right-from-square" />
          </a>
          <div class={styles.codeRow}>
            <span class={styles.code}>{view.pending.userCode}</span>
            <button
              type="button"
              class={styles.secondaryBtn}
              onClick={() => copyCode(view.pending!.userCode)}
              title="Copy the one-time code"
            >
              <i class={copied ? 'fa-solid fa-check' : 'fa-regular fa-copy'} /> {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
          <div class={styles.actions}>
            <span class={styles.waiting}>
              <i class="fa-solid fa-spinner fa-spin" /> Waiting for sign-in…
            </span>
            <button type="button" class={styles.secondaryBtn} onClick={cancel} disabled={!view.canCancel}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {(view.mode === 'signed-out' || (view.mode === 'loading' && !loading)) && (
        <div class={styles.center}>
          <button type="button" class={styles.loginBtn} onClick={signIn} disabled={!view.canSignIn}>
            <i class={busy ? 'fa-solid fa-spinner fa-spin' : 'fa-solid fa-right-to-bracket'} /> Sign in with ChatGPT
          </button>
        </div>
      )}

      {view.mode === 'signed-in' && (
        <div class={styles.center}>
          <button type="button" class={styles.secondaryBtn} onClick={signOut} disabled={!view.canSignOut}>
            <i class="fa-solid fa-right-from-bracket" /> Sign out
          </button>
        </div>
      )}
    </StatusCard>
  )
}
