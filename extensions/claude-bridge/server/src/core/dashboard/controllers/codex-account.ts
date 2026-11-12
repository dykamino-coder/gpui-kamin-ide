// ============================================================================
// Dashboard: the server's shared ChatGPT (codex) account.
//
// One account per Bridge server, signed in by an administrator with the
// device-code flow: POST /login returns the URL and one-time code; the admin
// completes it in any browser; codex reports completion and the status
// endpoint turns `loggedIn`. All routes sit behind the dashboard auth
// middleware. Tokens never leave CODEX_HOME — only email/plan are reported.
// ============================================================================

import type { Hono } from 'hono'
import { classifyCodexError } from '../../codex/codex-errors'
import { codexRuntime, type CodexRuntime } from '../../codex/codex-runtime'
import { eventBus } from '../../events/bus'

export function registerCodexAccountRoutes(api: Hono, runtime: () => CodexRuntime = codexRuntime): void {
  const fail = (error: unknown) => ({ error: classifyCodexError((error as Error).message) })
  // Live dashboard refresh when the admin finishes (or fails) the device login.
  runtime().onAccountChange((status) =>
    eventBus.emit('account:updated', { engine: 'codex', loggedIn: status.loggedIn }),
  )

  api.get('/api/dashboard/codex/account', async (c) => {
    const status = await runtime().accountStatus({ fresh: c.req.query('force') === 'true' })
    return c.json(status)
  })

  api.post('/api/dashboard/codex/login', async (c) => {
    try {
      const login = await runtime().startDeviceLogin()
      eventBus.emit('account:updated', { engine: 'codex', pendingLogin: true })
      return c.json(login)
    } catch (error) {
      return c.json(fail(error), 502)
    }
  })

  api.post('/api/dashboard/codex/login/cancel', async (c) => {
    await runtime().cancelLogin()
    return c.json({ ok: true })
  })

  api.post('/api/dashboard/codex/logout', async (c) => {
    try {
      await runtime().logout()
      eventBus.emit('account:updated', { engine: 'codex', loggedIn: false })
      return c.json({ ok: true })
    } catch (error) {
      return c.json(fail(error), 502)
    }
  })

  api.get('/api/dashboard/codex/models', async (c) => {
    try {
      return c.json({ models: await runtime().models() })
    } catch (error) {
      return c.json({ models: [], ...fail(error) }, 502)
    }
  })
}
