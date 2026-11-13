import type { Hono } from 'hono'
import { getTerminalSessionCount } from '../../server/terminal'

/** Noninteractive counterparts to the terminal toolbar, without spawning a PTY. */
export function registerTerminalRoutes(api: Hono): void {
  api.get('/api/dashboard/terminal', (c) =>
    c.json({
      sessions: getTerminalSessionCount(),
      actions: [
        { engine: 'claude', action: 'status', method: 'GET', url: '/api/dashboard/auth-status?force=true' },
        { engine: 'claude', action: 'usage', method: 'GET', url: '/api/dashboard/usage?force=1' },
        { engine: 'codex', action: 'status', method: 'GET', url: '/api/dashboard/codex/account?force=true' },
        { engine: 'codex', action: 'usage', method: 'GET', url: '/api/dashboard/codex/limits?force=true' },
        { engine: 'codex', action: 'login', method: 'POST', url: '/api/dashboard/codex/login' },
        { engine: 'codex', action: 'logout', method: 'POST', url: '/api/dashboard/codex/logout' },
      ],
      interactive: {
        url: '/ws/terminal',
        claude: ['interactive', 'resume', 'login', 'logout'],
        codex: ['interactive', 'resume'],
      },
    }),
  )
}
